/// Sample type enumeration
pub enum SampleType {
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
pub enum TrackType {
    /// Video track
    Video,
    /// Audio track
    Audio,
}

/// Sample information structure
pub struct SampleInfo {
    /// Whether this is a random access point
    pub random_access: bool,
    /// Offset of the sample in the file
    pub offset: u64,
    /// Size of the sample
    pub sample_size: u32,
    /// Duration of the sample
    pub sample_delta: u32,
    // Continuation offset
    pub sample_ct_offset: i32,
}

/// Track information structure
pub struct Track {
    /// Track ID
    pub id: u32,
    /// Total duration of the track
    pub duration: u32,
    /// Time scale
    pub timescale: u32,
    /// Sample rate (audio)
    pub sample_rate: u32,
    /// Number of channels (audio)
    pub channel_count: u32,
    /// Width (video)
    pub width: u32,
    /// Height (video)
    pub height: u32,
    /// Codec type
    pub codec: Codec,
    /// VPS data (HEVC video)
    pub vps: Option<Vec<u8>>,
    /// SPS data (video)
    pub sps: Option<Vec<u8>>,
    /// PPS data (video)
    pub pps: Option<Vec<u8>>,
    /// Audio specific configuration information
    pub dsi: Option<[u8; 2]>,
    /// List of sample information
    pub samples: Vec<SampleInfo>,
    /// Track type
    pub track_type: TrackType,
}
