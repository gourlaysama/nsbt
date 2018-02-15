#[macro_use]
extern crate clap;
extern crate futures;
#[macro_use]
extern crate log;
extern crate nsbt;
extern crate simplelog;
extern crate tokio_core;
extern crate tokio_io;
extern crate tokio_uds;

use std::{env, io};

use clap::{App, Arg};
use futures::{stream, Future, Stream};
use nsbt::sbt_utils;
use nsbt::proto::SbtProto;
use simplelog::{Config, LevelFilter, TermLogger};
use tokio_core::reactor::Core;
use tokio_uds::UnixStream;

fn main() {
    std::process::exit(match run() {
        Ok(_) => 0,
        Err(_) => 1,
    });
}

fn run() -> Result<(), ()> {
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

    let log_level = match matches.occurrences_of("v") {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    TermLogger::init(log_level, Config::default()).unwrap();

    let path = match sbt_utils::lookup_from(&env::current_dir().unwrap()) {
        Ok(Some(path)) => path,
        Ok(None) => {
            error!("Cound not find running sbt server.");
            return Err(());
        }
        Err(e) => {
            error!("{}", e);
            return Err(());
        }
    };

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let stream = UnixStream::connect(path, &handle).unwrap();

    let proto = SbtProto::init(stream);

    let commands = matches.values_of_lossy("commands").unwrap_or(vec![]);
    let commands = stream::iter_ok::<_, io::Error>(commands);

    let task = commands
        .into_future()
        .map_err(|(e, _)| e)
        .join(proto)
        .and_then(|((c, cs), proto)| process_command(c, cs, proto, true));

    //let task = proto.and_then(|p| p.call("show scalaVersion"));

    let evt_loop = core.run(task.map(|_| ()));

    evt_loop.map_err(|e| {
        error!("{}", e);
        ()
    })
}

fn process_command<'a, S: 'a>(
    c: Option<String>,
    c_stream: S,
    proto: SbtProto,
    first: bool,
) -> Box<Future<Item = ((Option<String>, S), SbtProto), Error = io::Error> + 'a>
where
    S: Stream<Item = String, Error = io::Error>,
{
    match c {
        Some(command) => {
            trace!("New command: '{}'", command);
            Box::new(proto.call(&command, first).and_then(|proto| {
                c_stream
                    .into_future()
                    .map_err(|(e, _)| e)
                    .and_then(|cc| process_command(cc.0, cc.1, proto, false))
            }))
        }
        None => {
            trace!("No more commands, stopping.");
            Box::new(futures::future::ok(((None, c_stream), proto)))
        }
    }
}
