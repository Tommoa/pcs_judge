extern crate pcs_protocol;
use pcs_protocol::{ MsgType, SerDe };

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

pub fn socket_response(msg: MsgType, socket: Arc<Mutex<WriteHalf<tokio_rustls::TlsStream<net::TcpStream, rustls::ClientSession>>>>, judge: mpsc::Sender<judge::ToMark>, handle: Handle) -> Result<(), io::Error> {
        use futures::future::ok;
        match msg {
            MsgType::Verify => {
                let to_write = Vec::new();

                let write = tasks::Writer {
                    send:     socket,
                    to_write: to_write,
                    done:     0
                }
                    .map(|_| debug!("Succeeded in writing to socket!"))
                    .map_err(|e| error!("Error writing to socket! {}", e));
                handle.spawn(write);
            },
            MsgType::Hash(_hash) => {
                let fut = ok(());
                handle.spawn(fut);
            },
            MsgType::Decline => {
                let fut = ok(());
                handle.spawn(fut);
            },
            MsgType::Accept => {
                let fut = ok(());
                handle.spawn(fut);
            },
            MsgType::Need(_need) => {
                let fut = ok(());
                handle.spawn(fut);
            },
            MsgType::Status(_status) => {
                let fut = ok(());
                handle.spawn(fut);
            },
            MsgType::Give(_give) => {
                let fut = ok(());
                handle.spawn(fut);
            },
            MsgType::Mark(_mark) => {
                let fut = ok(());
                handle.spawn(fut);
            },
            MsgType::Marking(_marking) => {
                let fut = ok(());
                handle.spawn(fut);
            },
            MsgType::Done(_done) => {
                let fut = ok(());
                handle.spawn(fut);
            }
        };
        Ok(())
}
