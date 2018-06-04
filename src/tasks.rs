extern crate pcs_protocol;
use pcs_protocol::{ MsgType, SerDe };

extern crate futures;
use futures::prelude::*;

extern crate libc;

use std::sync::{ Arc, mpsc, Mutex };
use std::io::{ self, Read, Write };
use super::judge;

pub struct Judge<W> {
    pub recv: mpsc::Receiver<judge::ToSend>,
    pub send: Arc<Mutex<W>>,
}
impl<W: Write> Stream for Judge<W> {
    type Item = (judge::ToSend, Arc<Mutex<W>>);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, io::Error> {
        match self.recv.try_recv() {
            Ok(value) => Ok(Async::Ready(Some((value, self.send.clone())))),
            Err(mpsc::TryRecvError::Empty) => Ok(Async::NotReady),
            Err(mpsc::TryRecvError::Disconnected) => Err(io::Error::new(io::ErrorKind::BrokenPipe, "Judge thread disconnected"))
        }
    }
}

pub struct Server<R, W> {
    pub serv:    Arc<Mutex<W>>,
    pub read:    R,
    pub send:    mpsc::Sender<judge::ToMark>,
    pub recv_fd: i32
}
impl<R: Read, W: Write> Stream for Server<R, W> {
    type Item = (MsgType, Arc<Mutex<W>>, mpsc::Sender<judge::ToMark>);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, io::Error> {
        use std::mem;
        let mut pfd: libc::pollfd = unsafe { mem::zeroed() };
        pfd.fd = self.recv_fd;
        pfd.events |= libc::POLLIN;
        unsafe { libc::poll(&mut pfd, 1, 0) };
        if pfd.revents & libc::POLLIN > 0 {
            Ok(Async::Ready(Some((MsgType::deserialize(&mut self.read)?, self.serv.clone(), self.send.clone()))))
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
