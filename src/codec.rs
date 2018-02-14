use std::io;
use std::str;

use bytes::{BufMut, BytesMut};
use tokio_io::codec::{Decoder, Encoder};

#[derive(Debug)]
pub struct JsonRpcFramingCodec {
    length_to_decode: Option<usize>,
    parsing_payload: bool,
}

fn utf8(buf: &[u8]) -> Result<&str, io::Error> {
    str::from_utf8(buf)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Unable to decode input as UTF8"))
}

impl JsonRpcFramingCodec {
    pub fn new() -> JsonRpcFramingCodec {
        JsonRpcFramingCodec {
            length_to_decode: None,
            parsing_payload: false,
        }
    }
    fn split_line(&self, buf: &mut BytesMut) -> Result<Option<String>, io::Error> {
        if let Some(newline_offset) = buf.iter().position(|b| *b == b'\n') {
            let newline_index = newline_offset;
            let line = buf.split_to(newline_index + 1);
            let line = &line[..line.len() - 2];
            let line = utf8(line)?;
            Ok(Some(line.to_string()))
        } else {
            Ok(None)
        }
    }
}

impl Decoder for JsonRpcFramingCodec {
    type Item = String;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        trace!("Reading buffer of size {}", buf.len());
        if !self.parsing_payload {
            loop {
                let line = self.split_line(buf)?;
                trace!("Reading line : {:?}", line);
                match line {
                    Some(ref txt) if txt.starts_with("Content-Length: ") => {
                        let (_, rest) = txt.split_at(16);
                        self.length_to_decode = match rest.parse() {
                            Ok(l) => Some(l),
                            Err(e) => return Err(io::Error::new(io::ErrorKind::InvalidData, e)),
                        }
                    }
                    Some(ref txt) if txt.starts_with("Content-Type:") => {
                        if self.length_to_decode.is_none() {
                            return Err(io::Error::from(io::ErrorKind::InvalidData));
                        } else {
                            self.parsing_payload = true;
                        }
                    }
                    Some(ref txt) if txt.is_empty() => if self.parsing_payload {
                        break;
                    },
                    None => return Ok(None),
                    _ => return Err(io::Error::from(io::ErrorKind::InvalidData)),
                };
            }
        }

        trace!("State: {:?}", self);
        trace!("Buffer left (s={}): {:?}", buf.len(), buf);

        let json_str = if let Some(length) = self.length_to_decode {
            if buf.len() < length {
                return Ok(None);
            }
            let ss;
            {
                let s = utf8(&buf[..length])?;
                ss = s.to_string();
            }
            buf.split_to(length);
            ss
        } else {
            return Err(io::Error::from(io::ErrorKind::InvalidData));
        };

        self.length_to_decode = None;
        self.parsing_payload = false;

        Ok(Some(json_str))
    }
}

impl Encoder for JsonRpcFramingCodec {
    type Item = String;
    type Error = io::Error;

    fn encode(&mut self, line: String, buf: &mut BytesMut) -> Result<(), io::Error> {
        trace!("Sending line: {:?}", line);
        let length = line.len();
        buf.put(format!("Content-Length: {}\r\n\r\n", length));
        //buf.put("Content-Type: application/vscode-jsonrpc; charset=utf-8\r\n");
        buf.put(line);
        buf.put("\r\n");
        trace!("Buf: {:?}", buf);
        Ok(())
    }
}
