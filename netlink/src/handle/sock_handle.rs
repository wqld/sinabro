use anyhow::{bail, Result};

use crate::core::{message::Message, socket::Socket};

use super::{addr::AddrHandle, link::LinkHandle};

const PID_KERNEL: u32 = 0;

const NLMSG_DONE: u16 = 3;
const NLMSG_ERROR: u16 = 2;

#[derive(Clone)]
pub struct SocketHandle {
    pub socket: Socket,
    pub seq: u32,
}

impl SocketHandle {
    pub fn new(proto: i32) -> Self {
        Self {
            socket: Socket::new(proto, 0, 0).unwrap(),
            seq: 0,
        }
    }

    pub fn next_seq(&mut self) -> u32 {
        self.seq += 1;
        self.seq
    }

    pub fn handle_link(&self) -> LinkHandle {
        LinkHandle::from(self.clone())
    }

    pub fn handle_addr(&self) -> AddrHandle {
        AddrHandle::from(self.clone())
    }

    pub fn request(&mut self, msg: &mut Message, res_type: u16) -> Result<Vec<Vec<u8>>> {
        let next_seq = self.next_seq();
        msg.header.nlmsg_seq = next_seq;

        self.socket.send(&msg.serialize()?)?;

        let pid = self.socket.pid()?;
        let mut res: Vec<Vec<u8>> = Vec::new();

        'done: loop {
            let (msgs, from) = self.socket.recv()?;

            if from.nl_pid != PID_KERNEL {
                bail!(
                    "wrong sender pid: {}, expected: {}",
                    from.nl_pid,
                    PID_KERNEL
                );
            }

            for m in msgs {
                if m.verify_header(next_seq, pid).is_err() {
                    continue;
                }

                match m.header.nlmsg_type {
                    NLMSG_DONE | NLMSG_ERROR => {
                        let payload = m.payload.as_ref().unwrap();
                        let err_no = i32::from_ne_bytes(payload[0..4].try_into()?);

                        if err_no == 0 {
                            break 'done;
                        }

                        let err_msg = std::io::Error::from_raw_os_error(-err_no);
                        bail!("{} ({}): {:?}", err_msg, -err_no, &payload[4..]);
                    }
                    t if res_type != 0 && t != res_type => {
                        continue;
                    }
                    _ => {
                        res.push(m.payload.clone().unwrap());
                    }
                }

                if m.check_last_message() {
                    break 'done;
                }
            }
        }

        Ok(res)
    }
}
