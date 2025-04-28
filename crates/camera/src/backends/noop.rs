use crate::*;

#[derive(Debug)]
pub struct NoOpCamera {}

unsafe impl Send for NoOpCamera {}
unsafe impl Sync for NoOpCamera {}

impl CameraHandler for NoOpCamera {
    fn init() -> Self
    where
        Self: Sized,
    {
        Self {}
    }

    fn get_frame(&mut self) -> Result<Vec<u8>, CameraState> {
        Ok(vec![])
    }

    fn connect(&mut self, _source: String) -> Result<(), CameraState> {
        Ok(())
    }

    fn disconnect(&mut self) {}
}
