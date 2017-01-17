extern crate nsbt;

extern crate futures;
extern crate tokio_core;
extern crate rustyline;

use futures::{Future, Sink, Stream};
use futures::stream::BoxStream;
use futures::sync::mpsc;
use rustyline::Editor;
use tokio_core::reactor::Core;
use std::{io, thread};


fn main() {

    let mut core = Core::new().unwrap();
    let handle = core.handle();
    let handle2 = core.handle();

    let addr = "127.0.0.1:5369".parse().unwrap();

    let conn = nsbt::Client::connect(&addr, handle);

    let server = conn.and_then(|client| {
        let (up, down) = client.split();

        let fw = readline()
                     .map_err(|_| io::Error::new(io::ErrorKind::Other, "stdin error"))
                     .map(|s| {
                         nsbt::CommandMessage::ExecCommand {
                             command_line: s,
                             exec_id: None,
                         }
                     })
                     .forward(up)
                     .and_then(|(_, _)| Ok(()));

        let print = down.filter(|e| match e {
            &nsbt::EventMessage::LogEvent { .. } => true,
            _ => false,
        }).for_each(|line| {
                            println!("{}", line);
                            Ok(())
                        })
                        .map_err(|_| ());

        handle2.spawn(print);

        fw
    });

    core.run(server).unwrap();
}

fn readline() -> BoxStream<String, ()> {
    let (mut tx, rx) = mpsc::channel(1);

    thread::spawn(|| {
        let mut editor = Editor::<()>::new();
        loop {
            let readline = editor.readline("> ");
            match readline {
                Ok(ref str) if str == ":exit" => {
                    println!("Closing");
                    break;
                }
                Ok(line) => {
                    match tx.send(line).wait() {
                        Ok(t) => tx = t,
                        Err(_) => break,
                    }
                }
                Err(err) => {
                    println!("Error: {:?}", err);
                    break;
                }
            }
        }
    });

    rx.boxed()
}
