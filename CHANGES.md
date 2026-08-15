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

- [CHANGE] デバッグ出力を eprintln! から tracing に移行し ConnectionOptions::debug を削除する
  - @voluntas
- [FIX] 受信バッファがシーケンス番号ラップアラウンド境界でパケットを誤った順序で配送する問題を修正する
  - @voluntas
- [FIX] AES-CTR のカウンタブロック構築を SRT 仕様に準拠するよう修正する
  - @voluntas
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
- [FIX] TSBPD wrapping period の終了判定を受信時ではなくパケット配信時に行うよう修正し、ラップ境界の配信時刻計算を整合させる
  - @voluntas
- [FIX] drop_expired の TLPKTDROP 閾値を仕様の推奨値 (max(1.25 * latency, 1 秒)) に合わせる
  - @voluntas
- [FIX] drop_too_late の未受信パケット推定配送時刻を次側パケットの delivery_time に基づくよう修正する
  - @voluntas
- [FIX] wrapping period 終了範囲の上限を開区間 (60 秒を含まない) に修正する
  - @voluntas
- [FIX] relative_timestamp が start_time 未設定時に 0 を返すよう修正する
  - @voluntas
- [FIX] Listener が CONCLUSION 受信時に Caller の Initial Packet Sequence Number を採用するよう修正する
  - @voluntas

### misc

- [CHANGE] MSRV (rust-version) を 1.88 から 1.93 に上げる
  - @voluntas
- [CHANGE] sequence_less_than / sequence_greater_than を srt_packet.rs に集約する
  - @voluntas
- [CHANGE] 未使用の srt_congestion モジュール (AckInfo, BandwidthMode, CongestionControl, LiveCc) を削除する
  - @voluntas
- [CHANGE] ByteSliceExt / VecExt トレイトを廃止し、buf モジュールの free 関数に置き換える
  - @voluntas
- [UPDATE] CI / Release ワークフローの Slack 通知を shiguredo/github-actions の slack-notify に移行する
  - @voluntas
- [UPDATE] examples のユーザー向け表示を eprintln! から tracing に移行する
  - @voluntas
- [UPDATE] ErrorKind から #[non_exhaustive] を除去する
  - @voluntas
- [UPDATE] Copy な enum のメソッドを値 (self) で受け取るよう変更する
  - @voluntas
- [UPDATE] デコード時に入力バイナリ由来の Vec::with_capacity を使用しないよう変更する
  - @voluntas
- [UPDATE] #[allow] を #[expect] に置き換え、不要になった抑制を削除する
  - @voluntas
- [UPDATE] .unwrap() を情報量のある .expect() に置き換える
  - @voluntas
- [UPDATE] tests/sansio_test.rs を tests/test_srt_connection.rs にリネームする
  - @voluntas
