/// HEVC NALU types
pub const HEVC_NALU_TYPE_VPS: u8 = 32;
pub const HEVC_NALU_TYPE_SPS: u8 = 33;
pub const HEVC_NALU_TYPE_PPS: u8 = 34;

/// AVC NALU types
pub const AVC_NALU_TYPE_SPS: u8 = 7;
pub const AVC_NALU_TYPE_PPS: u8 = 8;

/// HEVC NALU types for specific slice types
pub const HEVC_NAL_BLA_W_LP: u8 = 16;
pub const HEVC_NAL_CRA_NUT: u8 = 21;

/// AVC NALU type for I-Slice
pub const AVC_NAL_ISLICE_NALU: u8 = 5;

// src/nalu.rs
/// Splits a byte slice into an iterator over NAL units.
///
/// This function takes a byte slice containing H.264/H.265 encoded data and returns
/// an iterator that yields individual NAL units. It handles both 3-byte (0x000001)
/// and 4-byte (0x00000001) start codes used to delimit NAL units in the bitstream.
///
/// # Arguments
///
/// * `data` - A byte slice containing the encoded video data
///
/// # Returns
///
/// An iterator that yields references to individual NAL units without their start codes
///
/// # Examples
///
/// ```
/// use mp4e::nalu::split_nalu;
///
/// let data = [0, 0, 0, 1, 10, 20, 30, 0, 0, 1, 40, 50,0, 0, 0, 1, 60, 70, 80];
/// let mut nalus = split_nalu(&data);
/// assert_eq!(nalus.next().unwrap(), &[10, 20, 30]);
/// assert_eq!(nalus.next().unwrap(), &[40, 50]);
/// assert_eq!(nalus.next().unwrap(), &[60, 70, 80])
/// assert_eq!(nalus.next(), None);
/// ```
pub fn split_nalu<'a>(data: &'a [u8]) -> impl Iterator<Item = &'a [u8]> + 'a {
    struct NaluIterator<'a> {
        data: &'a [u8],
        position: usize,
    }

    impl<'a> Iterator for NaluIterator<'a> {
        type Item = &'a [u8];

        fn next(&mut self) -> Option<Self::Item> {
            if self.position >= self.data.len() {
                return None;
            }

            // Find start code (0x00000001 or 0x000001)
            let start = self.position;
            let mut end = start;

            // Skip start code
            if start == 0 {
                // Find first start code
                if self.data.len() >= 4
                    && self.data[0] == 0
                    && self.data[1] == 0
                    && self.data[2] == 0
                    && self.data[3] == 1
                {
                    // 4-byte start code
                    self.position += 4;
                    return self.next();
                } else if self.data.len() >= 3
                    && self.data[0] == 0
                    && self.data[1] == 0
                    && self.data[2] == 1
                {
                    // 3-byte start code
                    self.position += 3;
                    return self.next();
                } else {
                    // No start code found, return entire data
                    self.position = self.data.len();
                    return Some(self.data);
                }
            }

            // Find next start code as end of current NALU
            while end < self.data.len() {
                // Check if there are enough bytes for start code
                if end + 3 < self.data.len()
                    && self.data[end] == 0
                    && self.data[end + 1] == 0
                    && self.data[end + 2] == 1
                {
                    // Found 3-byte start code
                    break;
                } else if end + 4 < self.data.len()
                    && self.data[end] == 0
                    && self.data[end + 1] == 0
                    && self.data[end + 2] == 0
                    && self.data[end + 3] == 1
                {
                    // Found 4-byte start code
                    break;
                }
                end += 1;
            }

            if end < self.data.len() {
                // Found next start code
                let nalu = &self.data[start..end];
                // Update position to after next start code
                if end + 4 < self.data.len()
                    && self.data[end] == 0
                    && self.data[end + 1] == 0
                    && self.data[end + 2] == 0
                    && self.data[end + 3] == 1
                {
                    self.position = end + 4;
                } else {
                    self.position = end + 3;
                }
                self.data = &self.data[end..];
                self.position = 0;
                Some(nalu)
            } else {
                // This is the last NALU
                self.position = self.data.len();
                Some(&self.data[start..])
            }
        }
    }

    NaluIterator { data, position: 0 }
}
