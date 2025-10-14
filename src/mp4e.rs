// use mp4e_macros::mp4_box;
use crate::boxes::*;
use crate::types::*;
use std::convert::TryInto;
use std::io::{Cursor, Error, Seek, SeekFrom, Write};
use std::vec;

use crate::util::BitReader;

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
    fragment_id: u32,
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
    /// * `samples` - The number of audio samples in this frame. This represents
    ///               the duration in sample count, not bytes. For example, if you
    ///               have 1024 PCM samples that were encoded, you pass 1024 here.
    ///               If you only know the duration in milliseconds, you can estimate
    ///               the sample count using the formula: duration_ms * sample_rate / 1000.
    ///               For example, with a 48kHz sample rate and 21.33ms duration:
    ///               samples = 21.33 * 48000 / 1000 = 1024 samples.
    ///               
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
                self.put_sample(data, duration, false, 0, SampleType::RandomAccess)?;
            }
        }
        Ok(())
    }

    /// Writes a video frame to the MP4 file (with no b frame)
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
                Codec::AVC => self.write_avc_frame(data, duration, 0)?,
                Codec::HEVC => self.write_hevc_frame(data, duration, 0)?,
                _ => {}
            }
        }

        Ok(())
    }
    /// Writes a video frame to the MP4 file with presentation timestamp (PTS)，support b frame
    ///
    /// This method allows for more precise control over video frame timing by accepting
    /// a presentation timestamp. It calculates the composition time offset (ct_offset)
    /// which represents the difference between decode time and presentation time.
    ///
    /// # Arguments
    /// * `data` - The video frame data (NAL units)
    /// * `duration` - The duration of the video frame in milliseconds
    /// * `pts` - Presentation timestamp in the track's timescale
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
    /// let mut muxer = Mp4e::new(&mut writer);
    ///
    /// // Set up video track first
    /// muxer.set_video_track(1920, 1080, Codec::AVC);
    ///
    /// // Encode a video frame with specific PTS
    /// let video_frame_data = vec![0; 1024]; // Example video frame data
    /// muxer.encode_video_with_pts(&video_frame_data, 33, 1000).unwrap();
    /// ```
    pub fn encode_video_with_pts(
        &mut self,
        data: &[u8],
        duration: u32,
        pts: u32,
    ) -> Result<(), Error> {
        self.init_header_if_needed()?;
        if let Some(track) = self.video_track.as_mut() {
            // Convert duration from milliseconds to track timescale
            let duration = duration * track.timescale / 1000;
            track.duration += duration;

            // Update the overall media duration if this track is longer
            self.duration = if track.duration > self.duration {
                track.duration
            } else {
                self.duration
            };

            // Calculate composition time offset (decode time to presentation time offset)
            let ct_offset =
                ((pts as i64) * track.timescale as i64 / 1000 - track.duration as i64) as i32;

            // Process the frame based on codec type
            match track.codec {
                Codec::AVC => self.write_avc_frame(data, duration, ct_offset)?,
                Codec::HEVC => self.write_hevc_frame(data, duration, ct_offset)?,
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
            fragment_id: 0,
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
    /// * `ct_offset` - The composition time offset for the frame
    ///
    ///
    /// # Returns
    /// * `Ok(())` on successful processing, or an error if writing fails
    fn write_hevc_frame(
        &mut self,
        data: &[u8],
        duration: u32,
        ct_offset: i32,
    ) -> Result<(), Error> {
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
                            self.put_sample(
                                frame_data,
                                duration,
                                true,
                                ct_offset,
                                SampleType::RandomAccess,
                            )?;
                            // Mark that we've received our first key frame
                            self.send_first_random_access = true;
                        }
                        // For non-key frames, only write them after we've received the first key frame
                        else if self.send_first_random_access {
                            // Write as a default (non-key) sample
                            self.put_sample(
                                frame_data,
                                duration,
                                true,
                                ct_offset,
                                SampleType::Default,
                            )?;
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
    /// * `ct_offset` - The composition time offset for the frame
    ///
    /// # AVC Specifics
    /// - NAL unit types are determined by the last 5 bits of the first byte
    /// - Frame boundaries are determined by parsing the slice header using UE-Golomb decoding
    /// - The first_mb_in_slice parameter indicates if this is a new frame (0) or continuation (!=0)
    ///
    /// # Returns
    /// * `Ok(())` on successful processing, or an error if writing fails
    fn write_avc_frame(&mut self, data: &[u8], duration: u32, ct_offset: i32) -> Result<(), Error> {
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
                            self.put_sample(frame_data, duration, true, ct_offset, sample_type)?;
                        }
                        // For non-I frames, only write them after we've received the first key frame
                        else if self.send_first_random_access {
                            // Write as a regular or continuation sample
                            self.put_sample(frame_data, duration, true, ct_offset, sample_type)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn init_mp4(&mut self) -> Result<(), Error> {
        self.write_pos += write_ftyp(self.writer)?;
        if !self.fragment {
            self.write_pos += write_mdat_header(self.writer)?;
        }
        Ok(())
    }
    fn put_sample(
        &mut self,
        data: &[u8],
        duration: u32,
        video: bool,
        ct_offset: i32,
        sample_type: SampleType,
    ) -> Result<(), Error> {
        if self.fragment {
            self.write_moov_if_needed()?;
            self.fragment_id += 1;
            let mut buf: [u8; 4096] = [0; 4096];
            let mut cursor = Cursor::new(&mut buf[..]);
            write_moof(
                self.fragment_id,
                data,
                duration,
                if video {
                    self.video_track.as_ref().unwrap()
                } else {
                    self.audio_track.as_ref().unwrap()
                },
                ct_offset,
                sample_type,
                &mut cursor,
            )?;
            let end_pos = cursor.position();
            self.writer.write_all(&buf[..end_pos as usize])?;
            self.write_pos += end_pos as u64;
            let box_size = write_mdat(data, video, self.writer)?;
            self.write_pos += box_size as u64;
            return Ok(());
        }
        if !video {
            let sample_info = SampleInfo {
                random_access: true,
                offset: self.write_pos,
                sample_size: data.len() as u32,
                sample_delta: duration,
                sample_ct_offset: ct_offset,
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
                    sample_ct_offset: ct_offset,
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
            write_moov(
                &self.video_track,
                &self.audio_track,
                self.create_time,
                self.track_ids,
                &self.language,
                self.fragment,
                &mut cursor,
            )?;
            let end_pos = cursor.position();
            let buf = cursor.into_inner();
            self.writer.write_all(&buf[..end_pos as usize])?;
            self.write_pos += end_pos;
            self.write_moov = true;
        }
        Ok(())
    }
}
