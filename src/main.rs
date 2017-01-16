extern crate nsbt;

extern crate futures;
extern crate tokio_core;

use futures::{Future, Sink, Stream};
use tokio_core::reactor::Core;


fn main() {
    println!("Starting...");

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let addr = "127.0.0.1:5369".parse().unwrap();

    core.run(
        nsbt::Client::connect(&addr, handle)
            .and_then(|client| {
                client.send("{\"type\": \"ExecCommand\", \"commandLine\": \"clean\"}".to_string())
                    .and_then(|c| {
                        println!("Sent clean command to server");
                        Ok(c)
                    })
            })
            .and_then(|client| {
                client.for_each(|line| {
                    println!("Received from Server: {}", line);
                    Ok(())
                })
            })
        ).unwrap();
}
