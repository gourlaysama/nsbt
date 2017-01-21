extern crate nsbt;

extern crate futures;
extern crate tokio_core;
extern crate tokio_service;
extern crate rustyline;
extern crate rand;

use std::error::Error;
use std::rc::Rc;
use std::{io, thread};

use futures::{BoxFuture, Future, Stream};
use futures::sync::oneshot;
use nsbt::Client;
use rustyline::Editor;
use rustyline::error::ReadlineError;
use tokio_core::reactor::Core;
use tokio_service::Service;


fn main() {

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let addr = "127.0.0.1:5369".parse().unwrap();

    let evt_loop = core.run(Client::connect(&addr, &handle)
        .and_then(|client| {
            println!("Connected to sbt server at '{}'", &addr);
            println!("Run ':exit' to close the shell.");
            let client = Rc::new(client);
            futures::stream::repeat(client).for_each(|c| {
                let input = readline2()
                    .then(|f| match f {
                        Ok(Some(s)) => Ok(s),
                        Ok(None) => {
                            println!("Closing client shell.");
                            // we use an error to break out of the infinite stream above
                            // TODO: use something better than io::Error everywhere
                            Err(io::Error::new(io::ErrorKind::Other, "Closing"))
                        }
                        Err(_) => Err(io::Error::new(io::ErrorKind::Other, "Readline error")),
                    })
                    .and_then(move |s| {
                        let response = c.call(nsbt::CommandMessage::ExecCommand {
                            command_line: s,
                            exec_id: Some(format!("nsbt-exec-{}", rand::random::<u32>())),
                        });

                        response.and_then(|s| {
                            s.for_each(|event| {
                                println!("{}", event);
                                Ok(())
                            })
                        })
                    });

                input
            })
        })
        .then(|o| match o {
            // the stream above is infinite so this never happens
            Ok(a) => Ok(a),
            // we transform a "Closing" error into a graceful exit from the event loop
            Err(ref e) if e.description() == "Closing" => Ok(()),
            Err(e) => Err(e),

        }));

    match evt_loop {
        Err(e) => {
            println!("Error: {}", e);
            std::process::exit(-1);
        }
        Ok(_) => (),
    }
}

fn readline2() -> BoxFuture<Option<String>, futures::Canceled> {
    let (tx, rx) = oneshot::channel();

    thread::spawn(|| {
        let mut editor = Editor::<()>::new();

        let readline = editor.readline("> ");
        match readline {
            Ok(ref str) if str == ":exit" => {
                tx.complete(None);
            }
            Ok(line) => {
                tx.complete(Some(line));
            }
            Err(ReadlineError::Eof) |
            Err(ReadlineError::Interrupted) => {
                tx.complete(None);
            }
            Err(err) => {
                println!("Error: {:?}", err);
                tx.complete(None);
            }
        }

    });

    rx.boxed()
}
