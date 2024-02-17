pub mod core;
pub mod handle;
pub mod netlink;
pub mod route;

const RTA_MTU: u16 = 0x2;
const RTA_VIA: u16 = 18;

pub fn align_of(len: usize, align_to: usize) -> usize {
    (len + align_to - 1) & !(align_to - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        assert_eq!(align_of(0x10, 0x4), 0x10);
    }
}
