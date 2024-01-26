use anyhow::{bail, Result};

use crate::core::{message::Message, socket::Socket};

use super::link::LinkHandle;

const PID_KERNEL: u32 = 0;

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
                if m.verify_header(next_seq, pid, res_type).is_err() {
                    continue;
                }

                match m.extract_payload()? {
                    Some(payload) => res.push(payload),
                    None => break 'done,
                }

                if m.check_last_message() {
                    break 'done;
                }
            }
        }

        Ok(res)
    }
}
