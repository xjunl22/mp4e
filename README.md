# mp4e

A simple MP4 muxer library for pure Rust.

## Introduction

`mp4e` is a simple MP4 muxing library that allows you to create standard MP4 files and fragmented MP4 (fMP4) files. It supports both H.264/AVC and H.265/HEVC video codecs as well as AAC and Opus audio codecs.

## Implementation Reference

This library's implementation is partially inspired by [minimp4](https://github.com/lieff/minimp4).

## License

This project is licensed under the MIT License.

## Usage Examples

Add this to your Cargo.toml:

```toml
[dependencies]
mp4e = "0.9"
```

### Creating a Standard MP4 File

```rust
use std::fs::File;
use std::io::BufWriter;
use mp4e::{Mp4e, Codec};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create output file
    let file = File::create("output.mp4")?;
    let mut writer = BufWriter::new(file);
    
    // Create MP4 muxer
    let mut muxer = Mp4e::new(&mut writer);
    
    // Set up video track (H.264, 1920x1080)
    muxer.set_video_track(1920, 1080, Codec::AVC);
    
    // Set up audio track (AAC, 48kHz, stereo)
    muxer.set_audio_track(48000, 2, Codec::AACLC);
    
    // Write video frame data (assuming you have encoded NALU data)
    let video_frame_data = vec![/* your video frame data */];
    muxer.encode_video(&video_frame_data, 33)?; // 33ms per frame (~30fps)
    
    // Write audio frame data
    let audio_frame_data = vec![/* your audio frame data */];
    muxer.encode_audio(&audio_frame_data, 1024)?; // 1024 samples
    
    // Finish writing
    muxer.flush()?;
    
    Ok(())
}
```

### Creating a Fragmented MP4 File (fMP4)

```rust
use std::fs::File;
use std::io::BufWriter;
use mp4e::{Mp4e, Codec};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create output file
    let file = File::create("output.m4s")?;
    let mut writer = BufWriter::new(file);
    
    // Create fragmented MP4 muxer
    let mut muxer = Mp4e::new_with_fragment(&mut writer);
    
    // Set up video track
    muxer.set_video_track(1920, 1080, Codec::HEVC);
    
    // Set up audio track
    muxer.set_audio_track(48000, 2, Codec::AACLC);
    
    // Write media data
    let video_data = vec![/* video data */];
    muxer.encode_video(&video_data, 33)?;
    
    let audio_data = vec![/* audio data */];
    muxer.encode_audio(&audio_data, 1024)?;
    
    // Finish writing
    muxer.flush()?;
    
    Ok(())
}
```

## Supported Codecs

### Video Codecs
- H.264/AVC
- H.265/HEVC

## Limitations

The current version of mp4e only supports the following track configuration:
- Up to 1 video track
- Up to 1 audio track (optional)

### Audio Codecs
- AAC-LC
- AAC-Main
- AAC-SSR
- AAC-LTP
- HE-AAC
- HE-AAC-v2
- Opus