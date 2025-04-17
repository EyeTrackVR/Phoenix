use std::sync::Arc;
use std::sync::Mutex;
use crate::{CameraHandler, CameraHandlers, CameraState};

pub struct Camera {
    frame: Arc<Mutex<Vec<u8>>>,
    frame_rate: Arc<Mutex<u16>>,
    should_stop: Arc<Mutex<bool>>,
    target_frame_rate: Arc<Mutex<u16>>,
    thread: std::thread::JoinHandle<()>,
    camera_state: Arc<Mutex<CameraState>>,
    handler: Arc<Mutex<Box<dyn CameraHandler>>>,
}

impl Camera {
    pub fn new(handler: CameraHandlers, frame_rate: u16) -> Camera {
        use crate::backends::*;

        #[allow(unreachable_patterns)]
        let backend: Box<dyn CameraHandler> = match handler {
            CameraHandlers::NoOp => {
                Box::new(NoOpCamera::init())
            }
            CameraHandlers::OpenCV => {
                Box::new(OpenCVCamera::init())
            }
            CameraHandlers::OpenIris => {
                todo!();
                // Box::new(OpenIrisCamera::init())
            }
            _ => {
                panic!("Unsupported camera handler");
            }
        };

        return Self::from_camera_handler(backend, frame_rate);
    }

    pub fn from_camera_handler(handler: Box<dyn CameraHandler>, target_frame_rate: u16) -> Camera {
        let frame = Arc::new(Mutex::new(vec![]));
        let handler = Arc::new(Mutex::new(handler));
        let should_stop = Arc::new(Mutex::new(false));
        let frame_rate = Arc::new(Mutex::new(u16::MIN));
        let target_frame_rate = Arc::new(Mutex::new(target_frame_rate));
        let camera_state = Arc::new(Mutex::new(CameraState::Disconnected));

        let thread = {
            // Arc doesn't impl copy
            let frame = frame.clone();
            let handler = handler.clone();
            let frame_rate = frame_rate.clone();
            let should_stop = should_stop.clone();
            let _camera_state = camera_state.clone();
            let target_frame_rate = target_frame_rate.clone();
            std::thread::spawn(move || {
                loop {
                    // Uhm might maybe max out the CPU's thread
                    if *(should_stop.lock().unwrap()) {continue}

                    let mut frame = frame.lock().unwrap();
                    let mut handler = handler.lock().unwrap();
                    let frame_time = std::time::Instant::now();
                    let mut frame_rate = frame_rate.lock().unwrap();
                    let mut _camera_state = _camera_state.lock().unwrap();
                    let target_frame_rate = target_frame_rate.lock().unwrap();

                    *frame = match handler.get_frame() {
                        Ok(frame) => {frame}

                        // TODO: proper error handling
                        Err(_) => {continue}
                    };

                    *frame_rate = (1000 / frame_time.elapsed().as_millis().max(1)) as u16;
                    let target_duration = std::time::Duration::from_millis(1000 / *target_frame_rate as u64);
                    if frame_time.elapsed() < target_duration {
                        std::thread::sleep(target_duration - frame_time.elapsed());
                    }
                }
            })
        };

        return Self {
            frame: frame,
            thread: thread,
            handler: handler,
            frame_rate: frame_rate,
            should_stop: should_stop,
            camera_state: camera_state,
            target_frame_rate: target_frame_rate,
        };
    }

    /// Retrieves the current most recent frame from the camera
    /// - Returns an error if the camera is not connected
    pub fn get_frame(&self) -> Result<Vec<u8>, CameraState> {
        if *(self.camera_state.lock().unwrap()) != CameraState::Connected {
            return Err(self.camera_state.lock().unwrap().clone());
        }

        // drain to avoid an extra unecessary clone of the data
        return Ok(self.frame.lock().unwrap().drain(..).collect());
    }

    pub fn connect(&self, source: String) -> Result<(), CameraState> {
        let mut handler = self.handler.lock().unwrap();
        let mut camera_state = self.camera_state.lock().unwrap();
        *camera_state = CameraState::Connecting;

        match handler.connect(source) {
            Ok(_) => {
                *camera_state = CameraState::Connected;
                return Ok(());
            }
            Err(e) => {
                *camera_state = e.clone();
                return Err(e);
            }
        }
    }

    /// Disconnects the camera
    pub fn disconnect(&self) {
        *self.should_stop.lock().unwrap() = true;
        self.handler.lock().unwrap().disconnect();
    }

    /// Returns the current frame rate of the camera
    pub fn frame_rate(&self) -> u16 {
        return *self.frame_rate.lock().unwrap()
    }

    /// Returns the target frame rate of the camera
    pub fn target_frame_rate(&self) -> u16 {
        return *self.target_frame_rate.lock().unwrap()
    }

    /// Set the target frame rate the camera should attempt to achieve
    pub fn set_target_frame_rate(&self, target_frame_rate: u16) {
        *self.target_frame_rate.lock().unwrap() = target_frame_rate;
    }

    pub fn shutdown(self) {
        self.handler.lock().unwrap().disconnect();
        // This should kill the thread loop...
        self.thread.thread().unpark();
        self.thread.join().unwrap();
    }
}
