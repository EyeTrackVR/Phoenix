// Look i am not happy about this either
// In a perfect world we ditch OpenCV and roll something custom
// But we dont live in a perfect world and dealing with different image coddecs is a huge pain
// Even when writing this for the first time it is already due for a massvice refactor to nuke OpenCV

use crate::*;
use opencv::prelude::*;
use opencv::videoio::{VideoCapture, CAP_FFMPEG};

pub struct OpenCVCamera {
    capture: VideoCapture,
    capture_source: String,
}

unsafe impl Send for OpenCVCamera {}
unsafe impl Sync for OpenCVCamera {}

impl CameraHandler for OpenCVCamera {
    fn init() -> Self where Self: Sized {
        return Self {
            capture_source: String::new(),
            capture: VideoCapture::default().unwrap(),
        };
    }

    fn get_frame(&mut self) -> Result<Vec<u8>, CameraState> {
        // FIXME: make sure the camera is open
        let mut mat = Mat::default();
        match self.capture.read(&mut mat) {
            Ok(true) => {
                if mat.size().unwrap().width > 0 && mat.size().unwrap().height > 0 {
                    return Ok(mat.data_bytes().unwrap().to_vec());
                }
                return Err(CameraState::ReadFailed);
            }
            Ok(false) => {return Err(CameraState::ReadFailed)}
            Err(e) => {return Err(CameraState::Error(format!("Failed to read frame: {}", e)))}
        }
    }

    fn connect(&mut self, source: String) -> Result<(), CameraState> {
        if self.capture_source == source {
            log::trace!("Skipping connection, already connected to {}", source);
            return Ok(());
        }

        // TODO: Ping the host to see if it is alive
        self.capture_source = source.clone();
        match self.capture.open_file(&source, CAP_FFMPEG) {
            Ok(state) => {
                if state {return Ok(())};
                match self.capture.is_opened() {
                    Ok(true) => {return Ok(())}
                    Ok(false) => {return Err(CameraState::Timeout)}
                    Err(e) => {return Err(CameraState::Error(format!("Failed to open camera: {}", e)))}
                }
            }
            Err(e) => {return Err(CameraState::Error(format!("Failed to open camera: {}", e)))}
        }
    }

    fn disconnect(&mut self) {
        self.capture.release().unwrap();
    }
}
