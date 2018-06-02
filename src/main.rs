extern crate clap;
use clap::{ Arg, App };

extern crate futures;
use futures::{ Future, Stream };

#[macro_use] extern crate log;
extern crate pretty_env_logger;

extern crate pcs_protocol;
use pcs_protocol::*;

extern crate rustls;

extern crate tokio_rustls;
use tokio_rustls::ClientConfigExt;

#[macro_use]
extern crate serde_derive;

extern crate tokio_core;
use tokio_core::{ net, reactor };

extern crate tokio_io;
use tokio_io::{ AsyncRead };

extern crate webpki;

mod executor;
mod judge;
mod ssl;
mod responses;
mod tasks;

use std::io;
use std::net::ToSocketAddrs;
use std::sync::{ Arc, Mutex };

fn main() -> Result<(), io::Error> {
    pretty_env_logger::init();

    let m = App::new("PCS competition judge")
        .author("Tom Almeida, tommoa256@gmail.com")
        .version("0.1")
        .about("A judge for running programs")
        .arg(
            Arg::with_name("host")
            .short("h")
            .long("host")
            .default_value("localhost")
            )
        .arg(
            Arg::with_name("domain")
            .short("d")
            .long("domain")
            .takes_value(true)
            )
        .arg(
            Arg::with_name("port")
            .short("p")
            .long("port")
            .default_value("11286")
            )
        .arg(Arg::with_name("cert")
             .short("c")
             .long("certificate")
             .takes_value(true)
            )
        .arg(Arg::with_name("executors")
             .short("e")
             .long("executors")
             .default_value("executors/")
            )
        .get_matches();

    debug!("Finished processing arguments");

    let mut core = reactor::Core::new().unwrap();
    let handle = core.handle();

    let server = m.value_of("host").unwrap();
    let port = m.value_of("port").unwrap().parse().unwrap();
    let domain = m.value_of("domain").unwrap_or(server);
    let addr = (server, port)
        .to_socket_addrs().unwrap()
        .next().unwrap(); 

    info!("Trying to connect to server!");
    let connection = net::TcpStream::connect(&addr, &handle);
    let arc_config = ssl::setup(m.value_of("cert"));

    let client = connection.map_err(|e| {
        error!("Error connecting to TCP server {}! {}", addr, e);
        e
    }).and_then(move |stream| {
        info!("Made TCP connection to {}", server);
        let domain = webpki::DNSNameRef::try_from_ascii_str(&domain).unwrap();
        info!("Trying to handshake SSL");
        arc_config.connect_async(domain, stream)
    }).wait().map_err(|e| {
        error!("Error connecting to SSL! {}", e);
    }).unwrap();
    info!("SSL connected");

    let (_, to_judge, from_judge) = judge::setup(m.value_of("executors").unwrap().to_string());
    info!("Started judge thread");

    let (read, write) = client.split();
    let write = Arc::new(Mutex::new(write));

    let judge_socket = tasks::Judge {
        recv: from_judge,
        send: write.clone()
    };
    let judge_stream = judge_socket.for_each(move |(mark, socket)| {
        let mut to_write = Vec::new();
        let done = MsgType::Done(MsgDone {
            sequence:   mark.sequence,
            batch:      mark.batch,
            test:       mark.case,
            result:     mark.result
        });
        done.serialize(&mut to_write);
        let write = tasks::Writer {
            send:     socket.clone(),
            to_write: to_write,
            done:     0
        }
            .map(|_| debug!("Succeeded in writing to socket!"))
            .map_err(|e| error!("Error writing to socket! {}", e));

        handle.spawn(write);
        Ok(())
    });

    let handle = core.handle();
    let server_socket = tasks::Server {
        recv:        read,
        resp:        write.clone(),
        send:        to_judge
    };
    let server_stream = server_socket.for_each(move |(msg, socket, judge)| responses::socket_response(msg, socket, judge, handle.clone()));

    core.run(judge_stream.select(server_stream)).map(|_| ()).map_err(|x| x.0)
}
