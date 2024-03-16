pub mod addr;
pub mod generic;
pub mod link;
pub mod neigh;
pub mod routing;
pub mod sock_handle;

#[macro_export]
macro_rules! test_setup {
    () => {
        if !nix::unistd::getuid().is_root() {
            eprintln!("test skipped, requires root");
            return;
        }
        nix::sched::unshare(nix::sched::CloneFlags::CLONE_NEWNET).expect("unshare(CLONE_NEWNET)");
    };
}

pub fn zero_terminated(s: &str) -> Vec<u8> {
    let mut v = Vec::with_capacity(s.len() + 1);
    v.extend_from_slice(s.as_bytes());
    v.push(0);
    v
}
