# mp4e

一个简单的 纯Rust MP4 复用器库。

## 简介

`mp4e` 是一个简单的 MP4 复用器库，可以用来创建标准 MP4 文件和分段 MP4 (fMP4) 文件。它支持 H.264/AVC 和 H.265/HEVC 视频编解码器，以及 AAC 和 Opus 音频编解码器。

## 实现参考

本库的部分实现参考了 [minimp4](https://github.com/lieff/minimp4) 项目。

## 许可证

本项目采用 MIT 许可证。

## 使用示例

将以下内容添加到你的 Cargo.toml:

```toml
[dependencies]
mp4e = "1.0"
```

### 创建标准 MP4 文件

```rust
use std::fs::File;
use std::io::BufWriter;
use mp4e::{Mp4e, Codec};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建输出文件
    let file = File::create("output.mp4")?;
    let mut writer = BufWriter::new(file);
    
    // 创建 MP4 复用器
    let mut muxer = Mp4e::new(&mut writer);
    
    // 设置视频轨道 (H.264, 1920x1080)
    muxer.set_video_track(1920, 1080, Codec::AVC);
    
    // 设置音频轨道 (AAC, 48kHz, 立体声)
    muxer.set_audio_track(48000, 2, Codec::AACLC);
    
    // 写入视频帧数据 (假设你已经有了编码好的 NALU 数据)
    let video_frame_data = vec![/* 你的视频帧数据 */];
    muxer.encode_video(&video_frame_data, 33)?; // 每帧 33ms (~30fps) 或者encode_video_with_pts
    
    // 写入音频帧数据
    let audio_frame_data = vec![/* 你的音频帧数据 */];
    muxer.encode_audio(&audio_frame_data, 1024)?; // 1024 个采样点
    
    // 完成写入
    muxer.flush()?;
    
    Ok(())
}
```

### 创建分段 MP4 文件 (fMP4)

```rust
use std::fs::File;
use std::io::BufWriter;
use mp4e::{Mp4e, Codec};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 创建输出文件
    let file = File::create("output.m4s")?;
    let mut writer = BufWriter::new(file);
    
    // 创建分段 MP4 复用器
    let mut muxer = Mp4e::new_with_fragment(&mut writer);
    
    // 设置视频轨道
    muxer.set_video_track(1920, 1080, Codec::HEVC);
    
    // 设置音频轨道
    muxer.set_audio_track(48000, 2, Codec::AACLC);
    
    // 写入媒体数据
    let video_data = vec![/* 视频数据 */];
    muxer.encode_video(&video_data, 33)?;
    
    let audio_data = vec![/* 音频数据 */];
    muxer.encode_audio(&audio_data, 1024)?;
    
    // 完成写入
    muxer.flush()?;
    
    Ok(())
}
```

## 支持的编解码器

### 视频编解码器
- H.264/AVC
- H.265/HEVC



### 音频编解码器
- AAC-LC
- AAC-Main
- AAC-SSR
- AAC-LTP
- HE-AAC
- HE-AAC-v2
- Opus


## 功能限制

当前版本的 mp4e 仅支持以下轨道配置：
- 最多1个视频轨道
- 最多1个音频轨道（可选）
- 仅支持原始样本位深度为 16 的音频数据（PCM 16-bit）
