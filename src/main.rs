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

    let addr = "127.0.0.1:5369".parse().unwrap();

    let console = readline();

    core.run(
        nsbt::Client::connect(&addr, handle)
            .and_then(|client| {
                let (up, down) = client.split();

                let fw = console.map_err(|_| io::Error::new(io::ErrorKind::Other, "stdin error"))
                  .map(|s| format!("{{\"type\": \"ExecCommand\", \"commandLine\": \"{}\"}}", s)).forward(up);

                down.for_each(|line| {
                    println!("[received] {}", line);
                    Ok(())
                }).join(fw)
            })
        ).unwrap();
}

fn readline() -> BoxStream<String, ()> {
    let (mut tx, rx) = mpsc::channel(1);

    thread::spawn(|| {
        let mut editor = Editor::<()>::new();
        loop {
          let readline = editor.readline(">");
          match readline {
              Ok(line) =>
                match tx.send(line).wait() {
                    Ok(t) => tx = t,
                    Err(_) => break,
                },
              Err(err) => {
                    println!("Error: {:?}", err);
                    break
              }
         }
       }
    });

    rx.boxed()
}
