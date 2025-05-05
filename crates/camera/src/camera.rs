use std::{
    fmt::Debug,
    sync::{Arc, atomic, mpsc},
};

use log::{debug, error, trace};
use replace_with::replace_with_or_abort_and_return;

use crate::{CameraHandler, CameraHandlers, CameraState};

// number of frames that will be kept in the channel
const BUFFERED_FRAMES: usize = 30;

type BoxedHandler = Box<dyn CameraHandler>;

#[derive(Debug)]
pub struct Atomics {
    should_stop: atomic::AtomicBool,
    frame_rate: atomic::AtomicU16,
    target_frame_rate: atomic::AtomicU16,
}

impl Atomics {
    fn new(target_frame_rate: u16) -> Self {
        Self {
            should_stop: atomic::AtomicBool::new(false),
            frame_rate: atomic::AtomicU16::new(0),
            target_frame_rate: atomic::AtomicU16::new(target_frame_rate),
        }
    }
}

#[derive(Debug)]
enum InternalState {
    Waiting(BoxedHandler),
    Connected {
        thread: std::thread::JoinHandle<BoxedHandler>,
        frame_rx: mpsc::Receiver<Vec<u8>>,
    },
}

#[derive(Debug)]
pub struct Camera {
    state: InternalState,
    atomics: Arc<Atomics>,
}

impl Camera {
    pub fn new(handler: CameraHandlers, frame_rate: u16) -> Camera {
        use crate::backends::*;

        #[allow(unreachable_patterns)]
        let backend: BoxedHandler = match handler {
            CameraHandlers::NoOp => Box::new(NoOpCamera::init()),
            CameraHandlers::OpenCV => Box::new(OpenCVCamera::init()),
            CameraHandlers::OpenIris => Box::new(OpenIrisCamera::init()),
            _ => {
                panic!("Unsupported camera handler");
            }
        };

        Self::from_camera_handler(backend, frame_rate)
    }

    pub fn from_camera_handler(handler: BoxedHandler, target_frame_rate: u16) -> Camera {
        Self {
            state: InternalState::Waiting(handler),
            atomics: Arc::new(Atomics::new(target_frame_rate)),
        }
    }

    /// Retrieves the current most recent frame from the camera
    /// - Returns an error if the camera is not connected
    pub fn get_frame(&self) -> Result<Vec<u8>, CameraState> {
        match &self.state {
            InternalState::Waiting(..) => Err(CameraState::Disconnected),
            InternalState::Connected {
                thread: _,
                frame_rx,
            } => match frame_rx.recv() {
                Ok(mut frame) => Ok(std::mem::take(&mut frame)),
                Err(e) => Err(CameraState::Error(e.to_string())),
            },
        }
    }

    pub fn connect(&mut self, source: String) -> Result<(), CameraState> {
        replace_with_or_abort_and_return(&mut self.state, |state| match state {
            InternalState::Connected { .. } => {
                (Err(CameraState::Error("Already connected".into())), state)
            }
            InternalState::Waiting(mut handler) => {
                // rationale: we're handing off the handler to the [`handler_recv`] thread
                // and transitioning our state to [`InternalState::Connected`], where handler is
                // not needed.
                // the handle later gets reclaimed in [`Camera::disconnect`], see return value
                // of the thread binding in current scope.

                if let Err(e) = handler.connect(source) {
                    return (Err(e), InternalState::Waiting(handler));
                };

                let (frame_tx, frame_rx) = mpsc::sync_channel(BUFFERED_FRAMES);
                let thread = handler_recv(handler, frame_tx, self.atomics.clone());

                (Ok(()), InternalState::Connected { thread, frame_rx })
            }
        })
    }

    /// Disconnects the camera
    pub fn disconnect(&mut self) -> Result<(), CameraState> {
        replace_with_or_abort_and_return(&mut self.state, |state| match state {
            InternalState::Connected { thread, frame_rx } => {
                self.atomics
                    .should_stop
                    .store(true, atomic::Ordering::Relaxed);

                // purge frame_rx by consuming all remaining elements
                let count = frame_rx.recv().iter().count();
                trace!("purged {count} frames from receive queue");

                // reclaim our injected camera implementation handler from thread
                let handler = thread.join().expect("receive thread has panicked");
                trace!("reclaimed handler from thread");

                (Ok(()), InternalState::Waiting(handler))
            }
            state => (
                Err(CameraState::Error(format!(
                    "Cannot disconnect during this state: {state:?}"
                ))),
                state,
            ),
        })
    }

    /// Returns the current frame rate of the camera
    pub fn frame_rate(&self) -> u16 {
        self.atomics.frame_rate.load(atomic::Ordering::Relaxed)
    }

    /// Returns the target frame rate of the camera
    pub fn target_frame_rate(&self) -> u16 {
        self.atomics
            .target_frame_rate
            .load(atomic::Ordering::Relaxed)
    }

    /// Set the target frame rate the camera should attempt to achieve
    pub fn set_target_frame_rate(&self, target_frame_rate: u16) {
        self.atomics
            .target_frame_rate
            .store(target_frame_rate, atomic::Ordering::Relaxed)
    }
}

pub fn handler_recv(
    mut handler: BoxedHandler,
    frame_tx: mpsc::SyncSender<Vec<u8>>,
    atomics: Arc<Atomics>,
) -> std::thread::JoinHandle<BoxedHandler> {
    std::thread::spawn(move || {
        loop {
            if atomics.should_stop.load(atomic::Ordering::Relaxed) {
                break;
            }

            let target_fps = atomics.target_frame_rate.load(atomic::Ordering::Relaxed) as u64;
            let frame_time = std::time::Instant::now();

            let frame = match handler.get_frame() {
                Ok(frame) => frame,
                Err(_) => {
                    // TODO: proper error handling
                    // error!("failed to fetch frame: {e:?}");
                    continue;
                }
            };

            if let Err(e) = frame_tx.send(frame) {
                // todo: abort thread here? this state should never be hit
                error!("failed to send frame to internal channel: {e:?}");
            }

            // sleep for consistent FPS
            let target_duration = std::time::Duration::from_millis(1000 / target_fps);
            if frame_time.elapsed() < target_duration {
                std::thread::sleep(target_duration - frame_time.elapsed());
            }

            let elapsed = frame_time.elapsed().as_millis().max(1);
            debug!("{} fps, elapsed {} ms", 1000 / elapsed, elapsed);

            atomics
                .frame_rate
                .store((1000 / elapsed) as u16, atomic::Ordering::Relaxed);
        }

        handler
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transitions() {
        let handler = CameraHandlers::NoOp;
        let source: String = "COM13".into();

        // camera was created and is waiting for a connection
        let mut camera = Camera::new(handler, 30);
        assert!(matches!(camera.state, InternalState::Waiting(..)));

        // camera is in connect state with valid thread handle
        camera
            .connect(source)
            .expect("no-op connect should always succeed");
        assert!(matches!(camera.state, InternalState::Connected { .. }));
        let InternalState::Connected { ref thread, .. } = camera.state else {
            unreachable!()
        };
        assert!(!thread.is_finished());

        // disconnect should move camera into waiting state
        camera
            .disconnect()
            .expect("disconnect should always succeed");
        assert!(matches!(camera.state, InternalState::Waiting(..)))
    }
}
