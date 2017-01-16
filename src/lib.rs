extern crate futures;
extern crate tokio_core;
extern crate tokio_proto;
extern crate tokio_service;

use futures::{Future, BoxFuture};
use tokio_core::io::{Io, Codec, EasyBuf, Framed};
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use tokio_proto::TcpClient;
use tokio_proto::pipeline::{ClientProto, ClientService};
use tokio_service::Service;
use std::{io, str};
use std::net::SocketAddr;

pub struct Client {
    inner: ClientService<TcpStream, SbtProto>,
}

pub struct SbtCodec;

struct SbtProto;

impl Client {
    pub fn connect(addr: &SocketAddr,
                   handle: Handle)
                   -> Box<Future<Item = Client, Error = io::Error>> {
        let ret = TcpClient::new(SbtProto)
                      .connect(addr, &handle)
                      .map(|s| Client { inner: s });

        Box::new(ret)
    }
}

impl Service for Client {
  type Request = String;
  type Response = String;
  type Error = io::Error;

    // TODO: boxing Futures, really?
  type Future = BoxFuture<String, io::Error>;

    fn call(&self, cmd: String) -> Self::Future {
        self.inner.call(cmd).boxed()
    }
}

impl Codec for SbtCodec {
  type In = String;
  type Out = String;

    fn decode(&mut self, buf: &mut EasyBuf) -> Result<Option<String>, io::Error> {
        if let Some(n) = buf.as_ref().iter().position(|b| *b == b'\n') {
            let line = buf.drain_to(n);

            buf.drain_to(1);

            return match str::from_utf8(&line.as_ref()) {
                Ok(s) => Ok(Some(s.to_string())),
                Err(_) => Err(io::Error::new(io::ErrorKind::Other, "invalid UTF8 string")),
            };
        }

        Ok(None)
    }

    fn encode(&mut self, msg: String, buf: &mut Vec<u8>) -> io::Result<()> {
        buf.extend_from_slice(msg.as_bytes());
        buf.push(b'\n');

        Ok(())
    }
}

impl<T: Io + 'static> ClientProto<T> for SbtProto {
  type Request = String;
  type Response = String;
  type Transport = Framed<T, SbtCodec>;
  type BindTransport = Result<Self::Transport, io::Error>;

    fn bind_transport(&self, io: T) -> Self::BindTransport {
        Ok(io.framed(SbtCodec))
    }
}
