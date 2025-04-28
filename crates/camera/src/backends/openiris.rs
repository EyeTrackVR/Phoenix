use std::{io, time::Duration};

use log::{error, info, trace, warn};
use nom::{IResult, Parser, bytes, number, sequence};
use serialport::{FlowControl, SerialPort};

use crate::{CameraHandler, CameraState};

// Serial communication protocol:
// header-begin (2 bytes)
// header-type (2 bytes)
// packet-size (2 bytes)
// packet (packet-size bytes)
// https://github.com/EyeTrackVR/OpenIris/blob/5da262c8daf27ea2cb060ec2e41a19a1c1c3db29/ESP/lib/src/io/Serial/SerialManager.cpp#L31

// header + header type
const ETVR_HEADER_FRAME: &[u8] = &[0xFF, 0xA0, 0xFF, 0xA1];
const BAUD_RATE: u32 = if cfg!(target_os = "macos") {
    // as per EyeTrackVR python: higher baud rate not working on macOS
    115_200
} else {
    3_000_000
};

type SerialPortVariant = Box<dyn SerialPort>;

#[derive(Debug)]
pub struct OpenIrisCamera {
    port: Option<SerialPortVariant>,
}

impl CameraHandler for OpenIrisCamera {
    fn init() -> Self
    where
        Self: Sized,
    {
        Self { port: None }
    }

    fn get_frame(&mut self) -> Result<Vec<u8>, CameraState> {
        if let Some(port) = self.port.as_mut() {
            let mut frame = get_frame(port)?;
            Ok(std::mem::take(&mut frame))
        } else {
            Err(CameraState::Disconnected)
        }
    }

    fn connect(&mut self, source: String) -> Result<(), CameraState> {
        if self.port.is_none() {
            let source = source.as_str();
            info!("connecting to {source}");

            // todo: figure out why the baud rate gets reset to 9600
            match serialport::new(source, BAUD_RATE)
                .timeout(Duration::from_millis(100))
                .flow_control(FlowControl::None)
                .open()
            {
                Ok(port) => {
                    info!("connected to serial port {source}");
                    dbg!(&port);
                    self.port = Some(port);

                    Ok(())
                }
                Err(e) => Err(CameraState::Error(format!("Failed to open port: {e}"))),
            }
        } else {
            Err(CameraState::Connected)
        }
    }

    fn disconnect(&mut self) {
        // port is closed once object is dropped
        if self.port.is_some() {
            self.port = None;
        }
    }
}

fn get_frame(port: &mut SerialPortVariant) -> Result<Vec<u8>, CameraState> {
    // should always be lower than a full jpeg frame
    let peek_buf_size = 256;
    let leftover_threshold = 8192;

    // peek into stream and retrieve header
    let input: Result<(Vec<u8>, u16), _> = loop {
        let data = match get_data(port.as_mut(), peek_buf_size) {
            Ok(data) => data,
            Err(e) => {
                break Err(format!(
                    "failed to read bytes from serial port buffer: {e:?}"
                ));
            }
        };

        trace!("read {} bytes during peek", data.len());

        // look for ~6 bytes in buffer, hopefully finding a match
        match parse_next_header(&data) {
            Ok((input, len_data)) => break Ok((Vec::from(input), len_data)),
            Err(nom::Err::Incomplete(needed)) => {
                trace!("need to peek further into data stream: {needed:?}");
                continue;
            }
            Err(nom::Err::Error(data)) | Err(nom::Err::Failure(data)) => {
                error!("failed to read data stream: {:?}", data.code);
                break Err(format!("failed to parse buffer via nom: {:?}", data.code));
            }
        };
    };

    // unpack remaining input and expected length of jpeg packet
    let (input, len_data) = match input {
        Ok(i) => i,
        Err(e) => {
            return Err(CameraState::Error(e.to_string()));
        }
    };

    // fetch missing bytes for full packet
    let missing_bytes = ((len_data as usize) - input.len()).max(0);
    let input = if missing_bytes > 0 {
        let missing = match get_data(port.as_mut(), missing_bytes) {
            Ok(data) => data,
            Err(e) => {
                return Err(CameraState::Error(format!(
                    "failed to read remaining bytes from serial port buffer: {e:?}"
                )));
            }
        };
        [input, missing].concat()
    } else {
        input
    };

    let result = match parse_jpeg_data(&input, len_data) {
        Ok((_remaining, result)) => {
            // todo: validate JPEG data? corrupted frames are currently possible
            // image = { version = "0.25.6" }
            // image::load_from_memory_with_format(result, image::ImageFormat::Jpeg)

            Ok(Vec::from(result))
        }
        Err(nom::Err::Incomplete(needed)) => {
            warn!("incomplete jpeg frame despite reading appropriate length: {needed:?}");
            Err(CameraState::Error("incomplete jpeg frame".to_string()))
        }
        Err(nom::Err::Error(data)) | Err(nom::Err::Failure(data)) => {
            error!("failed to read data stream: {:?}", data.code);

            Err(CameraState::Error(format!(
                "failed to read data stream: {:?}",
                data.code
            )))
        }
    };

    // drop leftover data to keep in sync with reality
    match port.bytes_to_read() {
        Ok(leftover) if leftover > leftover_threshold as u32 => {
            trace!("dropping leftover bytes: {leftover}");
            if let Err(e) = port.clear(serialport::ClearBuffer::All) {
                error!("failed to drop leftover bytes: {e:?}");
            }
        }
        Err(e) => {
            error!("failed to query leftover bytes in serial port: {e:?}");
        }
        _ => {}
    }
    result
}

fn get_data(port: &mut dyn SerialPort, buf_size: usize) -> io::Result<Vec<u8>> {
    let mut buf = vec![0u8; buf_size];
    port.read_exact(&mut buf)?;

    Ok(buf)
}

fn parse_next_header(input: &[u8]) -> IResult<&[u8], u16> {
    let (input, _preamble) = bytes::streaming::take_until(ETVR_HEADER_FRAME).parse(input)?;

    sequence::preceded(
        bytes::streaming::tag(ETVR_HEADER_FRAME),
        number::streaming::le_u16,
    )
    .parse(input)
}

fn parse_jpeg_data(input: &[u8], len_data: u16) -> IResult<&[u8], &[u8]> {
    bytes::streaming::take(len_data).parse_complete(input)
}

// todo: proper impls
unsafe impl Send for OpenIrisCamera {}
unsafe impl Sync for OpenIrisCamera {}
