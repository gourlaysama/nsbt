extern crate bytes;
extern crate futures;
#[macro_use]
extern crate json;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_proto;
extern crate tokio_service;

pub mod messages;

use std::io;
use std::net::SocketAddr;

use bytes::{BufMut, BytesMut};
use futures::{Future, Poll, Stream};
use messages::*;
//use tokio_core::io::{Io, Codec, EasyBuf, Framed};
use tokio_core::reactor::Handle;
use tokio_io::{AsyncRead, AsyncWrite};
use tokio_io::codec::{Decoder, Encoder, Framed};
use tokio_proto::streaming::{Body, Message};
use tokio_proto::streaming::pipeline::{ClientProto, Frame};
use tokio_proto::TcpClient;
use tokio_proto::util::client_proxy::ClientProxy;
use tokio_service::Service;

pub struct Client {
    inner: ClientProxy<
        Message<CommandMessage, Body<(), io::Error>>,
        Message<EventMessage, Body<EventMessage, io::Error>>,
        io::Error,
    >,
}

pub struct SbtCodec {
    current_exec_id: Option<String>,
    channel_name: Option<String>,
}

pub struct SbtProto;

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
    pub fn connect(
        addr: &SocketAddr,
        handle: &Handle,
    ) -> Box<Future<Item = Client, Error = io::Error>> {
        let ret = TcpClient::new(SbtProto)
            .connect(addr, handle)
            .map(|client_proxy| Client {
                inner: client_proxy,
            });

        Box::new(ret)
    }
}

impl Decoder for SbtCodec {
    // type In = Frame<EventMessage, EventMessage, io::Error>;
    // type Out = Frame<CommandMessage, (), io::Error>;
    type Item = Frame<EventMessage, EventMessage, io::Error>;
    type Error = io::Error;

    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<Self::Item>, io::Error> {
        if let Some(n) = buf.as_ref().iter().position(|b| *b == b'\n') {
            let line = buf.split_to(n);

            buf.split_to(1);

            return match serde_json::from_slice::<EventMessage>(&line.as_ref()) {
                Err(_) => {
                    debug!("Received wrong message:");
                    debug!("{:?}", line);
                    Err(io::Error::new(io::ErrorKind::Other, "invalid json"))
                }
                Ok(ev) => {
                    match ev {
                        EventMessage::ExecStatusEvent {
                            ref status,
                            ref channel_name,
                            ref exec_id,
                            ..
                        } => {
                            // TODO: those 'clone' shouldn't be necessary
                            let ev = ev.clone();
                            if self.channel_name != *channel_name {
                                Ok(None)
                            } else {
                                match exec_id {
                                    e @ &Some(_) => {
                                        if status == "Processing" {
                                            // we receive a Processing, this means the start
                                            // of a new response from sbt
                                            self.current_exec_id = e.clone();
                                            Ok(Some(Frame::Message {
                                                message: ev,
                                                body: true,
                                            }))
                                        } else if self.current_exec_id == *e {
                                            if status == "Done" {
                                                // end of the server response
                                                self.current_exec_id = None;
                                                Ok(Some(Frame::Body { chunk: None }))
                                            } else {
                                                // some unknown status event, let's just move
                                                // it along, it doesn't terminate the response
                                                Ok(Some(Frame::Body { chunk: Some(ev) }))
                                            }
                                        } else {
                                            Ok(None)
                                        }
                                    }
                                    &None => Ok(None),
                                }
                            }
                        }
                        EventMessage::StringEvent {
                            ref channel_name,
                            ref exec_id,
                            ..
                        } => {
                            let ev = ev.clone();
                            if *channel_name != self.channel_name {
                                // a log entry that doesn't belong to our channel
                                // let's ignore it
                                debug!("Received unknown log entry: {:?}", ev);
                                Ok(None)
                            } else {
                                if self.current_exec_id.is_none() {
                                    // a log entry that isn't associated with the current
                                    // response, but is from us? Souldn't happen, but...
                                    Ok(Some(Frame::Message {
                                        message: ev,
                                        body: false,
                                    }))
                                } else if self.current_exec_id == *exec_id {
                                    Ok(Some(Frame::Body { chunk: Some(ev) }))
                                } else {
                                    // a log entry for us but from the wrong request?
                                    // let's ignore it
                                    Ok(None)
                                }
                            }
                        }
                        EventMessage::ChannelAcceptedEvent { ref channel_name } => {
                            let ev = ev.clone();
                            self.channel_name = Some(channel_name.clone());
                            Ok(Some(Frame::Message {
                                message: ev,
                                body: false,
                            }))
                        }
                    }
                }
            };
        }

        Ok(None)
    }
}
impl Encoder for SbtCodec {
    type Item = Frame<CommandMessage, (), io::Error>;
    type Error = io::Error;

    fn encode(&mut self, msg: Self::Item, buf: &mut BytesMut) -> io::Result<()> {
        match msg {
            Frame::Message {
                message:
                    CommandMessage::ExecCommand {
                        command_line,
                        exec_id,
                    },
                body: false,
            } => {
                let mut msg = object! {
                 "type" => "ExecCommand",
                 "commandLine" => command_line
                };
                if let Some(e) = exec_id {
                    msg["execId"] = e.as_str().into();
                    self.current_exec_id = Some(e);
                }
                //msg.write(buf).unwrap();
                buf.put(msg.dump());
                buf.put(b'\n');
                Ok(())
            }
            _ => Err(io::Error::new(
                io::ErrorKind::Other,
                "no streaming allowed for requests",
            )),
        }
    }
}

impl<T: AsyncRead + AsyncWrite + 'static> ClientProto<T> for SbtProto {
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

        let handshake = transport
            .into_future()
            .map_err(|(e, _)| e)
            .and_then(|(msg, transport)| match msg {
                Some(Frame::Message {
                    message: EventMessage::ChannelAcceptedEvent { .. },
                    body: false,
                }) => Ok(transport),
                _ => Err(io::Error::new(io::ErrorKind::Other, "invalid handshake")),
            });

        Box::new(handshake)
    }
}

impl Service for Client {
    type Request = CommandMessage;
    type Response = EventStream;
    type Error = io::Error;
    type Future = Box<Future<Item = EventStream, Error = io::Error>>;

    fn call(&self, req: CommandMessage) -> Self::Future {
        let into = self.inner.call(req.into()).map_err(|e| {
            io::Error::new(
                io::ErrorKind::Other,
                format!("Unable to send command ({})", e),
            )
        });

        Box::new(into.map(EventStream::from))
    }
}
