use std::io;

use codec::JsonRpcFramingCodec;
use futures::{Future, Sink, Stream};
use futures::future;
use languageserver_types::{LogMessageParams, PublishDiagnosticsParams};
use serde_json;
use tokio_io::AsyncRead;
use tokio_uds::UnixStream;

use messages::{CommandMessage, EventMessage};

pub struct SbtProto {
    sink: Box<Sink<SinkItem = String, SinkError = io::Error>>,
    stream: Box<Stream<Item = String, Error = io::Error>>,
    next_id: u8,
}

impl SbtProto {
    pub fn init(stream: UnixStream) -> Box<Future<Item = SbtProto, Error = io::Error>> {
        let transport = stream.framed(JsonRpcFramingCodec::new());

        let (t_sink, t_stream) = transport.split();

        let init_str = serde_json::to_string(&CommandMessage::initialize(1)).unwrap();
        let f_sink = t_sink.send(init_str);

        Box::new(f_sink.and_then(|t_sink| {
            let response = t_stream.into_future();
            response
                .map_err(|(e, _)| e)
                .and_then(|(item, t_stream)| match item {
                    Some(r) => {
                        debug!("Received capabilities: {}", r);
                        Ok(SbtProto {
                            sink: Box::new(t_sink),
                            stream: Box::new(t_stream),
                            next_id: 1,
                        })
                    }
                    None => Err(io::Error::from(io::ErrorKind::Interrupted)),
                })
        }))
    }

    pub fn call(
        self,
        command: &str,
        first: bool,
    ) -> Box<Future<Item = SbtProto, Error = io::Error>> {
        debug!("Calling with command '{}', first={}", command, first);
        let next_id = self.next_id;
        let exec_str =
            serde_json::to_string(&CommandMessage::sbt_exec(next_id, command.to_string())).unwrap();
        let f_sink = self.sink.send(exec_str);
        let t_stream = self.stream;

        let fut = f_sink.and_then(move |t_sink| {
            let t_stream = process_event(t_stream, false, first);
            t_stream.map(move |t_stream| {
                info!("Finished running command.");
                SbtProto {
                    sink: t_sink,
                    stream: t_stream,
                    next_id: next_id + 1,
                }
            })
        });

        Box::new(fut)
    }
}

fn process_event<'a, S: 'a>(
    t_stream: S,
    waiting: bool,
    first: bool,
) -> Box<Future<Item = S, Error = io::Error> + 'a>
where
    S: Stream<Item = String, Error = io::Error>,
{
    let fut = t_stream
        .into_future()
        .map_err(|(e, _)| e)
        .and_then(|(opt, t_stream)| match opt {
            Some(event) => Ok((event, t_stream)),
            None => Err(io::Error::from(io::ErrorKind::Interrupted)),
        })
        .and_then(|(event, t_stream)| {
            let ev = serde_json::from_str::<EventMessage>(&event);
            ev.map(|e| (e, t_stream))
                .map_err(|er| io::Error::new(io::ErrorKind::InvalidData, er))
        })
        .and_then(move |(ev, t_stream)| match ev {
            EventMessage::Log(LogMessageParams { ref message, .. }) => {
                match (message.as_ref(), waiting) {
                    ("Processing", false) => {
                        debug!("Processing 1 started.");
                        process_event(t_stream, false, first)
                    }
                    ("Done", true) => {
                        debug!("Processing 2 done.");
                        Box::new(future::ok(t_stream))
                    }
                    ("Processing", true) => {
                        debug!("Processing 2 started.");
                        process_event(t_stream, true, first)
                    }
                    ("Done", false) => {
                        debug!("Processing 1 done.");
                        if first {
                            process_event(t_stream, true, first)
                        } else {
                            Box::new(future::ok(t_stream))
                        }
                    }
                    _ => {
                        println!("{}", ev);
                        process_event(t_stream, waiting, first)
                    }
                }
            }
            EventMessage::Diagnostic(PublishDiagnosticsParams {
                ref diagnostics, ..
            }) => {
                debug!("Received diagnostics: {:?}", diagnostics);
                print!("{}", ev);
                process_event(t_stream, waiting, first)
            }
        });

    Box::new(fut)
}
