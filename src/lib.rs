extern crate futures;
extern crate tokio_core;
#[macro_use]
extern crate json;

use futures::{Future, Stream};
use tokio_core::io::{Io, Codec, EasyBuf, Framed};
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use std::{fmt, io, str};
use std::net::SocketAddr;

pub struct Client;

pub struct SbtCodec;

pub enum CommandMessage {
    ExecCommand {
        command_line: String,
        exec_id: Option<String>,
    },
}

pub enum EventMessage {
    ChannelAcceptedEvent { channel_name: String },
    LogEvent {
        level: String,
        message: String,
        channel_name: Option<String>,
        exec_id: Option<String>,
    },
    ExecStatusEvent {
        status: String,
        channel_name: Option<String>,
        exec_id: Option<String>,
        command_queue: Vec<String>,
    },
}

impl fmt::Display for EventMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &EventMessage::ChannelAcceptedEvent { ref channel_name } => {
                write!(f, "Bound to channel {}", channel_name)
            }
            &EventMessage::LogEvent { ref level, ref message, ref channel_name, ref exec_id } => {
                write!(f,
                       "[{}] {} ({}, {})",
                       level,
                       message,
                       channel_name.clone().unwrap_or("none".to_string()),
                       exec_id.clone().unwrap_or("none".to_string()))
            }
            &EventMessage::ExecStatusEvent { ref status,
                                             ref channel_name,
                                             ref exec_id,
                                             ref command_queue } => {
                write!(f,
                       "[exec event] {} ({}, {})",
                       status,
                       channel_name.clone().unwrap_or("none".to_string()),
                       exec_id.clone().unwrap_or("none".to_string()))
            }
        }
    }
}

impl Client {
    pub fn connect(addr: &SocketAddr,
                   handle: Handle)
                   -> Box<Future<Item = Framed<TcpStream, SbtCodec>, Error = io::Error>> {
        let transport = TcpStream::connect(addr, &handle).and_then(|socket| {
            let transport = socket.framed(SbtCodec);

            let handshake = transport.into_future()
                .map_err(|(e, _)| e)
                .and_then(|(msg, transport)| match msg {
                    Some(EventMessage::ChannelAcceptedEvent { channel_name }) => {
                        println!("Server accepted our channel {}", channel_name);
                        Ok(transport)
                    }
                    _ => {
                        println!("Server handshake invalid!");
                        let err = io::Error::new(io::ErrorKind::Other, "invalid handshake");
                        Err(err)
                    }
                });

            handshake
        });


        Box::new(transport)
    }
}

impl Codec for SbtCodec {
    type In = EventMessage;
    type Out = CommandMessage;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<EventMessage>, io::Error> {
        if let Some(n) = buf.as_ref().iter().position(|b| *b == b'\n') {
            let line = buf.drain_to(n);

            buf.drain_to(1);

            let raw = match str::from_utf8(&line.as_ref()) {
                Ok(s) => s.to_string(),
                Err(_) => return Err(io::Error::new(io::ErrorKind::Other, "invalid UTF8 string")),
            };

            // println!("Decoding {}", raw);

            return match json::parse(&raw) {
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "invalid json")),
                Ok(js) => {
                    if js["type"].is_null() {
                        Err(io::Error::new(io::ErrorKind::Other, "invalid json"))
                    } else {
                        match js["type"].as_str() {
                            Some("ExecStatusEvent") => {
                                Ok(Some(EventMessage::ExecStatusEvent {
                                    status: js["status"]
                                        .as_str()
                                        .expect("missing status")
                                        .to_string(),
                                    channel_name: js["channelName"].as_str().map(|s| s.to_string()),
                                    exec_id: js["execId"].as_str().map(|s| s.to_string()),
                                    command_queue: js["commandQueue"]
                                        .members()
                                        .map(|j| j.as_str().unwrap().to_string())
                                        .collect(),
                                }))
                            }
                            Some("ChannelLogEntry") => {
                                Ok(Some(EventMessage::LogEvent {
                                    level: js["level"].as_str().expect("missing level").to_string(),
                                    message: js["message"]
                                        .as_str()
                                        .expect("missing message")
                                        .to_string(),
                                    channel_name: js["channelName"].as_str().map(|s| s.to_string()),
                                    exec_id: js["execId"].as_str().map(|s| s.to_string()),
                                }))
                            }
                            Some("ChannelAcceptedEvent") => {
                                match js["channelName"].as_str() {
                                    Some(channel_name) => {
                                        Ok(Some(EventMessage::ChannelAcceptedEvent {
                                            channel_name: channel_name.to_string(),
                                        }))
                                    }
                                    _ => {
                                        Err(io::Error::new(io::ErrorKind::Other,
                                                           "channel not found"))
                                    }
                                }
                            }
                            _ => Err(io::Error::new(io::ErrorKind::Other, "invalid json")),
                        }
                    }
                }
            };
        }

        Ok(None)
    }

    fn encode(&mut self, msg: CommandMessage, buf: &mut Vec<u8>) -> io::Result<()> {
        // buf.extend_from_slice(msg.dump().as_bytes());
        match msg {
            CommandMessage::ExecCommand { command_line, exec_id } => {
                let msg = object! {
                 "type" => "ExecCommand",
                 "commandLine" => command_line
             };
                msg.write(buf);
                buf.push(b'\n');
                Ok(())
            }
        }
    }
}
