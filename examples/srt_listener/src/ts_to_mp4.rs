//! MPEG2-TS から MP4 への変換モジュール

use std::collections::HashMap;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::num::{NonZeroU16, NonZeroU32};
use std::path::Path;

use mpeg2ts::es::{StreamId, StreamType};
use mpeg2ts::pes::{PesPacketReader, ReadPesPacket};
use mpeg2ts::ts::{Pid, ReadTsPacket, TsPacket, TsPayload};
use tracing::{info, warn};

use shiguredo_mp4::boxes::{
    AudioSampleEntryFields, Avc1Box, AvccBox, EsdsBox, Mp4aBox, SampleEntry,
    VisualSampleEntryFields,
};
use shiguredo_mp4::descriptors::{
    DecoderConfigDescriptor, DecoderSpecificInfo, EsDescriptor, SlConfigDescriptor,
};
use shiguredo_mp4::mux::{Mp4FileMuxer, Sample};
use shiguredo_mp4::{FixedPointNumber, TrackKind, Uint};

/// MPEG2-TS のタイムスケール (90kHz)
const TS_TIMESCALE: u32 = 90000;

/// AAC の 1 フレームあたりのサンプル数
const AAC_SAMPLES_PER_FRAME: u32 = 1024;

/// 変換エラー
#[derive(Debug)]
pub enum ConvertError {
    Io(std::io::Error),
    Mpeg2Ts(String),
    Mp4(shiguredo_mp4::mux::MuxError),
    InvalidData(String),
    UnsupportedCodec(String),
}

impl std::fmt::Display for ConvertError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConvertError::Io(e) => write!(f, "IO error: {}", e),
            ConvertError::Mpeg2Ts(s) => write!(f, "MPEG2-TS error: {}", s),
            ConvertError::Mp4(e) => write!(f, "MP4 error: {}", e),
            ConvertError::InvalidData(s) => write!(f, "Invalid data: {}", s),
            ConvertError::UnsupportedCodec(s) => write!(f, "Unsupported codec: {}", s),
        }
    }
}

impl std::error::Error for ConvertError {}

impl From<std::io::Error> for ConvertError {
    fn from(e: std::io::Error) -> Self {
        ConvertError::Io(e)
    }
}

impl From<shiguredo_mp4::mux::MuxError> for ConvertError {
    fn from(e: shiguredo_mp4::mux::MuxError) -> Self {
        ConvertError::Mp4(e)
    }
}

impl From<mpeg2ts::Error> for ConvertError {
    fn from(e: mpeg2ts::Error) -> Self {
        ConvertError::Mpeg2Ts(e.to_string())
    }
}

/// H.264 ストリーム情報
struct AvcStream {
    sps: Vec<u8>,
    pps: Vec<u8>,
    profile_idc: u8,
    level_idc: u8,
    width: u16,
    height: u16,
    samples: Vec<VideoSample>,
}

/// ビデオサンプル
struct VideoSample {
    data: Vec<u8>,
    #[expect(dead_code)]
    pts: u64,
    dts: u64,
    keyframe: bool,
}

/// AAC ストリーム情報
struct AacStream {
    profile: u8,
    sampling_frequency_index: u8,
    channel_configuration: u8,
    sample_rate: u32,
    samples: Vec<AudioSample>,
}

/// オーディオサンプル
struct AudioSample {
    data: Vec<u8>,
    #[expect(dead_code)]
    pts: u64,
}

/// ADTS ヘッダ情報
struct AdtsHeader {
    profile: u8,
    sampling_frequency_index: u8,
    channel_configuration: u8,
    frame_length: u16,
}

impl AdtsHeader {
    fn parse(data: &[u8]) -> Result<Self, ConvertError> {
        if data.len() < 7 {
            return Err(ConvertError::InvalidData("ADTS header too short".into()));
        }

        // シンクワード確認 (0xFFF)
        if data[0] != 0xFF || (data[1] & 0xF0) != 0xF0 {
            return Err(ConvertError::InvalidData("Invalid ADTS sync word".into()));
        }

        let profile = (data[2] >> 6) & 0x03;
        let sampling_frequency_index = (data[2] >> 2) & 0x0F;
        let channel_configuration = ((data[2] & 0x01) << 2) | ((data[3] >> 6) & 0x03);
        let frame_length = (u16::from(data[3] & 0x03) << 11)
            | (u16::from(data[4]) << 3)
            | (u16::from(data[5] >> 5) & 0x07);

        Ok(AdtsHeader {
            profile,
            sampling_frequency_index,
            channel_configuration,
            frame_length,
        })
    }

    fn header_length(&self) -> usize {
        7 // CRC なしの場合
    }

    fn sample_rate(&self) -> u32 {
        match self.sampling_frequency_index {
            0 => 96000,
            1 => 88200,
            2 => 64000,
            3 => 48000,
            4 => 44100,
            5 => 32000,
            6 => 24000,
            7 => 22050,
            8 => 16000,
            9 => 12000,
            10 => 11025,
            11 => 8000,
            12 => 7350,
            _ => 48000,
        }
    }
}

/// NAL ユニットタイプ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NalUnitType {
    SliceNonIdr = 1,
    SliceIdr = 5,
    Sei = 6,
    Sps = 7,
    Pps = 8,
    Aud = 9,
    Other,
}

impl NalUnitType {
    fn from_byte(b: u8) -> Self {
        match b & 0x1F {
            1 => NalUnitType::SliceNonIdr,
            5 => NalUnitType::SliceIdr,
            6 => NalUnitType::Sei,
            7 => NalUnitType::Sps,
            8 => NalUnitType::Pps,
            9 => NalUnitType::Aud,
            _ => NalUnitType::Other,
        }
    }
}

/// Byte-stream format から NAL ユニットを分離するイテレータ
struct NalUnitIterator<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> NalUnitIterator<'a> {
    fn new(data: &'a [u8]) -> Self {
        let mut iter = NalUnitIterator { data, pos: 0 };
        // 最初のスタートコードをスキップ
        iter.skip_start_code();
        iter
    }

    fn skip_start_code(&mut self) {
        let remaining = &self.data[self.pos..];
        if remaining.len() >= 4 && remaining.starts_with(&[0, 0, 0, 1]) {
            self.pos += 4;
        } else if remaining.len() >= 3 && remaining.starts_with(&[0, 0, 1]) {
            self.pos += 3;
        }
    }

    fn find_next_start_code(&self) -> Option<usize> {
        let data = &self.data[self.pos..];
        for i in 0..data.len().saturating_sub(3) {
            if data[i..].starts_with(&[0, 0, 0, 1]) || data[i..].starts_with(&[0, 0, 1]) {
                return Some(self.pos + i);
            }
        }
        None
    }
}

impl<'a> Iterator for NalUnitIterator<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.data.len() {
            return None;
        }

        let start = self.pos;
        if let Some(next_pos) = self.find_next_start_code() {
            self.pos = next_pos;
            self.skip_start_code();
            Some(&self.data[start..next_pos])
        } else {
            self.pos = self.data.len();
            Some(&self.data[start..])
        }
    }
}

/// Exp-Golomb ビットストリームリーダー
struct BitReader<'a> {
    data: &'a [u8],
    byte_pos: usize,
    bit_pos: u8,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        BitReader {
            data,
            byte_pos: 0,
            bit_pos: 0,
        }
    }

    fn read_bit(&mut self) -> Option<u8> {
        if self.byte_pos >= self.data.len() {
            return None;
        }
        let bit = (self.data[self.byte_pos] >> (7 - self.bit_pos)) & 1;
        self.bit_pos += 1;
        if self.bit_pos == 8 {
            self.bit_pos = 0;
            self.byte_pos += 1;
        }
        Some(bit)
    }

    fn read_bits(&mut self, n: u8) -> Option<u32> {
        let mut val = 0u32;
        for _ in 0..n {
            val = (val << 1) | u32::from(self.read_bit()?);
        }
        Some(val)
    }

    /// Exp-Golomb 符号化された unsigned 整数を読み取る
    fn read_ue(&mut self) -> Option<u32> {
        let mut leading_zeros = 0u8;
        while self.read_bit()? == 0 {
            leading_zeros += 1;
            if leading_zeros > 31 {
                return None;
            }
        }
        if leading_zeros == 0 {
            return Some(0);
        }
        let suffix = self.read_bits(leading_zeros)?;
        Some((1 << leading_zeros) - 1 + suffix)
    }

    /// Exp-Golomb 符号化された signed 整数を読み取る
    fn read_se(&mut self) -> Option<i32> {
        let val = self.read_ue()?;
        let sign = if val & 1 == 1 { 1 } else { -1 };
        Some(sign * val.div_ceil(2) as i32)
    }
}

/// SPS から width/height を抽出
fn parse_sps(sps: &[u8]) -> Result<(u16, u16, u8, u8), ConvertError> {
    if sps.len() < 4 {
        return Err(ConvertError::InvalidData("SPS too short".into()));
    }

    let profile_idc = sps[1];
    let level_idc = sps[3];

    // NAL ヘッダをスキップして SPS rbsp を取得
    let mut reader = BitReader::new(&sps[4..]);

    // seq_parameter_set_id
    reader.read_ue();

    // High Profile 以上の場合の追加フィールド
    if matches!(
        profile_idc,
        100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135
    ) {
        let chroma_format_idc = reader.read_ue().unwrap_or(1);
        if chroma_format_idc == 3 {
            reader.read_bits(1); // separate_colour_plane_flag
        }
        reader.read_ue(); // bit_depth_luma_minus8
        reader.read_ue(); // bit_depth_chroma_minus8
        reader.read_bits(1); // qpprime_y_zero_transform_bypass_flag

        let seq_scaling_matrix_present_flag = reader.read_bits(1).unwrap_or(0);
        if seq_scaling_matrix_present_flag == 1 {
            let count = if chroma_format_idc != 3 { 8 } else { 12 };
            for i in 0..count {
                let present = reader.read_bits(1).unwrap_or(0);
                if present == 1 {
                    let size = if i < 6 { 16 } else { 64 };
                    let mut last_scale = 8i32;
                    let mut next_scale = 8i32;
                    for _ in 0..size {
                        if next_scale != 0 {
                            let delta = reader.read_se().unwrap_or(0);
                            next_scale = (last_scale + delta + 256) % 256;
                        }
                        last_scale = if next_scale == 0 {
                            last_scale
                        } else {
                            next_scale
                        };
                    }
                }
            }
        }
    }

    reader.read_ue(); // log2_max_frame_num_minus4
    let pic_order_cnt_type = reader.read_ue().unwrap_or(0);

    if pic_order_cnt_type == 0 {
        reader.read_ue(); // log2_max_pic_order_cnt_lsb_minus4
    } else if pic_order_cnt_type == 1 {
        reader.read_bits(1); // delta_pic_order_always_zero_flag
        reader.read_se(); // offset_for_non_ref_pic
        reader.read_se(); // offset_for_top_to_bottom_field
        let num_ref_frames_in_pic_order_cnt_cycle = reader.read_ue().unwrap_or(0);
        for _ in 0..num_ref_frames_in_pic_order_cnt_cycle {
            reader.read_se(); // offset_for_ref_frame
        }
    }

    reader.read_ue(); // max_num_ref_frames
    reader.read_bits(1); // gaps_in_frame_num_value_allowed_flag

    let pic_width_in_mbs_minus1 = reader.read_ue().unwrap_or(119); // デフォルト 1920/16-1
    let pic_height_in_map_units_minus1 = reader.read_ue().unwrap_or(67); // デフォルト 1080/16-1 (interlaced 考慮)
    let frame_mbs_only_flag = reader.read_bits(1).unwrap_or(1);

    if frame_mbs_only_flag == 0 {
        reader.read_bits(1); // mb_adaptive_frame_field_flag
    }

    reader.read_bits(1); // direct_8x8_inference_flag

    let frame_cropping_flag = reader.read_bits(1).unwrap_or(0);
    let (crop_left, crop_right, crop_top, crop_bottom) = if frame_cropping_flag == 1 {
        (
            reader.read_ue().unwrap_or(0),
            reader.read_ue().unwrap_or(0),
            reader.read_ue().unwrap_or(0),
            reader.read_ue().unwrap_or(0),
        )
    } else {
        (0, 0, 0, 0)
    };

    // 解像度計算
    let width = ((pic_width_in_mbs_minus1 + 1) * 16 - crop_left * 2 - crop_right * 2) as u16;
    let height = ((2 - frame_mbs_only_flag) * (pic_height_in_map_units_minus1 + 1) * 16
        - crop_top * 2
        - crop_bottom * 2) as u16;

    Ok((width, height, profile_idc, level_idc))
}

/// TsPacketReader ラッパー（StreamType を追跡）
struct TsPacketReaderWrapper<R> {
    inner: mpeg2ts::ts::TsPacketReader<R>,
    pid_to_stream_type: HashMap<Pid, StreamType>,
    stream_id_to_pid: HashMap<StreamId, Pid>,
}

impl<R: std::io::Read> TsPacketReaderWrapper<R> {
    fn new(reader: R) -> Self {
        TsPacketReaderWrapper {
            inner: mpeg2ts::ts::TsPacketReader::new(reader),
            pid_to_stream_type: HashMap::new(),
            stream_id_to_pid: HashMap::new(),
        }
    }

    fn get_stream_type(&self, stream_id: StreamId) -> Option<StreamType> {
        self.stream_id_to_pid
            .get(&stream_id)
            .and_then(|pid| self.pid_to_stream_type.get(pid))
            .copied()
    }
}

impl<R: std::io::Read> ReadTsPacket for TsPacketReaderWrapper<R> {
    fn read_ts_packet(&mut self) -> mpeg2ts::Result<Option<TsPacket>> {
        if let Some(packet) = self.inner.read_ts_packet()? {
            match &packet.payload {
                Some(TsPayload::Pmt(pmt)) => {
                    for es_info in &pmt.es_info {
                        self.pid_to_stream_type
                            .insert(es_info.elementary_pid, es_info.stream_type);
                    }
                }
                Some(TsPayload::PesStart(pes))
                    if self.pid_to_stream_type.contains_key(&packet.header.pid) =>
                {
                    self.stream_id_to_pid
                        .insert(pes.header.stream_id, packet.header.pid);
                }
                _ => {}
            }
            Ok(Some(packet))
        } else {
            Ok(None)
        }
    }
}

/// TS パケット境界を見つける（0x47 同期バイト）
fn find_ts_sync(data: &[u8]) -> Option<usize> {
    const TS_PACKET_SIZE: usize = 188;
    const SYNC_BYTE: u8 = 0x47;

    (0..data.len().saturating_sub(TS_PACKET_SIZE * 3)).find(|&i| {
        data[i] == SYNC_BYTE
            && data.get(i + TS_PACKET_SIZE) == Some(&SYNC_BYTE)
            && data.get(i + TS_PACKET_SIZE * 2) == Some(&SYNC_BYTE)
    })
}

/// MPEG2-TS データをパースしてストリームを抽出
fn parse_ts_data(data: &[u8]) -> Result<(Option<AvcStream>, Option<AacStream>), ConvertError> {
    // TS パケット同期を見つける
    let sync_offset = find_ts_sync(data).unwrap_or(0);
    if sync_offset > 0 {
        info!("found TS sync at offset {}", sync_offset);
    }
    let data = &data[sync_offset..];

    let cursor = std::io::Cursor::new(data);
    let ts_reader = TsPacketReaderWrapper::new(cursor);
    let mut pes_reader = PesPacketReader::new(ts_reader);

    let mut avc_stream: Option<AvcStream> = None;
    let mut aac_stream: Option<AacStream> = None;

    let mut first_video_pts: Option<u64> = None;
    let mut first_audio_pts: Option<u64> = None;

    loop {
        let pes = match pes_reader.read_pes_packet() {
            Ok(Some(pes)) => pes,
            Ok(None) => break,
            Err(e) => {
                // パースエラーは警告を出して続行
                warn!("PES parse error (skipping): {}", e);
                continue;
            }
        };
        let stream_type = pes_reader
            .ts_packet_reader()
            .get_stream_type(pes.header.stream_id);

        if pes.header.stream_id.is_video() {
            if stream_type != Some(StreamType::H264) {
                continue;
            }

            let pts = pes
                .header
                .pts
                .map(|t| t.as_u64())
                .ok_or_else(|| ConvertError::InvalidData("Missing PTS".into()))?;
            let dts = pes.header.dts.map(|t| t.as_u64()).unwrap_or(pts);

            if first_video_pts.is_none() {
                first_video_pts = Some(pts);
            }

            // SPS/PPS を抽出して AvcStream を初期化
            if avc_stream.is_none() {
                let mut sps = None;
                let mut pps = None;

                for nal_unit in NalUnitIterator::new(&pes.data) {
                    if nal_unit.is_empty() {
                        continue;
                    }
                    match NalUnitType::from_byte(nal_unit[0]) {
                        NalUnitType::Sps => sps = Some(nal_unit.to_vec()),
                        NalUnitType::Pps => pps = Some(nal_unit.to_vec()),
                        _ => {}
                    }
                }

                if let (Some(sps_data), Some(pps_data)) = (sps, pps) {
                    let (width, height, profile_idc, level_idc) = parse_sps(&sps_data)?;
                    avc_stream = Some(AvcStream {
                        sps: sps_data,
                        pps: pps_data,
                        profile_idc,
                        level_idc,
                        width,
                        height,
                        samples: Vec::new(),
                    });
                }
            }

            if let Some(ref mut stream) = avc_stream {
                // NAL ユニットを Length-prefixed 形式に変換
                let mut sample_data = Vec::new();
                let mut is_keyframe = false;

                for nal_unit in NalUnitIterator::new(&pes.data) {
                    if nal_unit.is_empty() {
                        continue;
                    }

                    let nal_type = NalUnitType::from_byte(nal_unit[0]);
                    if nal_type == NalUnitType::SliceIdr {
                        is_keyframe = true;
                    }

                    // SPS/PPS/AUD はスキップ（既に抽出済み）
                    if matches!(
                        nal_type,
                        NalUnitType::Sps | NalUnitType::Pps | NalUnitType::Aud
                    ) {
                        continue;
                    }

                    // 4 バイト長プレフィックス + NAL ユニット
                    let len = nal_unit.len() as u32;
                    sample_data.extend_from_slice(&len.to_be_bytes());
                    sample_data.extend_from_slice(nal_unit);
                }

                if !sample_data.is_empty() {
                    let base_pts = first_video_pts.unwrap_or(0);
                    stream.samples.push(VideoSample {
                        data: sample_data,
                        pts: pts.saturating_sub(base_pts),
                        dts: dts.saturating_sub(base_pts),
                        keyframe: is_keyframe,
                    });
                }
            }
        } else if pes.header.stream_id.is_audio() {
            match stream_type {
                Some(StreamType::AdtsAac) => {}
                Some(other) => {
                    return Err(ConvertError::UnsupportedCodec(format!(
                        "Only AAC audio is supported, found {:?}",
                        other
                    )));
                }
                None => {
                    // PMT 未受信の場合はスキップ
                    continue;
                }
            }

            let pts = pes
                .header
                .pts
                .map(|t| t.as_u64())
                .ok_or_else(|| ConvertError::InvalidData("Missing audio PTS".into()))?;

            if first_audio_pts.is_none() {
                first_audio_pts = Some(pts);
            }

            // ADTS フレームをパース
            let mut offset = 0;
            while offset < pes.data.len() {
                let header = AdtsHeader::parse(&pes.data[offset..])?;

                if aac_stream.is_none() {
                    aac_stream = Some(AacStream {
                        profile: header.profile,
                        sampling_frequency_index: header.sampling_frequency_index,
                        channel_configuration: header.channel_configuration,
                        sample_rate: header.sample_rate(),
                        samples: Vec::new(),
                    });
                }

                if let Some(ref mut stream) = aac_stream {
                    let header_len = header.header_length();
                    let frame_len = header.frame_length as usize;

                    if offset + frame_len > pes.data.len() {
                        break;
                    }

                    // ADTS ヘッダを除去した raw AAC データ
                    let raw_data = pes.data[offset + header_len..offset + frame_len].to_vec();

                    stream.samples.push(AudioSample {
                        data: raw_data,
                        pts: pts.saturating_sub(first_audio_pts.unwrap_or(0)),
                    });
                }

                offset += header.frame_length as usize;
            }
        }
    }

    Ok((avc_stream, aac_stream))
}

/// AudioSpecificConfig を生成
fn build_audio_specific_config(
    profile: u8,
    sampling_frequency_index: u8,
    channel_configuration: u8,
) -> Vec<u8> {
    // audioObjectType (5 bits) = profile + 1
    // samplingFrequencyIndex (4 bits)
    // channelConfiguration (4 bits)
    // padding (3 bits)
    let audio_object_type = profile + 1;
    vec![
        (audio_object_type << 3) | (sampling_frequency_index >> 1),
        ((sampling_frequency_index & 1) << 7) | (channel_configuration << 3),
    ]
}

/// MPEG2-TS データを MP4 ファイルに変換
pub fn convert_ts_to_mp4(ts_data: &[u8], output_path: &Path) -> Result<(), ConvertError> {
    let (avc_stream, aac_stream) = parse_ts_data(ts_data)?;

    let avc_stream =
        avc_stream.ok_or_else(|| ConvertError::InvalidData("No H.264 stream found".into()))?;

    // ビデオのみでも OK とする
    let has_audio = aac_stream.is_some();

    let mut muxer = Mp4FileMuxer::new()?;

    // 初期ボックスをファイルに書き込み
    let initial_bytes = muxer.initial_boxes_bytes();
    let mut file = File::create(output_path)?;
    file.write_all(initial_bytes)?;

    let mut current_offset = initial_bytes.len() as u64;

    // ビデオサンプルエントリーを作成
    // High Profile 等 (66/77/88 以外) では追加フィールドが必要
    let needs_extended_fields = !matches!(avc_stream.profile_idc, 66 | 77 | 88);
    let avcc_box = AvccBox {
        avc_profile_indication: avc_stream.profile_idc,
        profile_compatibility: 0,
        avc_level_indication: avc_stream.level_idc,
        length_size_minus_one: Uint::new(3), // 4 バイト長
        sps_list: vec![avc_stream.sps.clone()],
        pps_list: vec![avc_stream.pps.clone()],
        chroma_format: if needs_extended_fields {
            Some(Uint::new(1)) // 4:2:0
        } else {
            None
        },
        bit_depth_luma_minus8: if needs_extended_fields {
            Some(Uint::new(0)) // 8-bit
        } else {
            None
        },
        bit_depth_chroma_minus8: if needs_extended_fields {
            Some(Uint::new(0)) // 8-bit
        } else {
            None
        },
        sps_ext_list: vec![],
    };

    let video_sample_entry = SampleEntry::Avc1(Avc1Box {
        visual: VisualSampleEntryFields {
            data_reference_index: NonZeroU16::new(1).expect("1 is always non-zero"),
            width: avc_stream.width,
            height: avc_stream.height,
            horizresolution: FixedPointNumber::new(72, 0),
            vertresolution: FixedPointNumber::new(72, 0),
            frame_count: 1,
            compressorname: [0; 32],
            depth: 0x0018,
        },
        avcc_box,
        unknown_boxes: vec![],
    });

    // オーディオサンプルエントリーを作成
    let audio_sample_entry = if let Some(ref aac) = aac_stream {
        let audio_specific_config = build_audio_specific_config(
            aac.profile,
            aac.sampling_frequency_index,
            aac.channel_configuration,
        );

        let esds_box = EsdsBox {
            es: EsDescriptor {
                es_id: 2,
                stream_priority: EsDescriptor::LOWEST_STREAM_PRIORITY,
                depends_on_es_id: None,
                url_string: None,
                ocr_es_id: None,
                dec_config_descr: DecoderConfigDescriptor {
                    object_type_indication:
                        DecoderConfigDescriptor::OBJECT_TYPE_INDICATION_AUDIO_ISO_IEC_14496_3,
                    stream_type: DecoderConfigDescriptor::STREAM_TYPE_AUDIO,
                    up_stream: DecoderConfigDescriptor::UP_STREAM_FALSE,
                    buffer_size_db: Uint::new(0),
                    max_bitrate: 0,
                    avg_bitrate: 0,
                    dec_specific_info: Some(DecoderSpecificInfo {
                        payload: audio_specific_config,
                    }),
                },
                sl_config_descr: SlConfigDescriptor,
            },
        };

        let channel_count = match aac.channel_configuration {
            1 => 1,
            2 => 2,
            3 => 3,
            4 => 4,
            5 => 5,
            6 => 6,
            7 => 8,
            _ => 2,
        };

        Some(SampleEntry::Mp4a(Mp4aBox {
            audio: AudioSampleEntryFields {
                data_reference_index: NonZeroU16::new(1).expect("1 is always non-zero"),
                channelcount: channel_count,
                samplesize: 16,
                samplerate: FixedPointNumber::new(aac.sample_rate as u16, 0),
            },
            esds_box,
            unknown_boxes: vec![],
        }))
    } else {
        None
    };

    // ビデオサンプルを書き込み
    let video_timescale = NonZeroU32::new(TS_TIMESCALE).expect("TS_TIMESCALE is always non-zero");
    let mut is_first_video = true;

    for (i, sample) in avc_stream.samples.iter().enumerate() {
        // サンプルデータをファイルに書き込み
        file.write_all(&sample.data)?;

        // duration を計算（次のサンプルとの差分）
        let duration = if i + 1 < avc_stream.samples.len() {
            (avc_stream.samples[i + 1].dts - sample.dts) as u32
        } else {
            3000 // デフォルト: 約 33ms (30fps)
        };

        let mp4_sample = Sample {
            track_kind: TrackKind::Video,
            sample_entry: if is_first_video {
                is_first_video = false;
                Some(video_sample_entry.clone())
            } else {
                None
            },
            keyframe: sample.keyframe,
            timescale: video_timescale,
            duration,
            composition_time_offset: None,
            data_offset: current_offset,
            data_size: sample.data.len(),
        };

        muxer.append_sample(&mp4_sample)?;
        current_offset += sample.data.len() as u64;
    }

    // オーディオサンプルを書き込み
    if let (Some(aac), Some(audio_entry)) = (&aac_stream, &audio_sample_entry) {
        let audio_timescale =
            NonZeroU32::new(aac.sample_rate).expect("sample rate is always non-zero");
        let mut is_first_audio = true;

        for sample in &aac.samples {
            file.write_all(&sample.data)?;

            let mp4_sample = Sample {
                track_kind: TrackKind::Audio,
                sample_entry: if is_first_audio {
                    is_first_audio = false;
                    Some(audio_entry.clone())
                } else {
                    None
                },
                keyframe: true, // AAC は常にキーフレーム
                timescale: audio_timescale,
                duration: AAC_SAMPLES_PER_FRAME,
                composition_time_offset: None,
                data_offset: current_offset,
                data_size: sample.data.len(),
            };

            muxer.append_sample(&mp4_sample)?;
            current_offset += sample.data.len() as u64;
        }
    }

    // ファイナライズ
    let finalized = muxer.finalize()?;
    for (offset, bytes) in finalized.offset_and_bytes_pairs() {
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(bytes)?;
    }

    info!(
        "saved: {} ({} video samples{})",
        output_path.display(),
        avc_stream.samples.len(),
        if has_audio {
            format!(
                ", {} audio samples",
                aac_stream.as_ref().map(|a| a.samples.len()).unwrap_or(0)
            )
        } else {
            String::new()
        }
    );

    Ok(())
}
