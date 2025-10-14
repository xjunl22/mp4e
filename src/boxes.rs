use crate::types::{Codec, SampleInfo, SampleType, Track, TrackType};
use std::io::{Error, Seek, Write};

macro_rules! mp4_box {
    ($cursor:expr, $box_name:expr, $body:block) => {{
        use std::io::SeekFrom;
        let mp4_box_start_pos = ($cursor.seek(SeekFrom::Current(4))? - 4);
        $cursor.write_all($box_name)?;
        $body
        let end_pos = $cursor.stream_position()?;
        let mp4_box_size = (end_pos - mp4_box_start_pos) as u32;
        $cursor.seek(SeekFrom::Start(mp4_box_start_pos))?;
        $cursor.write_all(&mp4_box_size.to_be_bytes())?;
        $cursor.seek(SeekFrom::Start(end_pos))?;
        Ok(())
    }};

}

fn write_hdlr<Writer>(video: bool, cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"hdlr", {
        // version & flag
        cursor.write_all(&[0x00; 4])?;
        // pre_defined
        cursor.write_all(&[0x00; 4])?;
        if video {
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

fn write_vmhd<Writer>(cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"vmhd", {
        cursor.write_all(&[0, 0, 0, 1])?;
        cursor.write_all(&[0x00; 8])?;
    })
}

fn write_smhd<Writer>(cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"smhd", {
        // version & flag
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(&[0x00; 4])?;
    })
}

fn write_stsc<Writer>(fragment: bool, cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"stsc", {
        cursor.write_all(&[0x00; 4])?;
        if fragment {
            cursor.write_all(&[0x00; 4])?;
        } else {
            cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
            cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
            cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
            cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
        }
    })
}

fn write_url<Writer>(cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"url ", {
        cursor.write_all(&[0, 0, 0, 1])?;
    })
}

fn write_dref<Writer>(cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"dref", {
        // version & flag
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(b"\x00\x00\x00\x01")?;
        write_url(cursor)?;
    })
}

fn write_dinf<Writer>(cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"dinf", {
        write_dref(cursor)?;
    })
}

fn write_mfhd<Writer>(fragment_id: u32, cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"mfhd", {
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(&fragment_id.to_be_bytes())?;
    })
}

fn write_dops<Writer>(
    channel_count: u32,
    sample_rate: u32,
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"dops", {
        cursor.write_all(&[0x00])?;
        cursor.write_all(&(channel_count as u16).to_be_bytes())?;
        cursor.write_all(&[0x00; 2])?;
        cursor.write_all(&sample_rate.to_be_bytes())?;
        cursor.write_all(&[0x00; 2])?;
        cursor.write_all(&[0x00])?;
    })
}

fn write_stsz<Writer>(samples: &[SampleInfo], cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"stsz", {
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(&(samples.len() as u32).to_be_bytes())?;
        for sample in samples.iter() {
            cursor.write_all(&sample.sample_size.to_be_bytes())?;
        }
    })
}

fn write_stss<Writer>(samples: &[SampleInfo], cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"stss", {
        cursor.write_all(&[0x00; 4])?;
        let entry_point = cursor.stream_position()?;
        cursor.seek(SeekFrom::Current(4))?;
        let mut random_access_count = 0 as u32;
        for (i, sample) in samples.iter().enumerate() {
            if sample.random_access {
                cursor.write_all(&(i as u32 + 1).to_be_bytes())?;
                random_access_count += 1;
            }
        }
        let end_pos = cursor.stream_position()?;
        cursor.seek(SeekFrom::Start(entry_point)).unwrap();
        cursor.write_all(&random_access_count.to_be_bytes())?;
        cursor.seek(SeekFrom::Start(end_pos))?;
    })
}

fn write_co64<Writer>(samples: &[SampleInfo], cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"co64", {
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(&(samples.len() as u32).to_be_bytes())?;
        for sample in samples.iter() {
            cursor.write_all(&sample.offset.to_be_bytes())?;
        }
    })
}

fn write_stco<Writer>(samples: &[SampleInfo], cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"stco", {
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(&(samples.len() as u32).to_be_bytes())?;
        for sample in samples.iter() {
            cursor.write_all(&(sample.offset as u32).to_be_bytes())?;
        }
    })
}

fn write_mdhd<Writer>(
    timescale: u32,
    duration: u32,
    language: &[u8; 3],
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"mdhd", {
        // version & flag
        cursor.write_all(&[0x00; 4])?;
        // create_time
        cursor.write_all(&[0x00; 4])?;
        // modify_time
        cursor.write_all(&[0x00; 4])?;
        // timescale
        cursor.write_all(&timescale.to_be_bytes())?;
        // duration
        cursor.write_all(&duration.to_be_bytes())?;
        // language
        let lang_code: u32 = (language[0] as u32 & 31) << 10
            | (language[1] as u32 & 31) << 5
            | (language[2] as u32 & 31);
        cursor.write_all(&(lang_code as u16).to_be_bytes())?;
        cursor.write_all(&[0, 0])?;
    })
}

fn write_trex<Writer>(track_id: u32, cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"trex", {
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(&track_id.to_be_bytes())?;
        cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
        cursor.write_all(&[0x00; 12])?;
    })
}

fn write_opus<Writer>(
    channel_count: u32,
    sample_rate: u32,
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"opus", {
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(&[0x00; 2])?;
        cursor.write_all(&[0x00, 0x01])?;
        cursor.write_all(&[0x00; 8])?;
        cursor.write_all(&(channel_count as u16).to_be_bytes())?;
        cursor.write_all(&[0x00, 0x10])?; //16 bits per sample
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(&(sample_rate << 16).to_be_bytes())?;
        write_dops(channel_count, sample_rate, cursor)?;
    })
}

fn write_esds<Writer>(
    channel_count: u32,
    dsi: &Option<[u8; 2]>,
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"esds", {
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
        let write_od_len = |mut size: u32, cursor: &mut Writer| -> Result<(), Error> {
            while size > 0x7F {
                size -= 0x7F;
                cursor.write_all(&[0xff])?;
            }
            cursor.write_all(&[size as u8])?;
            Ok(())
        };
        if let Some(ref dsi) = dsi.as_ref() {
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
            cursor.write_all(&((channel_count * 6144 / 8) as u16).to_be_bytes())?;
            cursor.write_all(&[0x00; 8])?;
            cursor.write_all(&[0x05])?;
            write_od_len(dsi_bytes, cursor)?;
            cursor.write_all(&dsi[..])?;
        }
    })
}

fn write_mp4a<Writer>(
    channel_count: u32,
    sample_rate: u32,
    dsi: &Option<[u8; 2]>,
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"mp4a", {
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(&[0x00; 2])?;
        cursor.write_all(&[0x00, 0x01])?;

        cursor.write_all(&[0x00; 8])?;
        cursor.write_all(&(channel_count as u16).to_be_bytes())?;
        cursor.write_all(&[0x00, 0x10])?; //16 bits per sample
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(&(sample_rate << 16).to_be_bytes())?;
        write_esds(channel_count, dsi, cursor)?;
    })
}

fn write_avcc<Writer>(
    sps: &Option<Vec<u8>>,
    pps: &Option<Vec<u8>>,
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"avcC", {
        // configurationVersion
        cursor.write_all(&[0x01])?;
        if let Some(sps) = sps.as_ref() {
            cursor.write_all(&sps[1..4])?;
            cursor.write_all(&[255])?;
            cursor.write_all(&[0xe0 | 1])?;
            cursor.write_all(&(sps.len() as u16).to_be_bytes())?;
            cursor.write_all(&sps[..])?;
        }
        if let Some(pps) = pps.as_ref() {
            cursor.write_all(&[1])?;
            cursor.write_all(&(pps.len() as u16).to_be_bytes())?;
            cursor.write_all(&pps[..])?;
        }
    })
}

fn write_avc1<Writer>(
    width: u16,
    height: u16,
    sps: &Option<Vec<u8>>,
    pps: &Option<Vec<u8>>,
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"avc1", {
        cursor.write_all(&[0x00; 6])?;
        cursor.write_all(&[0x00, 0x01])?;
        cursor.write_all(&[0x00; 16])?;

        cursor.write_all(&width.to_be_bytes())?;
        cursor.write_all(&height.to_be_bytes())?;
        cursor.write_all(&0x00480000u32.to_be_bytes())?;
        cursor.write_all(&0x00480000u32.to_be_bytes())?;
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(&[0x00, 0x01])?;
        cursor.write_all(&[0x00; 32])?;
        cursor.write_all(&[0x00, 0x18])?;
        cursor.write_all(&(-1 as i16).to_be_bytes())?;
        write_avcc(sps, pps, cursor)?;
    })
}

fn write_hvcc<Writer>(
    vps: &Option<Vec<u8>>,
    sps: &Option<Vec<u8>>,
    pps: &Option<Vec<u8>>,
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"hvcC", {
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

        if let Some(vps) = vps.as_ref() {
            cursor.write_all(&[0x00, 0x01])?;
            cursor.write_all(&(vps.len() as u16).to_be_bytes())?;
            cursor.write_all(&vps[..])?;
        } else {
            cursor.write_all(&[0x00; 2])?;
        }
        cursor.write_all(&[(1 << 7) | (33 & 0x3f)])?; //sps
        if let Some(sps) = sps.as_ref() {
            cursor.write_all(&[0x00, 0x01])?;
            cursor.write_all(&(sps.len() as u16).to_be_bytes())?;
            cursor.write_all(&sps[..])?;
        } else {
            cursor.write_all(&[0x00; 2])?;
        }
        cursor.write_all(&[(1 << 7) | (34 & 0x3f)])?; //pps
        if let Some(pps) = pps.as_ref() {
            cursor.write_all(&[0x00, 0x01])?;
            cursor.write_all(&(pps.len() as u16).to_be_bytes())?;
            cursor.write_all(&pps[..])?;
        } else {
            cursor.write_all(&[0x00; 2])?;
        }
    })
}

fn write_hvc1<Writer>(
    width: u16,
    height: u16,
    vps: &Option<Vec<u8>>,
    sps: &Option<Vec<u8>>,
    pps: &Option<Vec<u8>>,
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"hvc1", {
        cursor.write_all(&[0x00; 6])?;
        cursor.write_all(&[0x00, 0x01])?;
        cursor.write_all(&[0x00; 16])?;
        cursor.write_all(&width.to_be_bytes())?;
        cursor.write_all(&height.to_be_bytes())?;
        cursor.write_all(&0x00480000u32.to_be_bytes())?;
        cursor.write_all(&0x00480000u32.to_be_bytes())?;
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(&[0x00, 0x01])?;
        cursor.write_all(&[0x00; 32])?;
        cursor.write_all(&[0x00, 0x18])?;
        cursor.write_all(&(-1 as i16).to_be_bytes())?;
        write_hvcc(vps, sps, pps, cursor)?;
    })
}

fn write_stsd<Writer>(track: &Track, cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"stsd", {
        cursor.write_all(&[0x00; 4])?;
        cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
        if let TrackType::Video = track.track_type {
            match track.codec {
                Codec::HEVC => {
                    write_hvc1(
                        track.width as u16,
                        track.height as u16,
                        &track.vps,
                        &track.sps,
                        &track.pps,
                        cursor,
                    )?;
                }
                Codec::AVC => {
                    write_avc1(
                        track.width as u16,
                        track.height as u16,
                        &track.sps,
                        &track.pps,
                        cursor,
                    )?;
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
                    write_mp4a(track.channel_count, track.sample_rate, &track.dsi, cursor)?;
                }
                Codec::OPUS => {
                    //
                    write_opus(track.channel_count, track.sample_rate, cursor)?;
                }
                _ => {}
            }
        }
    })
}

fn write_stts<Writer>(samples: &[SampleInfo], cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"stts", {
        cursor.write_all(&[0x00; 4])?;
        // entry count
        let entry_count_idx = cursor.stream_position()?;
        cursor.seek(SeekFrom::Current(4))?;
        let mut entry_count: u32 = 0;
        let mut cnt: u32 = 1;
        for i in 0..samples.len() {
            if i == samples.len() - 1 || samples[i].sample_delta != samples[i + 1].sample_delta {
                cursor.write_all(&cnt.to_be_bytes())?;
                cursor.write_all(&samples[i].sample_delta.to_be_bytes())?;
                cnt = 0;
                entry_count += 1;
            }
            cnt += 1;
        }
        let end_pos = cursor.stream_position()?;
        cursor.seek(SeekFrom::Start(entry_count_idx))?;
        cursor.write_all(&entry_count.to_be_bytes())?;
        cursor.seek(SeekFrom::Start(end_pos))?;
    })
}

fn write_ctts<Writer>(samples: &[SampleInfo], cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    let mut has_ctts = false;
    for sample in samples.iter() {
        if sample.sample_ct_offset != 0 {
            has_ctts = true;
            break;
        }
    }
    if !has_ctts {
        return Ok(());
    }
    mp4_box!(cursor, b"ctts", {
        cursor.write_all(&[0x00; 4])?;
        let entry_count_idx = cursor.stream_position()?;
        cursor.seek(SeekFrom::Current(4))?;
        let mut entry_count: u32 = 0;
        let mut cnt: u32 = 1;
        for i in 0..samples.len() {
            if i == samples.len() - 1
                || samples[i].sample_ct_offset != samples[i + 1].sample_ct_offset
            {
                cursor.write_all(&cnt.to_be_bytes())?;
                cursor.write_all(&samples[i].sample_ct_offset.to_be_bytes())?;
                cnt = 0;
                entry_count += 1;
            }
            cnt += 1;
        }
        let end_pos = cursor.stream_position()?;
        cursor.seek(SeekFrom::Start(entry_count_idx))?;
        cursor.write_all(&entry_count.to_be_bytes())?;
        cursor.seek(SeekFrom::Start(end_pos))?;
    })
}

fn write_stbl<Writer>(track: &Track, fragment: bool, cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"stbl", {
        write_stsd(track, cursor)?;
        write_stts(&track.samples, cursor)?;
        write_ctts(&track.samples, cursor)?;
        write_stsc(fragment, cursor)?;
        write_stsz(&track.samples, cursor)?;
        if track.samples.len() > 0 {
            let last_sample = track.samples.last().unwrap();
            if last_sample.offset > 0xffffffff {
                write_co64(&track.samples, cursor)?;
            } else {
                write_stco(&track.samples, cursor)?;
            }
        }
        if !fragment {
            if let TrackType::Video = track.track_type {
                //stss
                write_stss(&track.samples, cursor)?;
            }
        }
    })
}

fn write_minf<Writer>(track: &Track, fragment: bool, cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"minf", {
        match track.track_type {
            TrackType::Video => {
                write_vmhd(cursor)?;
            }
            TrackType::Audio => {
                write_smhd(cursor)?;
            }
        }
        write_dinf(cursor)?;
        write_stbl(track, fragment, cursor)?;
    })
}

fn write_tkhd<Writer>(track: &Track, cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"tkhd", {
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

fn write_mdia<Writer>(
    track: &Track,
    fragment: bool,
    language: &[u8; 3],
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"mdia", {
        write_mdhd(track.timescale, track.duration, &language, cursor)?;
        write_hdlr(matches!(track.track_type, TrackType::Video), cursor)?;
        write_minf(track, fragment, cursor)?;
    })
}

fn write_mvhd<Writer>(
    create_time: u64,
    duration: u32,
    timescale: u32,
    track_ids: u32,
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"mvhd", {
        if create_time != 0 {
            // version & flag
            cursor.write_all(&[0x01, 0x00, 0x00, 0x00])?;
            // create_time
            cursor.write_all(&create_time.to_be_bytes())?;
            // modify_time
            cursor.write_all(&create_time.to_be_bytes())?;
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
        cursor.write_all(&TIMESCALE.to_be_bytes())?;
        // duration
        let duration = duration / (timescale / TIMESCALE);
        if create_time != 0 {
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
        cursor.write_all(&track_ids.to_be_bytes())?;
    })
}

fn write_track<Writer>(
    language: &[u8; 3],
    fragment: bool,
    track: &Track,
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"trak", {
        write_tkhd(track, cursor)?;
        write_mdia(track, fragment, &language, cursor)?;
    })
}

fn write_trexs<Writer>(tracks: &[&Option<Track>], cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    for track in tracks.iter() {
        if let Some(track) = track.as_ref() {
            write_trex(track.id, cursor)?;
        }
    }
    Ok(())
}

fn write_mvex<Writer>(tracks: &[&Option<Track>], cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"mvex", {
        write_trexs(tracks, cursor)?;
    })
}
fn write_tracks<Writer>(
    language: &[u8; 3],
    fragment: bool,
    tracks: &[&Option<Track>],
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    for track in tracks.iter() {
        if let Some(track) = track.as_ref() {
            write_track(&language, fragment, track, cursor)?;
        }
    }
    Ok(())
}
pub fn write_moov<Writer>(
    video_track: &Option<Track>,
    audio_track: &Option<Track>,
    create_time: u64,
    track_ids: u32,
    language: &[u8; 3],
    fragment: bool,
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"moov", {
        write_mvhd(
            create_time,
            video_track.as_ref().unwrap().duration,
            video_track.as_ref().unwrap().timescale,
            track_ids,
            cursor,
        )?;
        write_tracks(language, fragment, &[video_track, audio_track], cursor)?;
        if fragment {
            write_mvex(&[video_track, audio_track], cursor)?;
        }
    })
}

fn write_tfhd<Writer>(track: &Track, sample_duration: u32, cursor: &mut Writer) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"tfhd", {
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

fn write_trun<Writer>(
    track: &Track,
    moof_pos: u64,
    data_size: u32,
    sample_duration: u32,
    ct_offset: i32,
    sample_type: SampleType,
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"trun", {
        let data_offset_pos;
        if let TrackType::Video = track.track_type {
            if let SampleType::RandomAccess = sample_type {
                let flags: u32 = 0x001 | 0x004 | 0x100 | 0x200 | 0x800;
                cursor.write_all(&flags.to_be_bytes())?;
                cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
                data_offset_pos = cursor.stream_position()?;
                cursor.seek(SeekFrom::Current(4))?;
                cursor.write_all(&0x2000000u32.to_be_bytes())?;
                cursor.write_all(&sample_duration.to_be_bytes())?;
                cursor.write_all(&data_size.to_be_bytes())?;
                cursor.write_all(&ct_offset.to_be_bytes())?;
            } else {
                let flags: u32 = 0x001 | 0x100 | 0x200 | 0x800;
                cursor.write_all(&flags.to_be_bytes())?;
                cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
                data_offset_pos = cursor.stream_position()?;
                cursor.seek(SeekFrom::Current(4))?;
                cursor.write_all(&sample_duration.to_be_bytes())?;
                cursor.write_all(&data_size.to_be_bytes())?;
                cursor.write_all(&ct_offset.to_be_bytes())?;
            }
        } else {
            let flags: u32 = 0x001 | 0x200;
            cursor.write_all(&flags.to_be_bytes())?;
            cursor.write_all(&[0x00, 0x00, 0x00, 0x01])?;
            data_offset_pos = cursor.stream_position()?;
            cursor.seek(SeekFrom::Current(4))?;
            cursor.write_all(&data_size.to_be_bytes())?;
        }
        let end_pos = cursor.stream_position()?;
        let data_offset = (end_pos - moof_pos + 8) as u32;
        cursor.seek(SeekFrom::Start(data_offset_pos))?;
        cursor.write_all(&data_offset.to_be_bytes())?;
        cursor.seek(SeekFrom::Start(end_pos)).unwrap();
    })
}

fn write_traf<Writer>(
    moof_pos: u64,
    track: &Track,
    data: &[u8],
    sample_duration: u32,
    ct_offset: i32,
    sample_type: SampleType,
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"traf", {
        write_tfhd(track, sample_duration, cursor)?;
        write_trun(
            track,
            moof_pos,
            data.len() as u32 + 4,
            sample_duration,
            ct_offset,
            sample_type,
            cursor,
        )?;
    })
}

pub fn write_moof<Writer>(
    fragment_id: u32,
    data: &[u8],
    duration: u32,
    track: &Track,
    ct_offset: i32,
    sample_type: SampleType,
    cursor: &mut Writer,
) -> Result<(), Error>
where
    Writer: Write + Seek,
{
    mp4_box!(cursor, b"moof", {
        let moof_pos = cursor.stream_position()? - 8;
        write_mfhd(fragment_id, cursor)?;
        write_traf(
            moof_pos,
            track,
            data,
            duration,
            ct_offset,
            sample_type,
            cursor,
        )?;
    })
}

pub fn write_mdat<Writer>(buf: &[u8], video: bool, writer: &mut Writer) -> Result<u64, Error>
where
    Writer: Write,
{
    let mut box_size = buf.len() as u32 + 8;
    if video {
        box_size += 4;
    }
    writer.write_all(&box_size.to_be_bytes())?;
    writer.write_all(b"mdat")?;
    if video {
        let nal_size_buf = (buf.len() as u32).to_be_bytes();
        writer.write_all(&nal_size_buf)?;
    }
    writer.write_all(buf)?;

    Ok(box_size as u64)
}

pub fn write_ftyp<Writer>(writer: &mut Writer) -> Result<u64, Error>
where
    Writer: Write,
{
    writer.write_all(b"\x00\x00\x00\x20ftypisom\x00\x00\x00\x00mp41isomiso6iso2")?;
    Ok(32)
}

pub fn write_mdat_header<Writer>(writer: &mut Writer) -> Result<u64, Error>
where
    Writer: Write,
{
    writer.write_all(b"\x00\x00\x00\x01mdat\x00\x00\x00\x00\x00\x00\x00\x10")?;
    Ok(16)
}
