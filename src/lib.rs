extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;
#[macro_use]
extern crate json;

use std::{fmt, io, str};
use std::net::SocketAddr;

use json::JsonValue;
use futures::{Future, Poll, Stream};
use tokio_core::io::{Io, Codec, EasyBuf, Framed};
use tokio_core::reactor::Handle;
use tokio_proto::streaming::{Body, Message};
use tokio_proto::streaming::pipeline::{ClientProto, Frame};
use tokio_proto::TcpClient;
use tokio_proto::util::client_proxy::ClientProxy;
use tokio_service::Service;

pub struct Client {
    inner: ClientProxy<Message<CommandMessage, Body<(), io::Error>>,
                       Message<EventMessage, Body<EventMessage, io::Error>>,
                       io::Error>,
}

pub struct SbtCodec {
    current_exec_id: Option<String>,
    channel_name: Option<String>,
}

pub struct SbtProto;

#[derive(Debug)]
pub enum CommandMessage {
    ExecCommand {
        command_line: String,
        exec_id: Option<String>,
    },
}

#[derive(Debug)]
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
            &EventMessage::LogEvent { ref level, ref message, .. } => {
                write!(f, "[{}] {}", level, message)
            }
            &EventMessage::ExecStatusEvent { ref status, .. } => {
                write!(f, "[exec event] {}", status)
            }
        }
    }
}

pub struct EventStream {
    inner: Body<EventMessage, io::Error>,
}

impl Stream for EventStream {
    type Item = EventMessage;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<EventMessage>, io::Error> {
        self.inner.poll()
    }
}

impl From<Message<EventMessage, Body<EventMessage, io::Error>>> for EventStream {
    fn from(src: Message<EventMessage, Body<EventMessage, io::Error>>) -> Self {
        match src {
            Message::WithBody(_, body) => EventStream { inner: body },
            Message::WithoutBody(_) => unimplemented!(),
        }
    }
}

impl From<CommandMessage> for Message<CommandMessage, Body<(), io::Error>> {
    fn from(src: CommandMessage) -> Self {
        // there is no streaming of anything to the server
        // all commands are without bodies
        Message::WithoutBody(src)
    }
}

impl Client {
    pub fn connect(addr: &SocketAddr,
                   handle: &Handle)
                   -> Box<Future<Item = Client, Error = io::Error>> {
        let ret = TcpClient::new(SbtProto)
            .connect(addr, handle)
            .map(|client_proxy| Client { inner: client_proxy });

        Box::new(ret)
    }
}

impl Codec for SbtCodec {
    type In = Frame<EventMessage, EventMessage, io::Error>;
    type Out = Frame<CommandMessage, (), io::Error>;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<Self::In>, io::Error> {
        if let Some(n) = buf.as_ref().iter().position(|b| *b == b'\n') {
            let line = buf.drain_to(n);

            buf.drain_to(1);

            return match parse_json(&line.as_ref()) {
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "invalid json")),
                Ok(js) => {
                    if js["type"].is_null() {
                        Err(io::Error::new(io::ErrorKind::Other, "invalid json"))
                    } else {
                        match js["type"].as_str() {
                            Some("ExecStatusEvent") => {
                                let channel_name =
                                    js["channelName"].as_str().map(|s| s.to_string());
                                let status = js["status"].as_str().expect("missing status");
                                let exec_id = js["execId"].as_str().map(|s| s.to_string());
                                if channel_name != self.channel_name {
                                    // wrong channel, what is this message even doing here?
                                    // let's just ignore it
                                    Ok(None)
                                } else {
                                    match exec_id {
                                        e @ Some(_) => {
                                            if status == "Processing" {
                                                // we receive a Processing, this means the start
                                                // of a new response from sbt
                                                self.current_exec_id = e.clone();
                                                Ok(Some(Frame::Message {
                                                   message: EventMessage::ExecStatusEvent {
                                                       status: status.to_string(),
                                                       channel_name: js["channelName"]
                                                           .as_str()
                                                           .map(|s| s.to_string()),
                                                       exec_id: e,
                                                       command_queue: js["commandQueue"]
                                                           .members()
                                                           .map(|j| {
                                                               j.as_str().unwrap().to_string()
                                                           })
                                                           .collect(),
                                                   },
                                                   body: true,
                                               }))
                                            } else if self.current_exec_id == e {
                                                if status == "Done" {
                                                    // end of the server response
                                                    self.current_exec_id = None;
                                                    Ok(Some(Frame::Body { chunk: None }))
                                                } else {
                                                    // some unknown status event, let's just move
                                                    // it along, it doesn't terminate the response
                                                    Ok(Some(Frame::Body {
                                                        chunk:
                                                            Some(EventMessage::ExecStatusEvent {
                                                            status: status.to_string(),
                                                            channel_name: channel_name,
                                                            exec_id: e,
                                                            command_queue: js["commandQueue"]
                                                                .members()
                                                                .map(|j| {
                                                                    j.as_str().unwrap().to_string()
                                                                })
                                                                .collect(),
                                                        }),
                                                    }))
                                                }
                                            } else {
                                                Ok(None)
                                            }
                                        }
                                        None => Ok(None),
                                    }
                                }
                            }
                            Some("ChannelLogEntry") => {
                                // some log event
                                let channel_name =
                                    js["channelName"].as_str().map(|s| s.to_string());
                                let exec_id = js["execId"].as_str().map(|s| s.to_string());

                                if channel_name != self.channel_name {
                                    // a log entry that doesn't belong to our channel
                                    // let's ignore it
                                    Ok(None)
                                } else {
                                    let event = EventMessage::LogEvent {
                                        level: js["level"]
                                            .as_str()
                                            .expect("missing level")
                                            .to_string(),
                                        message: js["message"]
                                            .as_str()
                                            .unwrap_or("")
                                            .to_string(),
                                        channel_name: channel_name,
                                        exec_id: exec_id.clone(),
                                    };

                                    if self.current_exec_id.is_none() {
                                        // a log entry that isn't associated with the current
                                        // response, but is from us? Souldn't happen, but...
                                        Ok(Some(Frame::Message {
                                            message: event,
                                            body: false,
                                        }))
                                    } else if self.current_exec_id == exec_id {
                                        Ok(Some(Frame::Body { chunk: Some(event) }))
                                    } else {
                                        // a log entry for us but from the wrong request?
                                        // let's ignore it
                                        Ok(None)
                                    }
                                }
                            }
                            Some("ChannelAcceptedEvent") => {
                                match js["channelName"].as_str() {
                                    Some(channel_name) => {
                                        self.channel_name = Some(channel_name.to_string());
                                        Ok(Some(Frame::Message {
                                            message: EventMessage::ChannelAcceptedEvent {
                                                channel_name: channel_name.to_string(),
                                            },
                                            body: false,
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

    fn encode(&mut self, msg: Self::Out, buf: &mut Vec<u8>) -> io::Result<()> {
        match msg {
            Frame::Message { message: CommandMessage::ExecCommand { command_line, exec_id },
                             body: false } => {
                let mut msg = object! {
                 "type" => "ExecCommand",
                 "commandLine" => command_line
                };
                if let Some(e) = exec_id {
                    msg["execId"] = e.as_str().into();
                    self.current_exec_id = Some(e);
                }
                msg.write(buf).unwrap();
                buf.push(b'\n');
                Ok(())
            }
            _ => Err(io::Error::new(io::ErrorKind::Other, "no streaming allowed for requests")),
        }
    }
}

impl<T: Io + 'static> ClientProto<T> for SbtProto {
    type Request = CommandMessage;
    type RequestBody = ();
    type Response = EventMessage;
    type ResponseBody = EventMessage;
    type Error = io::Error;

    type Transport = Framed<T, SbtCodec>;
    type BindTransport = Box<Future<Item = Self::Transport, Error = Self::Error>>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        let transport = io.framed(SbtCodec {
            current_exec_id: None,
            channel_name: None,
        });

        let handshake = transport.into_future()
            .map_err(|(e, _)| e)
            .and_then(|(msg, transport)| match msg {
                Some(Frame::Message { message: EventMessage::ChannelAcceptedEvent { .. },
                                      body: false }) => Ok(transport),
                _ => Err(io::Error::new(io::ErrorKind::Other, "invalid handshake")),
            });

        Box::new(handshake)
    }
}

fn parse_json(buf: &[u8]) -> Result<JsonValue, io::Error> {
    let raw = match str::from_utf8(buf) {
        Ok(s) => Ok(s.to_string()),
        Err(_) => Err(io::Error::new(io::ErrorKind::Other, "invalid UTF8 string")),
    };

    raw.and_then(|s| {
        json::parse(&s).map_err(|_| io::Error::new(io::ErrorKind::Other, "invalid json"))
    })
}

impl Service for Client {
    type Request = CommandMessage;
    type Response = EventStream;
    type Error = io::Error;
    type Future = Box<Future<Item = EventStream, Error = io::Error>>;

    fn call(&self, req: CommandMessage) -> Self::Future {
        let into = self.inner
            .call(req.into())
            .map_err(|e| {
                io::Error::new(io::ErrorKind::Other,
                               format!("Unable to send command ({})", e))
            });

        Box::new(into.map(EventStream::from))
    }
}
