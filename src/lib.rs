extern crate futures;
extern crate tokio_core;

use futures::{Future, Stream};
use tokio_core::io::{Io, Codec, EasyBuf, Framed};
use tokio_core::net::TcpStream;
use tokio_core::reactor::Handle;
use std::{io, str};
use std::net::SocketAddr;

pub struct Client;

pub struct SbtCodec;


impl Client {
    pub fn connect(addr: &SocketAddr,
                   handle: Handle)
                   -> Box<Future<Item = Framed<TcpStream,SbtCodec>, Error = io::Error>> {
        let transport = TcpStream::connect(addr, &handle).and_then(|socket| {
            let transport = socket.framed(SbtCodec);

            let handshake = transport.into_future()
                .map_err(|(e,_)| e)
                .and_then(|(line, transport)| {
                match line {
                  Some(ref msg) if msg.contains("ChannelAcceptedEvent") => {
                    println!("Server accepted our channel");
                    Ok(transport)
                  }
                  _ => {
                    println!("Server handshake invalid!");
                    let err = io::Error::new(io::ErrorKind::Other, "invalid handshake");
                    Err(err)
                  }
              }
            });
            
           handshake
        });


        Box::new(transport)
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
