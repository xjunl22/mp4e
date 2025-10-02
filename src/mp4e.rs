// use mp4e_macros::mp4_box;
use std::io::{Cursor, Error, Seek, SeekFrom, Write};
use std::vec;

use crate::util::BitReader;

macro_rules! mp4_box {
    ($cursor:expr, $body:block) => {{
        use std::io::SeekFrom;

        let mp4_box_start_pos = ($cursor.seek(SeekFrom::Current(4))? - 4);
        $body

        let mp4_box_size = ($cursor.position() - mp4_box_start_pos) as u32;
        let mp4_box_inner = $cursor.get_mut();
        mp4_box_inner[mp4_box_start_pos as usize..mp4_box_start_pos as usize + 4]
            .copy_from_slice(&mp4_box_size.to_be_bytes());
        Ok(())
    }};
}
/// Sample type enumeration
enum SampleType {
    /// Default sample type
    Default,
    /// Random access sample (key frame)
    RandomAccess,
    /// Continuation of previous sample
    Continuation,
}

/// Codec types supported
pub enum Codec {
    /// H.264/AVC video coding NALU
    AVC,
    /// H.265/HEVC video coding NALU
    HEVC,
    /// AAC-LC audio coding
    AACLC,
    /// AAC-Main audio coding
    AACMAIN,
    /// AAC-SSR audio coding
    AACSSR,
    /// AAC-LTP audio coding
    AACLTP,
    /// HE-AAC audio coding
    HEAAC,
    /// HE-AAC-V2 audio coding
    HEAACV2,
    /// Opus audio coding
    OPUS,
}

/// Track type enumeration
enum TrackType {
    /// Video track
    Video,
    /// Audio track
    Audio,
}

/// Sample information structure
struct SampleInfo {
    /// Whether this is a random access point
    random_access: bool,
    /// Offset of the sample in the file
    offset: u64,
    /// Size of the sample
    sample_size: u32,
    /// Duration of the sample
    sample_delta: u32,
}

/// Track information structure
struct Track {
    /// Track ID
    id: u32,
    /// Total duration of the track
    duration: u32,
    /// Time scale
    timescale: u32,
    /// Sample rate (audio)
    sample_rate: u32,
    /// Number of channels (audio)
    channel_count: u32,
    /// Width (video)
    width: u32,
    /// Height (video)
    height: u32,
    /// Codec type
    codec: Codec,
    /// VPS data (HEVC video)
    vps: Option<Vec<u8>>,
    /// SPS data (video)
    sps: Option<Vec<u8>>,
    /// PPS data (video)
    pps: Option<Vec<u8>>,
    /// Audio specific configuration information
    dsi: Option<[u8; 2]>,
    /// List of sample information
    samples: Vec<SampleInfo>,
    /// Track type
    track_type: TrackType,
}

/// Main MP4 muxer structure
pub struct Mp4e<'a, Writer>
where
    Writer: Write,
{
    /// Whether to use fragmented mode
    fragment: bool,
    /// Whether the header has been initialized
    init_header: bool,
    /// Current write position in the output stream
    write_pos: u64,
    /// Creation time
    create_time: u64,
    /// Fragment ID counter
    fregment_id: u32,
    /// Total duration of the media
    duration: u32,
    /// Track ID counter
    track_ids: u32,
    /// Whether the moov box has been written
    write_moov: bool,
    /// Whether the first random access point has been sent
    send_first_random_access: bool,
    /// Language setting
    language: [u8; 3],
    /// Data writer
    writer: &'a mut Writer,
    /// Video track information
    video_track: Option<Track>,
    /// Audio track information
    audio_track: Option<Track>,
}

impl<'a, Writer> Mp4e<'a, Writer>
where
    Writer: Write + Seek,
{
    /// Creates a new MP4 muxer instance with fragmented mode disabled
    ///
    /// # Arguments
    /// * `writer` - The writer to output the MP4 data to
    ///
    /// # Returns
    /// * A new `Mp4e` instance with fragmented mode disabled
    ///
    /// # Example
    /// ```
    /// use std::io::{Cursor, Seek, Write};
    /// use mp4e::Mp4e;
    ///
    /// let mut buffer = Vec::new();
    /// let mut writer = Cursor::new(&mut buffer);
    /// let mut muxer = Mp4e::new(&mut writer);
    /// ```
    pub fn new(writer: &'a mut Writer) -> Self {
        Self::new_encoder(false, writer)
    }
}

impl<'a, Writer> Mp4e<'a, Writer>
where
    Writer: Write,
{
    /// Creates a new MP4 muxer instance with fragmented mode enabled
    ///
    /// # Arguments
    /// * `writer` - The writer to output the MP4 data to
    ///
    /// # Returns
    /// * A new `Mp4e` instance with fragmented mode and stream mode enabled
    ///
    /// # Example
    /// ```
    /// use std::io::Cursor;
    /// use mp4e::Mp4e;
    ///
    /// let mut buffer = Vec::new();
    /// let mut writer = Cursor::new(&mut buffer);
    /// let mut muxer = Mp4e::new_with_fragment(&mut writer);
    /// ```
    pub fn new_with_fragment(writer: &'a mut Writer) -> Self {
        Self::new_encoder(true, writer)
    }

    /// Sets the language for the MP4 file
    ///
    /// # Arguments
    /// * `language` - A 3-byte array representing the language code
    ///
    /// # Example
    /// ```
    /// use std::io::Cursor;
    /// use mp4e::Mp4e;
    ///
    /// let mut buffer = Vec::new();
    /// let mut writer = Cursor::new(&mut buffer);
    /// let mut muxer = Mp4e::new_with_fragment(&mut writer); // or Mp4e::new(&mut writer);
    ///
    /// // Set language to Japanese
    /// muxer.set_language([b'j', b'p', b'n']);
    /// ```
    pub fn set_language(&mut self, language: [u8; 3]) {
        self.language = language;
    }

    /// Sets the creation time for the MP4 file
    ///
    /// # Arguments
    /// * `create_time` - The creation time in seconds since Unix epoch
    ///
    /// # Example
    /// ```
    /// use std::io::Cursor;
    /// use mp4e::Mp4e;
    /// use std::time::{SystemTime, UNIX_EPOCH};
    ///
    /// let mut buffer = Vec::new();
    /// let mut writer = Cursor::new(&mut buffer);
    /// let mut muxer = Mp4e::new_with_fragment(&mut writer); // or Mp4e::new(&mut writer);
    ///
    /// // Set creation time to current time
    /// muxer.set_create_time(SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs());
    /// ```
    pub fn set_create_time(&mut self, create_time: u64) {
        self.create_time = create_time + 2082844800;
    }

    /// Sets up an audio track with the specified parameters
    ///
    /// # Arguments
    /// * `sample_rate` - The audio sample rate in Hz
    /// * `channel_count` - The number of audio channels
    /// * `codec` - The audio codec to use
    ///
    /// # Example
    /// ```
    /// use std::io::Cursor;
    /// use mp4e::{Mp4e, Codec};
    ///
    /// let mut buffer = Vec::new();
    /// let mut writer = Cursor::new(&mut buffer);
    /// let mut muxer = Mp4e::new_with_fragment(&mut writer); // or Mp4e::new(&mut writer);
    ///
    /// // Set up an AAC-LC audio track with 48kHz sample rate and 2 channels
    /// muxer.set_audio_track(48000, 2, Codec::AACLC);
    /// ```
    pub fn set_audio_track(&mut self, sample_rate: u32, channel_count: u32, codec: Codec) {
        let profile = match codec {
            Codec::AACMAIN => 1,
            Codec::AACLC => 2,
            Codec::AACSSR => 3,
            Codec::AACLTP => 4,
            Codec::HEAAC => 5,
            Codec::HEAACV2 => 29,
            _ => 0,
        };
        let mut dsi = None;
        match codec {
            Codec::OPUS => {}
            _ => {
                let mut dsi_buf: [u8; 2] = [0; 2];
                use crate::util::get_sample_rate_idx;
                let sample_rate_idx = get_sample_rate_idx(sample_rate);
                dsi_buf[0] = (profile << 3) | ((sample_rate_idx & 0x0e) >> 1) as u8;
                dsi_buf[1] = ((sample_rate_idx & 0x01) << 7) as u8 | (channel_count << 3) as u8;
                dsi = Some(dsi_buf);
            }
        }

        self.audio_track = Some(Track {
            id: self.track_ids,
            duration: 0,
            timescale: sample_rate,
            samples: vec![],
            sample_rate,
            channel_count,
            codec,
            width: 0,
            height: 0,
            dsi: dsi,
            vps: None,
            sps: None,
            pps: None,
            track_type: TrackType::Audio,
        });

        self.track_ids += 1;
    }

    /// Sets up a video track with the specified parameters
    ///
    /// # Arguments
    /// * `width` - The video width in pixels
    /// * `height` - The video height in pixels
    /// * `codec` - The video codec to use
    ///
    /// # Example
    /// ```
    /// use std::io::Cursor;
    /// use mp4e::{Mp4e, Codec};
    ///
    /// let mut buffer = Vec::new();
    /// let mut writer = Cursor::new(&mut buffer);
    /// let mut muxer = Mp4e::new_with_fragment(&mut writer); // or Mp4e::new(&mut writer);
    ///
    /// // Set up an H.264 video track with 1920x1080 resolution
    /// muxer.set_video_track(1920, 1080, Codec::AVC);
    /// ```
    pub fn set_video_track(&mut self, width: u32, height: u32, codec: Codec) {
        self.video_track = Some(Track {
            id: self.track_ids,
            duration: 0,
            timescale: 90000,
            samples: vec![],
            width,
            height,
            codec,
            sample_rate: 0,
            channel_count: 0,
            dsi: None,
            vps: None,
            sps: None,
            pps: None,
            track_type: TrackType::Video,
        });
        self.track_ids += 1;
    }

    /// Writes an audio data to the MP4 file
    ///
    /// # Arguments
    /// * `data` - The audio data
    /// * `samples` - sample count.The duration of the audio sample
    ///
    /// # Returns
    /// * `Ok(())` on success, or an error if writing fails
    ///
    /// # Example
    /// ```
    /// use std::io::Cursor;
    /// use mp4e::{Mp4e, Codec};
    ///
    /// let mut buffer = Vec::new();
    /// let mut writer = Cursor::new(&mut buffer);
    /// let mut muxer = Mp4e::new_with_fragment(&mut writer); // or Mp4e::new(&mut writer);
    ///
    /// // Set up audio track first
    /// muxer.set_audio_track(48000, 2, Codec::AACLC);
    ///
    /// // ... process video frames first to establish synchronization ...
    ///
    /// // Encode audio data with 1024 samples
    /// let audio_data = vec![0; 512]; // Example audio data
    /// muxer.encode_audio(&audio_data, 1024).unwrap();
    /// ```
    pub fn encode_audio(&mut self, data: &[u8], samples: u32) -> Result<(), Error> {
        self.init_header_if_needed()?;
        if let Some(track) = self.audio_track.as_mut() {
            if self.send_first_random_access {
                let duration = samples;
                track.duration += duration;
                self.put_sample(data, duration, false, SampleType::RandomAccess)?;
            }
        }
        Ok(())
    }

    /// Writes a video frame to the MP4 file
    ///
    /// # Arguments
    /// * `data` - The video frame data
    /// * `duration` - The duration of the video frame in milliseconds
    ///
    /// # Returns
    /// * `Ok(())` on success, or an error if writing fails
    ///
    /// # Example
    /// ```
    /// use std::io::Cursor;
    /// use mp4e::{Mp4e, Codec};
    ///
    /// let mut buffer = Vec::new();
    /// let mut writer = Cursor::new(&mut buffer);
    /// let mut muxer = Mp4e::new_with_fragment(&mut writer); // or Mp4e::new(&mut writer);
    ///
    /// // Set up video track first
    /// muxer.set_video_track(1920, 1080, Codec::AVC);
    ///
    /// // Encode a video frame with 33ms duration (approximately 30fps)
    /// let video_frame_data = vec![0; 1024]; // Example video frame data
    /// muxer.encode_video(&video_frame_data, 33).unwrap();
    /// ```
    pub fn encode_video(&mut self, data: &[u8], duration: u32) -> Result<(), Error> {
        self.init_header_if_needed()?;
        if let Some(track) = self.video_track.as_mut() {
            let duration = duration * track.timescale / 1000;
            track.duration += duration;
            self.duration = if track.duration > self.duration {
                track.duration
            } else {
                self.duration
            };
            match track.codec {
                Codec::AVC => self.write_avc_frame(data, duration)?,
                Codec::HEVC => self.write_hevc_frame(data, duration)?,
                _ => {}
            }
        }

        Ok(())
    }
}

impl<'a, Writer> Mp4e<'a, Writer>
where
    Writer: Write + Seek,
{
    /// Flushes any remaining data and finalizes the MP4 file
    ///
    /// This method ensures that all MP4 boxes are properly written to the output,
    /// including the 'moov' box which contains metadata about the file.
    ///
    /// # Returns
    /// * `Ok(())` on success, or an error if writing fails
    /// # Example
    /// ```
    /// use std::io::{Cursor, Seek, Write};
    /// use mp4e::{Mp4e, Codec};
    ///
    /// let mut buffer = Vec::new();
    /// let mut writer = Cursor::new(&mut buffer);
    /// let mut muxer = Mp4e::new(&mut writer);
    ///
    /// // ... encode audio/video data ...
    ///
    /// muxer.flush().unwrap();
    /// ```
    pub fn flush(&mut self) -> Result<(), Error> {
        self.init_header_if_needed()?;
        if !self.write_moov {
            self.write_mdat_size()?;
            self.write_moov_if_needed()?;
        }
        Ok(())
    }
}

impl<'a, Writer> Mp4e<'a, Writer>
where
    Writer: Write + Seek,
{
    /// Updates the size field of the mdat box
    ///
    /// In MP4 files, the mdat box header needs to contain the total size of the box (including the header itself).
    /// Since the final size of media data cannot be known at initialization time, this value needs to be updated
    /// after all data has been written.
    ///
    /// This implementation uses the large size format (64-bit) for the mdat box.
    fn write_mdat_size(&mut self) -> Result<(), Error> {
        // Seek to the size field position of the mdat box (mdat box starts at offset 32, size field takes first 8 bytes for large size)
        self.writer.seek(SeekFrom::Start(40))?;
        // Calculate and write the actual mdat size (write_pos is current total write position, minus 32 bytes for headers)
        // Using large size format (64-bit)
        self.writer
            .write_all(&(self.write_pos - 32).to_be_bytes())?;
        // Restore file cursor to current write position
        self.writer.seek(SeekFrom::Start(self.write_pos))?;
        Ok(())
    }
}
impl<'a, Writer> Mp4e<'a, Writer>
where
    Writer: Write,
{
    /// Creates a new MP4 encoder instance with the specified configuration
    ///
    /// This is the internal constructor used by both `new` and `new_with_fragment` methods
    /// to initialize the Mp4e struct with default values.
    ///
    /// # Arguments
    /// * `fragment` - Whether to use fragmented MP4 mode (true) or standard mode (false)
    /// * `writer` - The writer object to output the MP4 data to
    ///
    /// # Returns
    /// * A new `Mp4e` instance with initialized fields
    fn new_encoder(fragment: bool, writer: &'a mut Writer) -> Self {
        Self {
            // Current position in the output stream, starts at 0
            write_pos: 0,
            // Media creation time, defaults to 0 (will be set later if needed)
            create_time: 0,
            // Whether to use fragmented mode (true) or standard mode (false)
            fragment: fragment,
            // Fragment sequence ID counter, starts at 0
            fregment_id: 0,
            // Total media duration, starts at 0
            duration: 0,
            // Track ID counter, starts at 1 (ID 0 is reserved)
            track_ids: 1,
            // Whether the MP4 header has been initialized
            init_header: false,
            // Whether the first random access point (keyframe) has been processed
            send_first_random_access: false,
            // Whether the moov box has been written to the output
            write_moov: false,
            // Default language code ("und" = undetermined)
            language: "und".as_bytes().try_into().unwrap(),
            // The writer object for outputting MP4 data
            writer,
            // Video track information, initially empty
            video_track: None,
            // Audio track information, initially empty
            audio_track: None,
        }
    }
    /// Processes and writes HEVC (H.265) video frames to the MP4 file
    ///
    /// This function takes HEVC NAL units, parses them, and handles different types appropriately:
    /// - VPS (Video Parameter Set): Stores configuration data
    /// - SPS (Sequence Parameter Set): Stores sequence configuration data
    /// - PPS (Picture Parameter Set): Stores picture configuration data
    /// - Other NAL units: Writes as video samples when key configuration is available
    ///
    /// For HEVC, key frames are identified by specific NAL unit types in the range
    /// [HEVC_NAL_BLA_W_LP, HEVC_NAL_CRA_NUT].
    ///
    /// # Arguments
    /// * `data` - The raw HEVC NAL unit data to process
    /// * `duration` - The duration of the frame in the track's timescale
    ///
    ///
    /// # Returns
    /// * `Ok(())` on successful processing, or an error if writing fails
    fn write_hevc_frame(&mut self, data: &[u8], duration: u32) -> Result<(), Error> {
        use crate::nalu::*;
        // Split the input data into individual NAL units
        for frame_data in split_nalu(data) {
            // Extract the NAL unit type (HEVC uses 6 bits for type, shifted right by 1)
            let nalu_type = (frame_data[0] & 0x7e) >> 1;
            // Get mutable reference to the video track
            let video_track = self.video_track.as_mut().unwrap();

            match nalu_type {
                // Handle Video Parameter Set
                HEVC_NALU_TYPE_VPS => {
                    // Only store the first VPS NAL unit
                    if video_track.vps.is_none() {
                        video_track.vps = Some(frame_data.to_vec());
                    }
                }
                // Handle Sequence Parameter Set
                HEVC_NALU_TYPE_SPS => {
                    // Only store the first SPS NAL unit
                    if video_track.sps.is_none() {
                        video_track.sps = Some(frame_data.to_vec());
                    }
                }
                // Handle Picture Parameter Set
                HEVC_NALU_TYPE_PPS => {
                    // Only store the first PPS NAL unit
                    if video_track.pps.is_none() {
                        video_track.pps = Some(frame_data.to_vec());
                    }
                }
                // Handle all other NAL unit types (video data)
                _ => {
                    // Only process video data NAL units after we have the essential configuration
                    if !video_track.vps.is_none()
                        && !video_track.sps.is_none()
                        && !video_track.vps.is_none()
                    {
                        // Check if this is a key frame (Random Access Point)
                        // Key frame types are in the range [BLA_W_LP, CRA_NUT]
                        if nalu_type >= HEVC_NAL_BLA_W_LP && nalu_type <= HEVC_NAL_CRA_NUT {
                            // Write the key frame as a random access sample
                            self.put_sample(frame_data, duration, true, SampleType::RandomAccess)?;
                            // Mark that we've received our first key frame
                            self.send_first_random_access = true;
                        }
                        // For non-key frames, only write them after we've received the first key frame
                        else if self.send_first_random_access {
                            // Write as a default (non-key) sample
                            self.put_sample(frame_data, duration, true, SampleType::Default)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Processes and writes AVC (H.264) video frames to the MP4 file
    ///
    /// This function takes AVC NAL units, parses them, and handles different types appropriately:
    /// - SPS (Sequence Parameter Set): Stores sequence configuration data
    /// - PPS (Picture Parameter Set): Stores picture configuration data
    /// - Other NAL units: Writes as video samples when key configuration is available
    ///
    /// For AVC, key frames are identified by I-Slice NAL units (AVC_NAL_ISLICE_NALU).
    /// Additionally, it analyzes slice headers to determine if a NAL unit is a continuation
    /// of a previous frame or a new frame.
    ///
    /// # Arguments
    /// * `data` - The raw AVC NAL unit data to process
    /// * `duration` - The duration of the frame in the track's timescale
    ///
    /// # AVC Specifics
    /// - NAL unit types are determined by the last 5 bits of the first byte
    /// - Frame boundaries are determined by parsing the slice header using UE-Golomb decoding
    /// - The first_mb_in_slice parameter indicates if this is a new frame (0) or continuation (!=0)
    ///
    /// # Returns
    /// * `Ok(())` on successful processing, or an error if writing fails
    fn write_avc_frame(&mut self, data: &[u8], duration: u32) -> Result<(), Error> {
        use crate::nalu::*;
        // Split the input data into individual NAL units
        for frame_data in split_nalu(data) {
            // Extract the NAL unit type (AVC uses last 5 bits of the first byte)
            let nalu_type = frame_data[0] & 0x1f;
            // Get mutable reference to the video track
            let video_track = self.video_track.as_mut().unwrap();

            match nalu_type {
                // Handle Sequence Parameter Set
                AVC_NALU_TYPE_SPS => {
                    // Only store the first SPS NAL unit
                    if video_track.sps.is_none() {
                        video_track.sps = Some(frame_data.to_vec());
                    }
                }
                // Handle Picture Parameter Set
                AVC_NALU_TYPE_PPS => {
                    // Only store the first PPS NAL unit
                    if video_track.pps.is_none() {
                        video_track.pps = Some(frame_data.to_vec());
                    }
                }
                // Handle all other NAL unit types (video data including I-frames, P-frames, B-frames, etc.)
                _ => {
                    // Only process video data NAL units after we have the essential configuration (SPS and PPS)
                    if !video_track.sps.is_none() && !video_track.pps.is_none() {
                        // Default sample type is a regular frame
                        let mut sample_type = SampleType::Default;

                        // Create a bit reader to parse the slice header (starting from the second byte)
                        let mut br: BitReader<'_> = BitReader::new(&frame_data[1..]);
                        // Read the first_mb_in_slice value using UE-Golomb decoding
                        // If it's 0, this is the start of a new frame; otherwise, it's a continuation
                        let first_mb_in_slice = br.ue_bits(1);

                        // Determine the sample type based on slice header information
                        if first_mb_in_slice != 0 {
                            // This NAL unit is a continuation of the previous frame
                            sample_type = SampleType::Continuation;
                        } else if nalu_type == AVC_NAL_ISLICE_NALU {
                            // This is the start of an I-frame (key frame)
                            sample_type = SampleType::RandomAccess;
                        }

                        // Process the NAL unit based on its type
                        if nalu_type == AVC_NAL_ISLICE_NALU {
                            // For I-frames (key frames):
                            // Mark that we've received our first key frame
                            self.send_first_random_access = true;
                            // Write the frame data as a video sample
                            self.put_sample(frame_data, duration, true, sample_type)?;
                        }
                        // For non-I frames, only write them after we've received the first key frame
                        else if self.send_first_random_access {
                            // Write as a regular or continuation sample
                            self.put_sample(frame_data, duration, true, sample_type)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    // #[mp4_box]
    fn write_mvhd(&mut self, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"mvhd")?;
            if self.create_time != 0 {
                // version & flag
                cursor.write_all(&[0x01, 0x00, 0x00, 0x00])?;
                // create_time
                cursor.write_all(&self.create_time.to_be_bytes())?;
                // modify_time
                cursor.write_all(&self.create_time.to_be_bytes())?;
            } else {
                // version & flag
                cursor.write_all(&[0x00; 4])?;
                // create_time
                cursor.write_all(&[0x00; 4])?;
                // modify_time
                cursor.write_all(&[0x00; 4])?;
            }

            // timescale
            const TIMESCALE: u32 = 1000;
            let track = self.video_track.as_ref().unwrap();
            cursor.write_all(&TIMESCALE.to_be_bytes())?;
            // duration
            let duration = self.duration / (track.timescale / TIMESCALE);
            if self.create_time != 0 {
                cursor.write_all(&(duration as u64).to_be_bytes())?;
            } else {
                cursor.write_all(&(duration).to_be_bytes())?;
            }
            // Write playback rate (0x00010000 = 1.0, normal speed)
            const RATE: u32 = 0x00010000;
            cursor.write_all(&RATE.to_be_bytes())?;
            // Write playback volume (0x0100 = 1.0, full volume)
            const VOLUME: u16 = 0x0100;
            cursor.write_all(&VOLUME.to_be_bytes())?;
            // reserved
            cursor.write_all(&[0x00; 10])?;
            // Write unity matrix for video transform
            cursor.write_all(&0x00010000u32.to_be_bytes())?;
            cursor.write_all(&[0x00; 12])?;
            cursor.write_all(&0x00010000u32.to_be_bytes())?;
            cursor.write_all(&[0x00; 12])?;
            cursor.write_all(&0x40000000u32.to_be_bytes())?;
            // pre_defined
            cursor.write_all(&[0x00; 24])?;
            // next_track_id
            cursor.write_all(&self.track_ids.to_be_bytes())?;
        })
    }
    fn write_tkhd(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"tkhd")?;
            // version & flag
            cursor.write_all(&7u32.to_be_bytes())?;
            // create_time
            cursor.write_all(&[0x00; 4])?;
            // modify_time
            cursor.write_all(&[0x00; 4])?;
            // track_id
            cursor.write_all(&track.id.to_be_bytes())?;
            // reserved
            cursor.write_all(&[0x00; 4])?;
            // duration
            cursor.write_all(&(track.duration / (track.timescale / 1000)).to_be_bytes())?; //
            cursor.write_all(&[0; 12])?;
            const VOLUME: u16 = 0x0100;
            cursor.write_all(&VOLUME.to_be_bytes())?;
            // reserved
            cursor.write_all(&[0x00; 2])?;
            // matrix
            cursor.write_all(&0x00010000u32.to_be_bytes())?;
            cursor.write_all(&[0x00; 12])?;
            cursor.write_all(&0x00010000u32.to_be_bytes())?;
            cursor.write_all(&[0x00; 12])?;
            cursor.write_all(&0x40000000u32.to_be_bytes())?;
            if let TrackType::Video = track.track_type {
                cursor.write_all(&(track.width * 0x10000).to_be_bytes())?;
                cursor.write_all(&(track.height * 0x10000).to_be_bytes())?;
            } else {
                cursor.write_all(&[0x00; 8])?;
            }
        })
    }

    fn write_hdlr(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"hdlr")?;
            // version & flag
            cursor.write_all(&[0x00; 4])?;
            // pre_defined
            cursor.write_all(&[0x00; 4])?;
            if let TrackType::Video = track.track_type {
                cursor.write_all(b"vide")?;
                // reserved
                cursor.write_all(&[0x00; 12])?;
                // name
                cursor.write_all(b"VideoHandler\x00")?;
            } else {
                cursor.write_all(b"soun")?;
                // reserved
                cursor.write_all(&[0x00; 12])?;
                // name
                cursor.write_all(b"SoundHandler\x00")?;
            }
        })
    }
    fn write_mdhd(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"mdhd")?;
            // version & flag
            cursor.write_all(&[0x00; 4])?;
            // create_time
            cursor.write_all(&[0x00; 4])?;
            // modify_time
            cursor.write_all(&[0x00; 4])?;
            // timescale
            cursor.write_all(&track.timescale.to_be_bytes())?;
            // duration
            cursor.write_all(&track.duration.to_be_bytes())?;
            // language
            let lang_code: u32 = (self.language[0] as u32 & 31) << 10
                | (self.language[1] as u32 & 31) << 5
                | (self.language[2] as u32 & 31);
            cursor.write_all(&(lang_code as u16).to_be_bytes())?;
            cursor.write_all(&[0, 0])?;
        })
    }
    fn write_vmhd(&self, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"vmhd")?;
            cursor.write_all(&[0, 0, 0, 1])?;
            cursor.write_all(&[0x00; 8])?;
        })
    }
    fn write_smhd(&self, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"smhd")?;
            // version & flag
            cursor.write_all(&[0x00; 4])?;
            cursor.write_all(&[0x00; 4])?;
        })
    }
    fn write_hvcc(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"hvcC")?;
            // configurationVersion
            cursor.write_all(&[0x01])?;
            // rofile Space (2), Tier (1), Profile (5)
            cursor.write_all(&[0x01])?;
            // Profile Compatibility
            cursor.write_all(&0x60000000u32.to_be_bytes())?;
            // progressive, interlaced, non packed constraint, frame only constraint flags
            cursor.write_all(&[0x00; 2])?;
            // constraint indicator flags
            cursor.write_all(&[0; 4])?;
            // level_idc
            cursor.write_all(&[0])?;
            // Min Spatial Segmentation
            cursor.write_all(&0xf000u16.to_be_bytes())?;
            // Parallelism Type
            cursor.write_all(&[0xfc])?;
            // Chroma Format
            cursor.write_all(&[0xfc])?;
            // Luma Depth
            cursor.write_all(&[0xf8])?;
            // Chroma Depth
            cursor.write_all(&[0xf8])?;
            // Avg Frame Rate
            cursor.write_all(&[0; 2])?;
            // ConstantFrameRate (2), NumTemporalLayers (3), TemporalIdNested (1), LengthSizeMinusOne (2)
            cursor.write_all(&[0x03])?;
            // Num Of Arrays
            cursor.write_all(&[0x03])?;
            cursor.write_all(&[(1 << 7) | (32 & 0x3f)])?; //vps

            if let Some(vps) = track.vps.as_ref() {
                cursor.write_all(&[0x00, 0x01])?;
                cursor.write_all(&(vps.len() as u16).to_be_bytes())?;
                cursor.write_all(&vps[..])?;
            } else {
                cursor.write_all(&[0x00; 2])?;
            }
            cursor.write_all(&[(1 << 7) | (33 & 0x3f)])?; //sps
            if let Some(sps) = track.sps.as_ref() {
                cursor.write_all(&[0x00, 0x01])?;
                cursor.write_all(&(sps.len() as u16).to_be_bytes())?;
                cursor.write_all(&sps[..])?;
            } else {
                cursor.write_all(&[0x00; 2])?;
            }
            cursor.write_all(&[(1 << 7) | (34 & 0x3f)])?; //pps
            if let Some(pps) = track.pps.as_ref() {
                cursor.write_all(&[0x00, 0x01])?;
                cursor.write_all(&(pps.len() as u16).to_be_bytes())?;
                cursor.write_all(&pps[..])?;
            } else {
                cursor.write_all(&[0x00; 2])?;
            }
        })
    }
    fn write_avcc(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"avcC")?;
            // configurationVersion
            cursor.write_all(&[0x01])?;
            if let Some(sps) = track.sps.as_ref() {
                cursor.write_all(&sps[1..4])?;
                cursor.write_all(&[255])?;
                cursor.write_all(&[0xe0 | 1])?;
                cursor.write_all(&(sps.len() as u16).to_be_bytes())?;
                cursor.write_all(&sps[..])?;
            }
            if let Some(pps) = track.pps.as_ref() {
                cursor.write_all(&[1])?;
                cursor.write_all(&(pps.len() as u16).to_be_bytes())?;
                cursor.write_all(&pps[..])?;
            }
        })
    }
    fn write_hvc1(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"hvc1")?;
            cursor.write_all(&[0x00; 6])?;
            cursor.write_all(&[0x00, 0x01])?;
            cursor.write_all(&[0x00; 16])?;
            if let TrackType::Video = track.track_type {
                cursor.write_all(&(track.width as u16).to_be_bytes())?;
                cursor.write_all(&(track.height as u16).to_be_bytes())?;
                cursor.write_all(&0x00480000u32.to_be_bytes())?;
                cursor.write_all(&0x00480000u32.to_be_bytes())?;
                cursor.write_all(&[0x00; 4])?;
                cursor.write_all(&[0x00, 0x01])?;
                cursor.write_all(&[0x00; 32])?;
                cursor.write_all(&[0x00, 0x18])?;
                cursor.write_all(&(-1 as i16).to_be_bytes())?;
                self.write_hvcc(track, cursor)?;
            }
        })
    }
    fn write_avc1(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"avc1")?;
            cursor.write_all(&[0x00; 6])?;
            cursor.write_all(&[0x00, 0x01])?;
            cursor.write_all(&[0x00; 16])?;

            if let TrackType::Video = track.track_type {
                cursor.write_all(&(track.width as u16).to_be_bytes())?;
                cursor.write_all(&(track.height as u16).to_be_bytes())?;
                cursor.write_all(&0x00480000u32.to_be_bytes())?;
                cursor.write_all(&0x00480000u32.to_be_bytes())?;
                cursor.write_all(&[0x00; 4])?;
                cursor.write_all(&[0x00, 0x01])?;
                cursor.write_all(&[0x00; 32])?;
                cursor.write_all(&[0x00, 0x18])?;
                cursor.write_all(&(-1 as i16).to_be_bytes())?;
                self.write_avcc(track, cursor)?;
            }
        })
    }

    fn write_esds(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"esds")?;
            cursor.write_all(&[0x00; 4])?;
            let od_size_of_size = |size: u32| -> u32 {
                let mut size_of_size = 1;
                let mut i = size;
                while i > 0x7f {
                    size_of_size += 1;
                    i -= 0x7f;
                }
                size_of_size
            };
            let write_od_len = |mut size: u32, cursor: &mut Cursor<Vec<u8>>| -> Result<(), Error> {
                while size > 0x7F {
                    size -= 0x7F;
                    cursor.write_all(&[0xff])?;
                }
                cursor.write_all(&[size as u8])?;
                Ok(())
            };
            if let Some(ref dsi) = track.dsi.as_ref() {
                let dsi_bytes = dsi.len() as u32;
                let dsi_size_size = od_size_of_size(dsi_bytes);
                let dcd_bytes = dsi_bytes + dsi_size_size + 1 + (1 + 1 + 3 + 4 + 4);
                let dcd_size_size = od_size_of_size(dcd_bytes);
                let esd_bytes = dcd_bytes + dcd_size_size + 1 + 3;
                cursor.write_all(&[0x03])?;
                write_od_len(esd_bytes, cursor)?;
                cursor.write_all(&[0x00; 3])?;
                cursor.write_all(&[0x04])?;
                write_od_len(dcd_bytes, cursor)?;
                cursor.write_all(&[0x40])?;
                cursor.write_all(&[5 << 2])?;
                cursor.write_all(&[0x00])?;
                cursor.write_all(&((track.channel_count * 6144 / 8) as u16).to_be_bytes())?;
                cursor.write_all(&[0x00; 8])?;
                cursor.write_all(&[0x05])?;
                write_od_len(dsi_bytes, cursor)?;
                cursor.write_all(&dsi[..])?;
            }
        })
    }
    fn write_mp4a(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"mp4a")?;
            cursor.write_all(&[0x00; 4])?;
            cursor.write_all(&[0x00; 2])?;
            cursor.write_all(&[0x00, 0x01])?;

            if let TrackType::Audio = track.track_type {
                cursor.write_all(&[0x00; 8])?;
                cursor.write_all(&(track.channel_count as u16).to_be_bytes())?;
                cursor.write_all(&[0x00, 0x10])?; //16 bits per sample
                cursor.write_all(&[0x00; 4])?;
                cursor.write_all(&(track.sample_rate << 16).to_be_bytes())?;
                self.write_esds(track, cursor)?;
            }
        })
    }
    fn write_dops(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"dops")?;
            cursor.write_all(&[0x00])?;
            cursor.write_all(&(track.channel_count as u16).to_be_bytes())?;
            cursor.write_all(&[0x00; 2])?;
            cursor.write_all(&track.sample_rate.to_be_bytes())?;
            cursor.write_all(&[0x00; 2])?;
            cursor.write_all(&[0x00])?;
        })
    }
    fn write_opus(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"opus")?;
            cursor.write_all(&[0x00; 4])?;
            cursor.write_all(&[0x00; 2])?;
            cursor.write_all(&[0x00, 0x01])?;
            if let TrackType::Audio = track.track_type {
                cursor.write_all(&[0x00; 8])?;
                cursor.write_all(&(track.channel_count as u16).to_be_bytes())?;
                cursor.write_all(&[0x00, 0x10])?; //16 bits per sample
                cursor.write_all(&[0x00; 4])?;
                cursor.write_all(&(track.sample_rate << 16).to_be_bytes())?;
                self.write_dops(track, cursor)?;
            }
        })
    }
    fn write_stsd(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"stsd")?;
            cursor.write_all(&[0x00; 4])?;
            cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
            if let TrackType::Video = track.track_type {
                match track.codec {
                    Codec::HEVC => {
                        self.write_hvc1(track, cursor)?;
                    }
                    Codec::AVC => {
                        self.write_avc1(track, cursor)?;
                    }
                    _ => {}
                }
            } else {
                match track.codec {
                    Codec::AACLC
                    | Codec::AACMAIN
                    | Codec::AACSSR
                    | Codec::AACLTP
                    | Codec::HEAAC
                    | Codec::HEAACV2 => {
                        self.write_mp4a(track, cursor)?;
                    }
                    Codec::OPUS => {
                        //
                        self.write_opus(track, cursor)?;
                    }
                    _ => {}
                }
            }
        })
    }
    fn write_stsc(&self, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"stsc")?;
            cursor.write_all(&[0x00; 4])?;
            if self.fragment {
                cursor.write_all(&[0x00; 4])?;
            } else {
                cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
                cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
                cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
                cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
            }
        })
    }
    fn write_stsz(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"stsz")?;
            cursor.write_all(&[0x00; 4])?;
            cursor.write_all(&[0x00; 4])?;
            cursor.write_all(&(track.samples.len() as u32).to_be_bytes())?;
            for sample in track.samples.iter() {
                cursor.write_all(&sample.sample_size.to_be_bytes())?;
            }
        })
    }
    fn write_stts(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"stts")?;
            cursor.write_all(&[0x00; 4])?;
            // entry count
            // cursor.write_all(&(track.samples.len() as u32).to_be_bytes())?;
            let entry_count_idx = cursor.position();
            cursor.seek(SeekFrom::Current(4))?;
            let mut entry_count: u32 = 0;
            let mut cnt: u32 = 1;
            for i in 0..track.samples.len() {
                if i == track.samples.len() - 1
                    || track.samples[i].sample_delta != track.samples[i + 1].sample_delta
                {
                    cursor.write_all(&cnt.to_be_bytes())?;
                    cursor.write_all(&track.samples[i].sample_delta.to_be_bytes())?;
                    cnt = 0;
                    entry_count += 1;
                }
                cnt += 1;
            }
            let end_pos = cursor.position();
            cursor.seek(SeekFrom::Start(entry_count_idx))?;
            cursor.write_all(&entry_count.to_be_bytes())?;
            cursor.seek(SeekFrom::Start(end_pos))?;
        })
    }
    fn write_stss(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"stss")?;
            cursor.write_all(&[0x00; 4])?;
            let entry_point = cursor.position();
            let mut random_access_count = 0 as u32;
            for (i, sample) in track.samples.iter().enumerate() {
                if sample.random_access {
                    cursor.write_all(&(i as u32 + 1).to_be_bytes())?;
                    random_access_count += 1;
                }
            }
            let end_pos = cursor.position();
            cursor.seek(SeekFrom::Start(entry_point + 12)).unwrap();
            cursor.write_all(&random_access_count.to_be_bytes())?;
            cursor.seek(SeekFrom::Start(end_pos))?;
        })
    }
    fn write_stbl(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"stbl")?;
            self.write_stsd(track, cursor)?;
            self.write_stts(track, cursor)?;
            self.write_stsc(cursor)?;
            self.write_stsz(track, cursor)?;
            if track.samples.len() > 0 {
                let last_sample = track.samples.last().unwrap();
                if last_sample.offset > 0xffffffff {
                    self.write_co64(track, cursor)?;
                } else {
                    self.write_stco(track, cursor)?;
                }
            }
            if let TrackType::Video = track.track_type {
                //stss
                self.write_stss(track, cursor)?;
            }
        })
    }
    fn write_co64(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"co64")?;
            cursor.write_all(&[0x00; 4])?;
            cursor.write_all(&(track.samples.len() as u32).to_be_bytes())?;
            for sample in track.samples.iter() {
                cursor.write_all(&sample.offset.to_be_bytes())?;
            }
        })
    }
    fn write_stco(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"stco")?;
            cursor.write_all(&[0x00; 4])?;
            cursor.write_all(&(track.samples.len() as u32).to_be_bytes())?;
            for sample in track.samples.iter() {
                cursor.write_all(&(sample.offset as u32).to_be_bytes())?;
            }
        })
    }
    fn write_url(&self, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"url ")?;
            cursor.write_all(&[0, 0, 0, 1])?;
        })
    }
    fn write_dref(&self, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"dref")?;
            // version & flag
            cursor.write_all(&[0x00; 4])?;

            cursor.write_all(b"\x00\x00\x00\x01")?;
            self.write_url(cursor)?;
        })
    }
    fn write_dinf(&self, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"dinf")?;
            self.write_dref(cursor)?;
        })
    }
    fn write_minf(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"minf")?;
            match track.track_type {
                TrackType::Video => {
                    self.write_vmhd(cursor)?;
                }
                TrackType::Audio => {
                    self.write_smhd(cursor)?;
                }
            }
            self.write_dinf(cursor)?;
            self.write_stbl(track, cursor)?;
        })
    }
    fn write_mdia(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"mdia")?;
            self.write_mdhd(track, cursor)?;
            self.write_hdlr(track, cursor)?;
            self.write_minf(track, cursor)?;
        })
    }
    fn write_track(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"trak")?;
            self.write_tkhd(track, cursor)?;
            self.write_mdia(track, cursor)?;
        })
    }

    fn write_tracks(&mut self, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        for track in [&self.video_track, &self.audio_track].iter() {
            if let Some(track) = track.as_ref() {
                self.write_track(track, cursor)?;
            }
        }
        Ok(())
    }
    fn write_trex(&self, track: &Track, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"trex\x00\x00\x00\x00")?;
            cursor.write_all(&track.id.to_be_bytes())?;
            cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
            cursor.write_all(&[0x00; 12])?;
        })
    }
    fn write_trexs(&mut self, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        for track in [&self.video_track, &self.audio_track].iter() {
            if let Some(track) = track.as_ref() {
                self.write_trex(track, cursor)?;
            }
        }
        Ok(())
    }

    fn write_mvex(&mut self, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"mvex")?;
            self.write_trexs(cursor)?;
        })
    }
    fn write_moov(&mut self, cursor: &mut Cursor<Vec<u8>>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"moov")?;
            self.write_mvhd(cursor)?;
            self.write_tracks(cursor)?;
            if self.fragment {
                self.write_mvex(cursor)?;
            }
        })
    }
    fn write_tfhd(
        &mut self,
        video: bool,
        sample_duration: u32,
        cursor: &mut Cursor<&mut [u8]>,
    ) -> Result<(), Error> {
        mp4_box!(cursor, {
            let track = if video {
                self.video_track.as_ref().unwrap()
            } else {
                self.audio_track.as_ref().unwrap()
            };
            cursor.write_all(b"tfhd")?;
            if let TrackType::Video = track.track_type {
                cursor.write_all(&0x20020u32.to_be_bytes())?;
                cursor.write_all(&track.id.to_be_bytes())?;
                cursor.write_all(&0x1010000u32.to_be_bytes())?;
            } else {
                cursor.write_all(&0x20008u32.to_be_bytes())?;
                cursor.write_all(&track.id.to_be_bytes())?;
                cursor.write_all(&sample_duration.to_be_bytes())?;
            }
        })
    }
    fn write_trun(
        &mut self,
        moof_pos: u64,
        video: bool,
        data_size: u32,
        sample_duration: u32,
        sample_type: SampleType,
        cursor: &mut Cursor<&mut [u8]>,
    ) -> Result<(), Error> {
        mp4_box!(cursor, {
            let track = if video {
                self.video_track.as_ref().unwrap()
            } else {
                self.audio_track.as_ref().unwrap()
            };
            cursor.write_all(b"trun")?;
            let data_offset_pos;
            if let TrackType::Video = track.track_type {
                if let SampleType::RandomAccess = sample_type {
                    let flags: u32 = 0x001 | 0x004 | 0x100 | 0x200;
                    cursor.write_all(&flags.to_be_bytes())?;
                    cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
                    data_offset_pos = cursor.position();
                    cursor.seek(SeekFrom::Current(4))?;
                    cursor.write_all(&0x2000000u32.to_be_bytes())?;
                    cursor.write_all(&sample_duration.to_be_bytes())?;
                    cursor.write_all(&data_size.to_be_bytes())?;
                } else {
                    let flags: u32 = 0x001 | 0x100 | 0x200;
                    cursor.write_all(&flags.to_be_bytes())?;
                    cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
                    data_offset_pos = cursor.position();
                    cursor.seek(SeekFrom::Current(4))?;
                    cursor.write_all(&sample_duration.to_be_bytes())?;
                    cursor.write_all(&data_size.to_be_bytes())?;
                }
            } else {
                let flags: u32 = 0x001 | 0x200;
                cursor.write_all(&flags.to_be_bytes())?;
                cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
                data_offset_pos = cursor.position();
                cursor.seek(SeekFrom::Current(4))?;
                cursor.write_all(&data_size.to_be_bytes())?;
            }
            let end_pos = cursor.position();
            let data_offset = (end_pos - moof_pos + 8) as u32;
            cursor.seek(SeekFrom::Start(data_offset_pos))?;
            cursor.write_all(&data_offset.to_be_bytes())?;
            cursor.seek(SeekFrom::Start(end_pos)).unwrap();
        })
    }
    fn write_traf(
        &mut self,
        moof_pos: u64,
        video: bool,
        data: &[u8],
        sample_duration: u32,
        sample_type: SampleType,
        cursor: &mut Cursor<&mut [u8]>,
    ) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"traf")?;
            self.write_tfhd(video, sample_duration, cursor)?;
            self.write_trun(
                moof_pos,
                video,
                data.len() as u32 + 4,
                sample_duration,
                sample_type,
                cursor,
            )?;
        })
    }
    fn write_mfhd(&mut self, cursor: &mut Cursor<&mut [u8]>) -> Result<(), Error> {
        mp4_box!(cursor, {
            cursor.write_all(b"mfhd\x00\x00\x00\x00")?;
            cursor.write_all(&self.fregment_id.to_be_bytes())?;
        })
    }

    fn write_moof(
        &mut self,
        data: &[u8],
        duration: u32,
        video: bool,
        sample_type: SampleType,
        cursor: &mut Cursor<&mut [u8]>,
    ) -> Result<(), Error> {
        mp4_box!(cursor, {
            let moov_pos = cursor.position() - 4;
            cursor.write_all(b"moof")?;
            self.write_mfhd(cursor)?;
            self.write_traf(moov_pos, video, data, duration, sample_type, cursor)?;
        })
    }

    fn write_mdat(&mut self, buf: &[u8], video: bool) -> Result<(), Error> {
        let mut box_size = buf.len() as u32 + 8;
        if video {
            box_size += 4;
        }
        self.writer.write_all(&box_size.to_be_bytes())?;
        self.writer.write_all(b"mdat")?;
        if video {
            let nal_size_buf = (buf.len() as u32).to_be_bytes();
            self.writer.write_all(&nal_size_buf)?;
        }
        self.writer.write_all(buf)?;

        self.write_pos += box_size as u64;

        Ok(())
    }
    fn init_mp4(&mut self) -> Result<(), Error> {
        self.writer
            .write_all(b"\x00\x00\x00\x18ftypmp42\x00\x00\x00\x00mp42isom\x00\x00\x00\x08free")?;
        self.write_pos += 32;
        if !self.fragment {
            // mdat
            self.writer
                .write_all(b"\x00\x00\x00\x01mdat\x00\x00\x00\x00\x00\x00\x00\x10")?;
            self.write_pos += 16;
        }
        Ok(())
    }
    fn put_sample(
        &mut self,
        data: &[u8],
        duration: u32,
        video: bool,
        sample_type: SampleType,
    ) -> Result<(), Error> {
        if self.fragment {
            self.write_moov_if_needed()?;
            self.fregment_id += 1;
            let mut buf: [u8; 4096] = [0; 4096];
            let mut cursor = Cursor::new(&mut buf[..]);
            self.write_moof(data, duration, video, sample_type, &mut cursor)?;
            let end_pos = cursor.position();
            self.writer.write_all(&buf[..end_pos as usize])?;
            self.write_pos += end_pos as u64;
            self.write_mdat(data, video)?;
            return Ok(());
        }
        if !video {
            let sample_info = SampleInfo {
                random_access: true,
                offset: self.write_pos,
                sample_size: data.len() as u32,
                sample_delta: duration,
            };
            self.audio_track.as_mut().unwrap().samples.push(sample_info);
            self.writer.write_all(data)?;
            self.write_pos += data.len() as u64;
        } else {
            if let SampleType::Default | SampleType::RandomAccess = sample_type {
                let sample_info = SampleInfo {
                    random_access: if let SampleType::RandomAccess = sample_type {
                        true
                    } else {
                        false
                    },
                    offset: self.write_pos,
                    sample_size: data.len() as u32 + 4,
                    sample_delta: duration,
                };
                self.video_track.as_mut().unwrap().samples.push(sample_info);
            } else {
                let samples = &mut self.video_track.as_mut().unwrap().samples;
                let last_sample = samples.last_mut().unwrap();
                last_sample.sample_size += data.len() as u32 + 4;
            }
            let nal_size_buf = (data.len() as u32).to_be_bytes();
            self.writer.write_all(&nal_size_buf[..])?;
            self.writer.write_all(data)?;
            self.write_pos += data.len() as u64 + 4;
        }

        Ok(())
    }

    fn init_header_if_needed(&mut self) -> Result<(), Error> {
        if !self.init_header {
            self.init_mp4()?;
            self.init_header = true;
        }
        Ok(())
    }
    fn write_moov_if_needed(&mut self) -> Result<(), Error> {
        if !self.write_moov {
            let mut cursor: Cursor<Vec<u8>> = Cursor::new(Vec::new());
            self.write_moov(&mut cursor)?;
            let end_pos = cursor.position();
            let buf = cursor.into_inner();
            self.writer.write_all(&buf[..end_pos as usize])?;
            self.write_pos += end_pos;
            self.write_moov = true;
        }
        Ok(())
    }
}
