pub struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> BitReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data: data, pos: 0 }
    }

    // Decodes an unsigned exponential-Golomb-coded value with a specified number of bits
    pub fn ue_bits(&mut self, bits: usize) -> u32 {
        let mut leading_zeros = 0;

        // Calculate the number of leading zeros
        while self.get_bit() == 0 {
            leading_zeros += 1;
            // Prevent exceeding the specified bit limit
            if leading_zeros >= bits {
                return 0;
            }
        }

        // If leading_zeros is 0, then the value is 0
        if leading_zeros == 0 {
            return 0;
        }

        // Read the next 'leading_zeros' bits
        let mut value = 1;
        for _ in 0..leading_zeros {
            value = (value << 1) | self.get_bit();
        }

        value - 1
    }

    /// Get the next bit
    fn get_bit(&mut self) -> u32 {
        if self.pos >= self.data.len() * 8 {
            return 0; // All data has been read
        }

        let byte_index = self.pos / 8;
        let bit_index = 7 - (self.pos % 8); // MSB first
        let bit = (self.data[byte_index] >> bit_index) & 1;
        self.pos += 1;

        bit as u32
    }
}

/// Sample rate array containing standard AAC sample rates
const SAMPLE_RATE_ARRAY: [u32; 13] = [
    96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
];

/// Get the index of a given sample rate in the sample rate array
///
/// # Arguments
/// * `sample_rate` - The sample rate to look up
///
/// # Returns
/// * The index of the sample rate in the array, or 0x0b (11) if not found
pub fn get_sample_rate_idx(sample_rate: u32) -> u32 {
    SAMPLE_RATE_ARRAY
        .iter()
        .position(|&rate| rate == sample_rate)
        .map(|pos| pos as u32)
        .unwrap_or(0x0b)
}
