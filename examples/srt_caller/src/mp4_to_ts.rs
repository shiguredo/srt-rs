//! MP4 から MPEG2-TS への変換モジュール

use std::io::Write;
use std::path::Path;

use mpeg2ts::es::{StreamId, StreamType};
use mpeg2ts::pes::PesHeader;
use mpeg2ts::time::{ClockReference, Timestamp};
use mpeg2ts::ts::payload::{Bytes, Pat, Pes, Pmt};
use mpeg2ts::ts::{
    AdaptationField, ContinuityCounter, EsInfo, Pid, ProgramAssociation,
    TransportScramblingControl, TsHeader, TsPacket, TsPacketWriter, TsPayload, VersionNumber,
    WriteTsPacket,
};

use shiguredo_mp4::TrackKind;
use shiguredo_mp4::boxes::SampleEntry;
use shiguredo_mp4::demux::{DemuxError, Input, Mp4FileDemuxer};
use tracing::info;

/// MPEG2-TS のタイムスケール (90kHz)
const TS_TIMESCALE: u64 = 90000;

/// TS パケットサイズ
const TS_PACKET_SIZE: usize = 188;

/// TS パケットのペイロード最大サイズ
const TS_PAYLOAD_MAX: usize = TS_PACKET_SIZE - 4;

/// 変換エラー
#[derive(Debug)]
pub enum ConvertError {
    Io(std::io::Error),
    Mpeg2Ts(String),
    Mp4(DemuxError),
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

impl From<DemuxError> for ConvertError {
    fn from(e: DemuxError) -> Self {
        ConvertError::Mp4(e)
    }
}

impl From<mpeg2ts::Error> for ConvertError {
    fn from(e: mpeg2ts::Error) -> Self {
        ConvertError::Mpeg2Ts(e.to_string())
    }
}

/// PID の定義
const PID_PAT: u16 = 0x0000;
const PID_PMT: u16 = 0x1000;
const PID_VIDEO: u16 = 0x0100;
const PID_AUDIO: u16 = 0x0101;

/// コーデック情報
#[derive(Debug, Clone)]
enum VideoCodec {
    H264 {
        sps: Vec<u8>,
        pps: Vec<u8>,
    },
    H265 {
        vps: Vec<u8>,
        sps: Vec<u8>,
        pps: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
struct AudioCodec {
    profile: u8,
    sampling_frequency_index: u8,
    channel_configuration: u8,
    #[expect(dead_code)]
    sample_rate: u32,
}

/// MP4 を MPEG2-TS に変換するコンバーター
pub struct Mp4ToTsConverter {
    file_data: Vec<u8>,
    demuxer: Mp4FileDemuxer,
    video_codec: Option<VideoCodec>,
    audio_codec: Option<AudioCodec>,
    video_cc: ContinuityCounter,
    audio_cc: ContinuityCounter,
    pat_cc: ContinuityCounter,
    pmt_cc: ContinuityCounter,
    video_track_id: Option<u32>,
    audio_track_id: Option<u32>,
    video_timescale: u32,
    audio_timescale: u32,
}

impl Mp4ToTsConverter {
    /// MP4 ファイルから MPEG2-TS コンバーターを作成する
    pub fn from_file(path: &Path) -> Result<Self, ConvertError> {
        let file_data = std::fs::read(path)?;
        Self::from_data(file_data)
    }

    /// MP4 データから MPEG2-TS コンバーターを作成する
    pub fn from_data(file_data: Vec<u8>) -> Result<Self, ConvertError> {
        let input = Input {
            position: 0,
            data: &file_data,
        };
        let mut demuxer = Mp4FileDemuxer::new();
        demuxer.handle_input(input);

        let tracks = demuxer.tracks()?.to_vec();

        let mut video_track_id = None;
        let mut audio_track_id = None;
        let mut video_timescale = TS_TIMESCALE as u32;
        let mut audio_timescale = TS_TIMESCALE as u32;

        for track in &tracks {
            match track.kind {
                TrackKind::Video => {
                    if video_track_id.is_none() {
                        video_track_id = Some(track.track_id);
                        video_timescale = track.timescale.get();
                    }
                }
                TrackKind::Audio => {
                    if audio_track_id.is_none() {
                        audio_track_id = Some(track.track_id);
                        audio_timescale = track.timescale.get();
                    }
                }
            }
        }

        if video_track_id.is_none() && audio_track_id.is_none() {
            return Err(ConvertError::InvalidData(
                "No video or audio tracks found".into(),
            ));
        }

        info!(
            "mp4->ts: found {} tracks (video: {:?}, audio: {:?})",
            tracks.len(),
            video_track_id,
            audio_track_id
        );

        Ok(Self {
            file_data,
            demuxer,
            video_codec: None,
            audio_codec: None,
            video_cc: ContinuityCounter::new(),
            audio_cc: ContinuityCounter::new(),
            pat_cc: ContinuityCounter::new(),
            pmt_cc: ContinuityCounter::new(),
            video_track_id,
            audio_track_id,
            video_timescale,
            audio_timescale,
        })
    }

    /// MPEG2-TS データを生成する
    pub fn convert(&mut self) -> Result<Vec<u8>, ConvertError> {
        let mut output = Vec::new();
        let mut writer = TsPacketWriter::new(&mut output);

        // PAT を書き込む
        self.write_pat(&mut writer)?;

        // PMT を書き込む (初回は空、後で更新)
        let pmt_position = output.len();

        // サンプルを処理
        let mut samples: Vec<SampleData> = Vec::new();

        // demuxer から全サンプルを取得
        loop {
            let input = Input {
                position: 0,
                data: &self.file_data,
            };
            self.demuxer.handle_input(input);

            match self.demuxer.next_sample() {
                Ok(Some(sample)) => {
                    // サンプル情報をコピー
                    let track_id = sample.track.track_id;
                    let data_offset = sample.data_offset as usize;
                    let data_size = sample.data_size;
                    let timestamp = sample.timestamp;
                    let keyframe = sample.keyframe;
                    let sample_entry = sample.sample_entry.cloned();
                    let track_kind = sample.track.kind;

                    let sample_data = self.process_sample_data(
                        track_id,
                        data_offset,
                        data_size,
                        timestamp,
                        keyframe,
                        sample_entry.as_ref(),
                        track_kind,
                    )?;
                    samples.push(sample_data);
                }
                Ok(None) => break,
                Err(DemuxError::InputRequired(_)) => {
                    // 入力を再供給
                    let input = Input {
                        position: 0,
                        data: &self.file_data,
                    };
                    self.demuxer.handle_input(input);
                }
                Err(e) => return Err(e.into()),
            }
        }

        // PMT を書き込む
        let mut pmt_output = Vec::new();
        let mut pmt_writer = TsPacketWriter::new(&mut pmt_output);
        self.write_pmt(&mut pmt_writer)?;

        // 出力を再構成
        let mut final_output = Vec::new();
        final_output.extend_from_slice(&output[..pmt_position]);
        final_output.extend_from_slice(&pmt_output);

        // サンプルを書き込む
        let mut sample_writer = TsPacketWriter::new(&mut final_output);
        for sample in samples {
            self.write_sample(&mut sample_writer, &sample)?;
        }

        Ok(final_output)
    }

    #[expect(clippy::too_many_arguments)]
    fn process_sample_data(
        &mut self,
        track_id: u32,
        data_offset: usize,
        data_size: usize,
        timestamp: u64,
        keyframe: bool,
        sample_entry: Option<&SampleEntry>,
        _track_kind: TrackKind,
    ) -> Result<SampleData, ConvertError> {
        let is_video = Some(track_id) == self.video_track_id;
        let is_audio = Some(track_id) == self.audio_track_id;

        if !is_video && !is_audio {
            return Err(ConvertError::InvalidData("Unknown track".into()));
        }

        // サンプルデータを取得
        let data = self.file_data[data_offset..data_offset + data_size].to_vec();

        // タイムスタンプを 90kHz に変換
        let timescale = if is_video {
            self.video_timescale
        } else {
            self.audio_timescale
        };
        let pts = timestamp * TS_TIMESCALE / timescale as u64;

        if is_video {
            // コーデック情報を抽出
            if let Some(entry) = sample_entry {
                self.extract_video_codec(entry)?;
            }

            // H.264/H.265 データを byte-stream format に変換
            let es_data = self.convert_video_to_byte_stream(&data, keyframe)?;

            Ok(SampleData {
                is_video: true,
                pts,
                dts: None,
                keyframe,
                data: es_data,
            })
        } else {
            // AAC コーデック情報を抽出
            if let Some(entry) = sample_entry {
                self.extract_audio_codec(entry)?;
            }

            // AAC データに ADTS ヘッダを追加
            let es_data = self.add_adts_header(&data)?;

            Ok(SampleData {
                is_video: false,
                pts,
                dts: None,
                keyframe: true,
                data: es_data,
            })
        }
    }

    fn extract_video_codec(&mut self, sample_entry: &SampleEntry) -> Result<(), ConvertError> {
        match sample_entry {
            SampleEntry::Avc1(avc1) => {
                let sps = avc1.avcc_box.sps_list.first().cloned().unwrap_or_default();
                let pps = avc1.avcc_box.pps_list.first().cloned().unwrap_or_default();
                self.video_codec = Some(VideoCodec::H264 { sps, pps });
            }
            SampleEntry::Hev1(hev1) => {
                let mut vps = Vec::new();
                let mut sps = Vec::new();
                let mut pps = Vec::new();
                for array in &hev1.hvcc_box.nalu_arrays {
                    for nalu in &array.nalus {
                        let nal_type = array.nal_unit_type.get() & 0x3F;
                        match nal_type {
                            32 => vps = nalu.clone(),
                            33 => sps = nalu.clone(),
                            34 => pps = nalu.clone(),
                            _ => {}
                        }
                    }
                }
                self.video_codec = Some(VideoCodec::H265 { vps, sps, pps });
            }
            _ => {
                return Err(ConvertError::UnsupportedCodec(
                    "Only H.264/H.265 video is supported".into(),
                ));
            }
        }
        Ok(())
    }

    fn extract_audio_codec(&mut self, sample_entry: &SampleEntry) -> Result<(), ConvertError> {
        match sample_entry {
            SampleEntry::Mp4a(mp4a) => {
                let asc = &mp4a.esds_box.es.dec_config_descr.dec_specific_info;
                if let Some(info) = asc {
                    let payload = &info.payload;
                    if payload.len() >= 2 {
                        let profile = ((payload[0] >> 3) & 0x1F).saturating_sub(1);
                        let sampling_frequency_index =
                            ((payload[0] & 0x07) << 1) | ((payload[1] >> 7) & 0x01);
                        let channel_configuration = (payload[1] >> 3) & 0x0F;

                        let sample_rate = match sampling_frequency_index {
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
                        };

                        self.audio_codec = Some(AudioCodec {
                            profile,
                            sampling_frequency_index,
                            channel_configuration,
                            sample_rate,
                        });
                    }
                }
            }
            _ => {
                return Err(ConvertError::UnsupportedCodec(
                    "Only AAC audio is supported".into(),
                ));
            }
        }
        Ok(())
    }

    fn convert_video_to_byte_stream(
        &self,
        data: &[u8],
        keyframe: bool,
    ) -> Result<Vec<u8>, ConvertError> {
        let mut output = Vec::new();
        let start_code: [u8; 4] = [0, 0, 0, 1];

        // キーフレームの場合、SPS/PPS を先頭に追加
        if keyframe {
            match &self.video_codec {
                Some(VideoCodec::H264 { sps, pps }) => {
                    if !sps.is_empty() {
                        output.extend_from_slice(&start_code);
                        output.extend_from_slice(sps);
                    }
                    if !pps.is_empty() {
                        output.extend_from_slice(&start_code);
                        output.extend_from_slice(pps);
                    }
                }
                Some(VideoCodec::H265 { vps, sps, pps }) => {
                    if !vps.is_empty() {
                        output.extend_from_slice(&start_code);
                        output.extend_from_slice(vps);
                    }
                    if !sps.is_empty() {
                        output.extend_from_slice(&start_code);
                        output.extend_from_slice(sps);
                    }
                    if !pps.is_empty() {
                        output.extend_from_slice(&start_code);
                        output.extend_from_slice(pps);
                    }
                }
                None => {}
            }
        }

        // NAL ユニットを byte-stream format に変換
        let mut offset = 0;
        while offset + 4 <= data.len() {
            let nal_len = u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize;
            offset += 4;

            if offset + nal_len > data.len() {
                break;
            }

            output.extend_from_slice(&start_code);
            output.extend_from_slice(&data[offset..offset + nal_len]);
            offset += nal_len;
        }

        Ok(output)
    }

    fn add_adts_header(&self, data: &[u8]) -> Result<Vec<u8>, ConvertError> {
        let codec = self
            .audio_codec
            .as_ref()
            .ok_or_else(|| ConvertError::InvalidData("No audio codec info".into()))?;

        let frame_len = data.len() + 7; // ADTS ヘッダ 7 バイト

        let mut adts = Vec::with_capacity(frame_len);

        // ADTS ヘッダ
        adts.push(0xFF); // シンクワード
        adts.push(0xF1); // MPEG-4, Layer 0, no CRC

        let profile = codec.profile & 0x03;
        let freq_idx = codec.sampling_frequency_index & 0x0F;
        let channel = codec.channel_configuration & 0x07;

        adts.push((profile << 6) | (freq_idx << 2) | ((channel >> 2) & 0x01));
        adts.push(((channel & 0x03) << 6) | ((frame_len >> 11) as u8 & 0x03));
        adts.push((frame_len >> 3) as u8);
        adts.push(((frame_len & 0x07) as u8) << 5 | 0x1F);
        adts.push(0xFC);

        adts.extend_from_slice(data);

        Ok(adts)
    }

    fn write_pat<W: Write>(&mut self, writer: &mut TsPacketWriter<W>) -> Result<(), ConvertError> {
        let pat = Pat {
            transport_stream_id: 1,
            version_number: VersionNumber::new(),
            table: vec![ProgramAssociation {
                program_num: 1,
                program_map_pid: Pid::new(PID_PMT)?,
            }],
        };

        let packet = TsPacket {
            header: TsHeader {
                transport_error_indicator: false,
                transport_priority: false,
                pid: Pid::new(PID_PAT)?,
                transport_scrambling_control: TransportScramblingControl::NotScrambled,
                continuity_counter: self.pat_cc,
            },
            adaptation_field: None,
            payload: Some(TsPayload::Pat(pat)),
        };
        self.pat_cc.increment();

        writer.write_ts_packet(&packet)?;
        Ok(())
    }

    fn write_pmt<W: Write>(&mut self, writer: &mut TsPacketWriter<W>) -> Result<(), ConvertError> {
        let mut es_info = Vec::new();

        if self.video_track_id.is_some() && self.video_codec.is_some() {
            let stream_type = match &self.video_codec {
                Some(VideoCodec::H264 { .. }) => StreamType::H264,
                Some(VideoCodec::H265 { .. }) => StreamType::H265,
                None => return Err(ConvertError::InvalidData("No video codec".into())),
            };

            es_info.push(EsInfo {
                stream_type,
                elementary_pid: Pid::new(PID_VIDEO)?,
                descriptors: vec![],
            });
        }

        if self.audio_track_id.is_some() && self.audio_codec.is_some() {
            es_info.push(EsInfo {
                stream_type: StreamType::AdtsAac,
                elementary_pid: Pid::new(PID_AUDIO)?,
                descriptors: vec![],
            });
        }

        let pmt = Pmt {
            program_num: 1,
            pcr_pid: if self.video_track_id.is_some() {
                Some(Pid::new(PID_VIDEO)?)
            } else {
                Some(Pid::new(PID_AUDIO)?)
            },
            version_number: VersionNumber::new(),
            program_info: vec![],
            es_info,
        };

        let packet = TsPacket {
            header: TsHeader {
                transport_error_indicator: false,
                transport_priority: false,
                pid: Pid::new(PID_PMT)?,
                transport_scrambling_control: TransportScramblingControl::NotScrambled,
                continuity_counter: self.pmt_cc,
            },
            adaptation_field: None,
            payload: Some(TsPayload::Pmt(pmt)),
        };
        self.pmt_cc.increment();

        writer.write_ts_packet(&packet)?;
        Ok(())
    }

    fn write_sample<W: Write>(
        &mut self,
        writer: &mut TsPacketWriter<W>,
        sample: &SampleData,
    ) -> Result<(), ConvertError> {
        let pid = if sample.is_video {
            PID_VIDEO
        } else {
            PID_AUDIO
        };
        let cc = if sample.is_video {
            &mut self.video_cc
        } else {
            &mut self.audio_cc
        };

        let stream_id = if sample.is_video {
            StreamId::new(0xE0) // video
        } else {
            StreamId::new(0xC0) // audio
        };

        let pts = Timestamp::new(sample.pts)?;

        // PES ヘッダを作成
        let pes_header = PesHeader {
            stream_id,
            priority: false,
            data_alignment_indicator: true,
            copyright: false,
            original_or_copy: true,
            pts: Some(pts),
            dts: sample.dts.map(Timestamp::new).transpose()?,
            escr: None,
        };

        // PES optional header の長さを計算
        // 3 bytes (flags + header_len) + 5 bytes (PTS) + 5 bytes (DTS, optional) + 6 bytes (ESCR, optional)
        let pes_optional_header_len: usize =
            3 + pes_header.pts.map_or(0, |_| 5) + pes_header.dts.map_or(0, |_| 5);

        // PES パケット長を計算 (6 bytes PES header は含まない)
        let pes_packet_len = if sample.data.len() + pes_optional_header_len > 65535 {
            0 // ビデオの場合は 0 を使用
        } else {
            (sample.data.len() + pes_optional_header_len) as u16
        };

        let mut data_offset = 0;
        let mut first_packet = true;

        while data_offset < sample.data.len() || first_packet {
            let mut adaptation_field = None;

            // 最初のパケットで PCR を含める (ビデオキーフレームの場合)
            if first_packet && sample.is_video && sample.keyframe {
                let pcr = ClockReference::from(pts);
                adaptation_field = Some(AdaptationField {
                    discontinuity_indicator: false,
                    random_access_indicator: true,
                    es_priority_indicator: false,
                    pcr: Some(pcr),
                    opcr: None,
                    splice_countdown: None,
                    transport_private_data: vec![],
                    extension: None,
                });
            }

            // Adaptation field のサイズを計算
            // 1 byte (length) + 1 byte (flags) + 6 bytes (PCR, optional)
            let adaptation_len = adaptation_field
                .as_ref()
                .map(|a| 2 + a.pcr.map_or(0, |_| 6))
                .unwrap_or(0);
            let available = TS_PAYLOAD_MAX - adaptation_len;

            let (pes_data, remaining) = if first_packet {
                // 最初のパケットには PES ヘッダを含める
                // PES ヘッダ: 6 bytes (start code + stream_id + packet_len) + optional header
                let pes_header_total = 6 + pes_optional_header_len;
                let max_data = available.saturating_sub(pes_header_total);
                let chunk_len = max_data.min(sample.data.len() - data_offset);

                let pes_bytes = Bytes::new(&sample.data[data_offset..data_offset + chunk_len])
                    .expect("chunk range is within sample data");
                let pes = Pes {
                    header: pes_header.clone(),
                    pes_packet_len,
                    data: pes_bytes,
                };

                data_offset += chunk_len;
                (
                    Some(TsPayload::PesStart(pes)),
                    sample.data.len() - data_offset,
                )
            } else {
                // 継続パケットは Raw データ
                let chunk_len = available.min(sample.data.len() - data_offset);
                let raw_bytes = Bytes::new(&sample.data[data_offset..data_offset + chunk_len])
                    .expect("chunk range is within sample data");
                data_offset += chunk_len;
                (
                    Some(TsPayload::Raw(raw_bytes)),
                    sample.data.len() - data_offset,
                )
            };

            let packet = TsPacket {
                header: TsHeader {
                    transport_error_indicator: false,
                    transport_priority: false,
                    pid: Pid::new(pid)?,
                    transport_scrambling_control: TransportScramblingControl::NotScrambled,
                    continuity_counter: *cc,
                },
                adaptation_field,
                payload: pes_data,
            };
            cc.increment();

            writer.write_ts_packet(&packet)?;
            first_packet = false;

            if remaining == 0 {
                break;
            }
        }

        Ok(())
    }
}

/// 処理済みサンプルデータ
struct SampleData {
    is_video: bool,
    pts: u64,
    dts: Option<u64>,
    keyframe: bool,
    data: Vec<u8>,
}
