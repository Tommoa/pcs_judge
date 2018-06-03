extern crate pcs_protocol;

extern crate futures;
use futures::Future;

extern crate tokio_rustls;
extern crate rustls;
extern crate tokio_core;
use tokio_core::{ net, reactor::Handle };

extern crate tokio_io;
use tokio_io::io::{ WriteHalf };

use std::io;

use std::sync::{ Arc, mpsc, Mutex };
use super::{ tasks, judge };

pub fn socket_response(socket: Arc<Mutex<tokio_rustls::TlsStream<net::TcpStream, rustls::ClientSession>>>, judge: mpsc::Sender<judge::ToMark>, handle: Handle) -> Result<(), io::Error> {
        Ok(())
}
