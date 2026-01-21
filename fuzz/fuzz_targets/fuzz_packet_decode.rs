#![no_main]

use libfuzzer_sys::fuzz_target;
use shiguredo_srt::SrtPacket;

fuzz_target!(|data: &[u8]| {
    // パケットデコードを試行
    // エラーは無視 (パニックしないことを確認)
    let _ = SrtPacket::decode(data);
});
