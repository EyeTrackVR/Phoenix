use crate::*;

pub struct NoOpCamera {}

unsafe impl Send for NoOpCamera {}
unsafe impl Sync for NoOpCamera {}

impl CameraHandler for NoOpCamera {
    fn disconnect(&mut self) {}

    fn init() -> Self where Self: Sized {return Self {}}

    fn get_frame(&mut self) -> Result<Vec<u8>, CameraState> {return Ok(vec![])}

    fn connect(&mut self, _source: String) -> Result<(), CameraState> {return Ok(())}
}
