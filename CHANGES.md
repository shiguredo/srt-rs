# 変更履歴

- UPDATE
  - 後方互換がある変更
- ADD
  - 後方互換がある追加
- CHANGE
  - 後方互換のない変更
- FIX
  - バグ修正

## develop

- [FIX] drop_too_late のドロップ判定をパケット個別の配信時刻に基づいて行うよう修正する
  - @voluntas
- [FIX] unwrap_sek で 8 バイト未満の入力によるパニックを修正する
  - @voluntas
- [FIX] handle_ack の take_while を filter に置き換え、シーケンス番号ラップアラウンド後のバッファリークを修正する
  - @voluntas
- [FIX] Light ACK の type_specific_info を SRT 仕様に従い 0 に設定するよう修正する
  - @voluntas
- [FIX] TSBPD 配信時刻計算を SRT 仕様の TsbpdTimeBase に準拠するよう修正する
  - @voluntas
- [FIX] KeyFlag の KK フィールド判別子値を SRT 仕様に準拠するよう修正する
  - @voluntas
- [FIX] Listener が CONCLUSION 受信時に Caller の Initial Packet Sequence Number を採用するよう修正する
  - @voluntas

### misc

- [CHANGE] sequence_less_than / sequence_greater_than を srt_packet.rs に集約する
  - @voluntas
- [CHANGE] 未使用の srt_congestion モジュール (AckInfo, BandwidthMode, CongestionControl, LiveCc) を削除する
  - @voluntas
