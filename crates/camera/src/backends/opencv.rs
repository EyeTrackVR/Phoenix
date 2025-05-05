// Look i am not happy about this either
// In a perfect world we ditch OpenCV and roll something custom
// But we dont live in a perfect world and dealing with different image coddecs
// is a huge pain Even when writing this for the first time it is already due
// for a massvice refactor to nuke OpenCV

use opencv::{
    prelude::*,
    videoio::{CAP_FFMPEG, VideoCapture},
};

use crate::*;

#[derive(Debug)]
enum InternalState {
    Waiting,
    Connected {
        capture: VideoCapture,
        source: String,
    },
}

#[derive(Debug)]
pub struct OpenCVCamera {
    state: InternalState,
}

unsafe impl Send for OpenCVCamera {}
unsafe impl Sync for OpenCVCamera {}

impl CameraHandler for OpenCVCamera {
    fn init() -> Self
    where
        Self: Sized,
    {
        Self {
            state: InternalState::Waiting,
        }
    }

    fn get_frame(&mut self) -> Result<Vec<u8>, CameraState> {
        match self.state {
            InternalState::Connected {
                ref mut capture, ..
            } => {
                let mut mat = Mat::default();

                match capture.read(&mut mat) {
                    Ok(true) => {
                        if mat.size().unwrap().width > 0 && mat.size().unwrap().height > 0 {
                            return Ok(mat.data_bytes().unwrap().to_vec());
                        }

                        Err(CameraState::ReadFailed)
                    }
                    Ok(false) => Err(CameraState::ReadFailed),
                    Err(e) => Err(CameraState::Error(format!("Failed to read frame: {}", e))),
                }
            }
            InternalState::Waiting => Err(CameraState::Disconnected),
        }
    }

    fn connect(&mut self, source: String) -> Result<(), CameraState> {
        match self.state {
            InternalState::Connected {
                source: ref current_source,
                ..
            } if current_source == &source => {
                log::trace!("Skipping connection, already connected to {source}");
                Ok(())
            }
            InternalState::Connected { .. } => {
                // todo connect to other source here?
                Ok(())
            }
            InternalState::Waiting => {
                // TODO: Ping the host to see if it is alive

                let mut capture = VideoCapture::default().unwrap();

                if let Err(e) = match capture.open_file(&source, CAP_FFMPEG) {
                    Ok(state) if state => Ok(()),
                    Ok(_) => match capture.is_opened() {
                        // todo: is this branch really necessary?
                        Ok(true) => Ok(()),
                        Ok(false) => Err(CameraState::Timeout),
                        Err(e) => Err(CameraState::Error(format!("Failed to open camera: {e}"))),
                    },
                    Err(e) => Err(CameraState::Error(format!("Failed to open camera: {e}"))),
                } {
                    Err(e)
                } else {
                    self.state = InternalState::Connected { capture, source };

                    Ok(())
                }
            }
        }
    }

    fn disconnect(&mut self) {
        if let InternalState::Connected {
            ref mut capture, ..
        } = self.state
        {
            capture.release().unwrap()
        }
    }
}
