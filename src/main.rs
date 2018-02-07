#[macro_use]
extern crate clap;
extern crate futures;
#[macro_use]
extern crate log;
extern crate nsbt;
extern crate rand;
extern crate rustyline;
extern crate simplelog;
extern crate tokio_core;
extern crate tokio_service;

use std::cell::RefCell;
use std::error::Error;
use std::fs::File;
use std::rc::Rc;
use std::{io, thread};

use clap::{App, Arg};
use futures::{BoxFuture, Future, Stream};
use futures::sync::oneshot;
use nsbt::{messages, Client};
use rustyline::Editor;
use rustyline::error::ReadlineError;
use simplelog::*;
use tokio_core::reactor::Core;
use tokio_service::Service;

fn main() {
    let matches = App::new("nsbt")
        .version(crate_version!())
        .author(crate_authors!())
        .about("Connects to a running sbt server to send commands to it")
        .arg(
            Arg::with_name("host")
                .short("H")
                .long("host")
                .help("Host to connect to")
                .takes_value(true)
                .default_value("127.0.0.1"),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .long("port")
                .help("Port to connect to")
                .takes_value(true)
                .default_value("5369"),
        )
        .arg(
            Arg::with_name("commands")
                .multiple(true)
                .index(1)
                .value_name("COMMANDS")
                .help(
                    "Some commands to run once connected to the sbt server.
If no command is specified, an interactive shell is displayed.",
                ),
        )
        .arg(
            Arg::with_name("v")
                .short("v")
                .multiple(true)
                .help("Sets the level of verbosity"),
        )
        .get_matches();

    let level = match matches.occurrences_of("v") {
        0 => LevelFilter::Info,
        1 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };
    TermLogger::init(
        level,
        Config {
            time: Some(Level::Debug),
            level: Some(Level::Error),
            target: Some(Level::Debug),
            location: Some(Level::Trace),
            time_format: None,
        },
    ).unwrap();

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let addr = format!(
        "{}:{}",
        matches.value_of("host").unwrap(),
        matches.value_of("port").unwrap()
    ).parse()
        .unwrap();

    let commands = matches.values_of_lossy("commands");
    let go_to_shell = match commands {
        None => true,
        Some(ref c) => c.iter().any(|s| s == ":shell"),
    };
    let commands = commands.map(|c| c.into_iter());
    let rcmds = commands.map(|c| Rc::new(RefCell::new(c)));
    //let b_commands = commands.map(|c| c.as_ref());

    let evt_loop = core.run(
        Client::connect(&addr, &handle)
            .and_then(|client| {
                info!("[client] Connected to sbt server at '{}'", &addr);
                if go_to_shell {
                    info!("[client] Run ':exit' to close the shell.");
                }
                let client = Rc::new(client);
                futures::stream::repeat((client, rcmds)).for_each(|(cl, cds)| {
                    let next_command = futures::future::result(
                        cds.ok_or(futures::Canceled)
                            .and_then(|c| c.borrow_mut().next().ok_or(futures::Canceled)),
                    ).then(|e| match e {
                        Ok(ref s) if s == ":shell" => Err(futures::Canceled),
                        Ok(a) => Ok(Some(a)),
                        Err(a) => Err(a),
                    });

                    let input = next_command
                        .or_else(|_| {
                            if go_to_shell {
                                readline2()
                            } else {
                                futures::future::ok(None).boxed()
                            }
                        })
                        .then(|f| match f {
                            Ok(Some(s)) => Ok(s),
                            Ok(None) => {
                                if go_to_shell {
                                    info!("[client] Closing shell.");
                                }
                                // we use an error to break out of the infinite stream above
                                // TODO: use something better than io::Error everywhere
                                Err(io::Error::new(io::ErrorKind::Other, "Closing"))
                            }
                            Err(_) => Err(io::Error::new(io::ErrorKind::Other, "Readline error")),
                        })
                        .and_then(move |s| {
                            let response = cl.call(messages::CommandMessage::ExecCommand {
                                command_line: s,
                                exec_id: Some(format!("nsbt-exec-{}", rand::random::<u32>())),
                            });

                            response.and_then(|s| {
                                s.for_each(|event| {
                                    info!("[server] {}", event);
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
            }),
    );

    match evt_loop {
        Err(e) => {
            error!("[client] {}", e);
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
                tx.send(None).unwrap();
            }
            Ok(line) => {
                tx.send(Some(line)).unwrap();
            }
            Err(ReadlineError::Eof) | Err(ReadlineError::Interrupted) => {
                tx.send(None).unwrap();
            }
            Err(err) => {
                error!("{:?}", err);
                tx.send(None).unwrap();
            }
        }
    });

    rx.boxed()
}
