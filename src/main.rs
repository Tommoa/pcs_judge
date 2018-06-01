extern crate clap;
use clap::{ Arg, App };

extern crate futures;
use futures::prelude::*;

#[macro_use] extern crate log;
extern crate pretty_env_logger;

extern crate pcs_protocol;
use pcs_protocol::*;

extern crate mio;

extern crate rustls;

extern crate tokio_rustls;
use tokio_rustls::ClientConfigExt;

#[macro_use]
extern crate serde_derive;

extern crate tokio_core;
use tokio_core::{ net, reactor };

extern crate tokio_io;
use tokio_io::AsyncRead;

extern crate webpki;

mod judge;
mod ssl;
mod executor;

use std::io;
use std::sync::mpsc;
use std::net::ToSocketAddrs;

struct EventLoop {
    pub ssl:        tokio_rustls::TlsStream<net::TcpStream, rustls::ClientSession>,
    pub judge_recv: mpsc::Receiver<judge::ToSend>,
    pub judge_send: mpsc::Sender<judge::ToMark>,
}
impl Future for EventLoop {
    type Item = ();
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> { 
        let mut to_write = Vec::new();
        match self.judge_recv.try_recv() {
            Ok(value) => {
                let done = MsgType::Done(MsgDone {
                    sequence:   value.sequence,
                    batch:      value.batch,
                    test:       value.case,
                    result:     value.result
                });
                done.serialize(&mut to_write);
            },
            Err(err) => {
                match err {
                    mpsc::TryRecvError::Disconnected => {
                        error!("Judge queue disconnected!");
                        return Err(io::Error::new(io::ErrorKind::BrokenPipe, "Judge thread closed"))
                    },
                    mpsc::TryRecvError::Empty => {},
                }
            }
        }
        let mut v = [0u8;2];
        match self.ssl.poll_read(&mut v) {
            Ok(Async::Ready(_)) => {
                match MsgType::deserialize(v, &mut self.ssl) {
                    Ok(res) => {
                        match res {
                            MsgType::Verify => {
                                let m = MsgType::Accept;
                                m.serialize(&mut to_write);
                            },
                            MsgType::Decline => {
                                info!("Declined by server!");
                                return Ok(Async::Ready(()));
                            },
                            MsgType::Accept => {
                                info!("Serving server!");
                            },
                            MsgType::Mark(mark) => {
                                self.judge_send.send(judge::ToMark {
                                    sequence:   mark.sequence,
                                    batch:      mark.batch,
                                    answer:     mark.text,
                                    lang:       mark.lang,
                                    max_time:   mark.time,
                                    case_in:    Vec::new(),
                                    case_out:   Vec::new()
                                }).unwrap();
                            },
                            MsgType::Done(_) => {
                                info!("Closing requested by server!");
                                return Ok(Async::Ready(()));
                            },
                            _ => {}
                        }
                    },
                    Err(err) => {
                        error!("Couldn't deserialize message! {}", err);
                        return Err(err);
                    }
                }
            },
            Ok(Async::NotReady) => {
            },
            Err(err) => {
                error!("Error reading from socket! {}", err);
                return Err(err);
            }
        }
        Ok(Async::NotReady)
    }
}

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

    let event_loop = EventLoop {
        ssl:        client,
        judge_recv: from_judge,
        judge_send: to_judge,
    };

    core.run(event_loop)
}
