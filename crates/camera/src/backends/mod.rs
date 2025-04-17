mod noop;
mod opencv;
mod openiris;

pub use noop::NoOpCamera;
pub use opencv::OpenCVCamera;
pub use openiris::OpenIrisCamera;

pub enum CameraHandlers {
    NoOp,
    OpenCV,
    OpenIris,
}
