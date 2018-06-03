extern crate pcs_protocol;

extern crate futures;
use futures::prelude::*;

extern crate rustls;
extern crate tokio_rustls;
extern crate tokio_core;
use tokio_core::net;
extern crate tokio_io;
use tokio_io::{ AsyncWrite };

extern crate libc;

use std::sync::{ Arc, mpsc, Mutex };
use std::io;
use super::judge;

pub struct Judge {
    pub recv: mpsc::Receiver<judge::ToSend>,
    pub send: Arc<Mutex<tokio_rustls::TlsStream<net::TcpStream, rustls::ClientSession>>>,
}
impl Stream for Judge {
    type Item = (judge::ToSend, Arc<Mutex<tokio_rustls::TlsStream<net::TcpStream, rustls::ClientSession>>>);
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
    pub serv:    Arc<Mutex<tokio_rustls::TlsStream<net::TcpStream, rustls::ClientSession>>>,
    pub send:    mpsc::Sender<judge::ToMark>,
    pub recv_fd: i32
}
impl Stream for Server {
    type Item = (Arc<Mutex<tokio_rustls::TlsStream<net::TcpStream, rustls::ClientSession>>>, mpsc::Sender<judge::ToMark>);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, io::Error> {
        use std::mem;
        let mut pfd: libc::pollfd = unsafe { mem::zeroed() };
        pfd.fd = self.recv_fd;
        pfd.events |= libc::POLLIN;
        unsafe { libc::poll(&mut pfd, 1, 0) };
        if pfd.revents & libc::POLLIN > 0 {
            Ok(Async::Ready(Some((self.serv.clone(), self.send.clone()))))
        } else if pfd.revents & libc::POLLHUP > 0 {
            Ok(Async::Ready(None))
        } else if pfd.revents & libc::POLLERR > 0 {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "POLLERR"))
        } else if pfd.revents & libc::POLLNVAL > 0 {
            Err(io::Error::new(io::ErrorKind::InvalidData, "POLLNVAL"))
        } else {
            Ok(Async::NotReady)
        }
    }
}

pub struct Writer<T> {
    pub send:     Arc<Mutex<T>>,
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
