extern crate pcs_protocol;
use pcs_protocol::{ MsgType, SerDe };

extern crate futures;
use futures::prelude::*;

extern crate rustls;
extern crate tokio_rustls;
extern crate tokio_core;
use tokio_core::net;
extern crate tokio_io;
use tokio_io::{ AsyncRead, AsyncWrite, io::{ ReadHalf, WriteHalf } };

use std::sync::{ Arc, mpsc, Mutex };
use std::io;
use super::judge;

pub struct Judge {
    pub recv: mpsc::Receiver<judge::ToSend>,
    pub send: Arc<Mutex<WriteHalf<tokio_rustls::TlsStream<net::TcpStream, rustls::ClientSession>>>>,
}
impl Stream for Judge {
    type Item = (judge::ToSend, Arc<Mutex<WriteHalf<tokio_rustls::TlsStream<net::TcpStream, rustls::ClientSession>>>>);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, io::Error> {
        match self.recv.try_recv() {
            Ok(value) => Ok(Async::Ready(Some((value, self.send.clone())))),
            Err(mpsc::TryRecvError::Empty) => Ok(Async::NotReady),
            Err(mpsc::TryRecvError::Disconnected) => Err(io::Error::new(io::ErrorKind::BrokenPipe, "Judge thread disconnected"))
        }
    }
}

pub struct Server {
    pub recv:        ReadHalf<tokio_rustls::TlsStream<net::TcpStream, rustls::ClientSession>>,
    pub resp:        Arc<Mutex<WriteHalf<tokio_rustls::TlsStream<net::TcpStream, rustls::ClientSession>>>>,
    pub send:        mpsc::Sender<judge::ToMark>,
}
impl Stream for Server {
    type Item = (MsgType, Arc<Mutex<WriteHalf<tokio_rustls::TlsStream<net::TcpStream, rustls::ClientSession>>>>, mpsc::Sender<judge::ToMark>);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, io::Error> {
        let mut v = [0u8;2];
        match self.recv.poll_read(&mut v) {
            Ok(Async::Ready(2)) => {
                Ok(Async::Ready(
                    Some((MsgType::deserialize(v, &mut self.recv).unwrap(),
                          self.resp.clone(), self.send.clone()))
                ))
            },
            Ok(_) => {
                Ok(Async::NotReady)
            },
            Err(e) => {
                Err(e)
            }
        }
    }
}

pub struct Writer<T> {
    pub send:     Arc<Mutex<WriteHalf<T>>>,
    pub to_write: Vec<u8>,
    pub done:     usize
}
impl<T: AsyncWrite> Future for Writer<T> {
    type Item = ();
    type Error = io::Error;
    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        if self.done >= self.to_write.len() {
            Ok(Async::Ready(()))
        } else {
            match self.send.lock().unwrap().poll_write(&self.to_write[self.done..]) {
                Ok(Async::Ready(x)) => {
                    self.done += x;
                    if self.done >= self.to_write.len() {
                        Ok(Async::Ready(()))
                    } else {
                        Ok(Async::NotReady)
                    }
                },
                Ok(Async::NotReady) => Ok(Async::NotReady),
                Err(e) => Err(e)
            }
        }
    }
}
