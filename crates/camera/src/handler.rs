use std::fmt::Debug;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CameraState {
    Timeout,
    Connected,
    ReadFailed,
    Connecting,
    Disconnected,
    InvalidSource,
    Error(String),
}

/// A trait for camera backends to implement and provide
/// a way to communicate and manage different camera types
/// ## Safety
/// - The handler is expected to be thread safe and implement `Send` + `Sync`
/// - `CameraHandler::Init` is guaranteed to be called before any other methods
///    - The init function will only ever be called once when allocating the
///      handler
pub trait CameraHandler: Send + Sync + Debug {
    /// Initializes the camera backend
    /// - This method is only called a single time at startup
    /// - All variables should be initialized and allocated here The backend
    ///   should not attempt to connect to the camera at this stage
    fn init() -> Self
    where
        Self: Sized;

    // FIXME: this needs a concrete type
    fn get_frame(&mut self) -> Result<Vec<u8>, CameraState>;

    /// Attempts to establish a connection to the camera
    /// - Backends should try to capture a single frame and discard it to ensure
    ///   proper functionality and make sure the camera is working
    fn connect(&mut self, source: String) -> Result<(), CameraState>;

    /// Disconnects the camera and releases any resources
    /// - Disconnecting is meant to be able to be reversed via
    ///   `CameraHandler::connect`
    /// - Release any active connections and free up any resources but dont
    ///   dispose of the handler
    fn disconnect(&mut self);
}
