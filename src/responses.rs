extern crate pcs_protocol;

use std::io;
use std::sync::{ Arc, mpsc, Mutex };

use super::judge;

pub fn socket_response<W: io::Write>(
    _msg: pcs_protocol::MsgType,
    _write: Arc<Mutex<W>>,
    _judge: mpsc::Sender<judge::ToMark>) -> Result<(), io::Error>
{
    Ok(())
}
