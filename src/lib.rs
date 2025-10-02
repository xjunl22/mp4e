//! # mp4e
//!
#![doc = include_str!("../README.md")]
#![doc = include_str!("../LICENSE")]

mod mp4e;
pub mod nalu;
mod util;
pub use mp4e::{Codec, Mp4e};

#[cfg(test)]
mod tests {

    #[test]
    fn parse_nalu_test() {
        use crate::nalu::split_nalu;
        let nalu0 = [
            0x00, 0x00, 0x00, 0x01, 0x67, 0x42, 0xC0, 0x0D, 0xF4, 0x01, 0x00, 0x03, 0x00, 0x04,
            0x00, 0x00, 0x03, 0x00, 0x64, 0x00, 0x00, 3,
        ];
        let nalu1: [u8; 15] = [
            0x00, 0x00, 0x01, 0x68, 0xE1, 0x01, 0x00, 0x00, 0x03, 0x00, 0x04, 0x00, 0x00, 0x03,
            0x00,
        ];
        let nalu: Vec<u8> = [&nalu0[..], &nalu1[..]].concat();
        let mut iter = split_nalu(&nalu[..]);
        assert!(iter.next().unwrap().eq(&nalu0[4..]));
        assert!(iter.next().unwrap().eq(&nalu1[3..]));
        assert!(iter.next().is_none());
    }
}
