use serialport::SerialPort;

pub struct OpenIrisCamera {
    pub port: Box<dyn SerialPort>,
}
