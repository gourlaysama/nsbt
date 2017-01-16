extern crate nsbt;

extern crate futures;
extern crate tokio_core;
extern crate tokio_service;

use futures::Future;
use tokio_core::reactor::Core;
use tokio_service::Service;


fn main() {
    println!("Starting...");

    let mut core = Core::new().unwrap();
    let handle = core.handle();

    let addr = "127.0.0.1:5369".parse().unwrap();

    core.run(
        nsbt::Client::connect(&addr, handle)
            .and_then(|client| {
                client.call("{\"type\": \"ExecCommand\", \"commandLine\": \"clean\"}".to_string())
                    .and_then(move |response| {
                        println!("Got from the server: {:?}", response);
                        Ok(())
                    })
            })
        ).unwrap();
}
