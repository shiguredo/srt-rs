# srt_congestion.rs の死にコードを削除する

- Priority: Low
- Created: 2026-05-14
- Completed: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/change-remove-congestion-dead-code

## 目的

`src/srt_congestion.rs` の `LiveCc` 構造体と `CongestionControl` trait が `src/srt_connection.rs` から一切参照されていない。全 591 行が孤立した死にコードであり、コードベースの保守性を下げている。削除してコードベースを整理する。

## 優先度根拠

死にコードであるため機能に影響はない。保守性の改善が目的であり、緊急性はない。

## 現状

`src/srt_congestion.rs` は以下の型を定義している:

- `AckInfo` 構造体
- `CongestionControl` trait
- `BandwidthMode` 列挙型
- `LiveCc` 構造体（`CongestionControl` の唯一の実装）

これらは `src/lib.rs` で `pub use` されているが、`src/srt_connection.rs` を含むクレート内部のどのモジュールからも使用されていない。

`SrtConnection` の輻輳制御は `SenderBuffer` 内部の `congestion_window: u32` フィールド（初期値 16）と `set_congestion_window` メソッドで実装されている。しかし `srt_connection.rs` は `set_congestion_window` を一度も呼び出しておらず、輻輳ウィンドウは実質的に固定値 16 のまま機能している。`LiveCc` との結合は存在しない。

なお `src/srt_handshake.rs` の `add_congestion_extension` は SRT ハンドシェイクプロトコルの輻輳制御アルゴリズム名（`"live"` 等）をピアに通知する拡張であり、`srt_congestion.rs` の `LiveCc` 実装とは無関係である。

## 設計方針

`src/srt_congestion.rs` を全削除し、`lib.rs` の `mod srt_congestion;` および `pub use srt_congestion::...` も削除する。

`LiveCc` を `SrtConnection` に統合する選択肢もあるが、これは新機能実装であり、死にコード削除とはスコープが異なる。輻輳制御の実装が必要になった場合は別 issue で対応する。

### 修正対象

1. `src/srt_congestion.rs` を削除する
2. `src/lib.rs` の `mod srt_congestion;` を削除する
3. `src/lib.rs` の `pub use srt_congestion::{AckInfo, BandwidthMode, CongestionControl, LiveCc};` を削除する
4. `CHANGES.md` の `## develop` セクションに `[CHANGE]` エントリを追加する:
   - `[CHANGE] 未使用の srt_congestion モジュール（AckInfo, BandwidthMode, CongestionControl, LiveCc）を削除する`

### 修正不要の確認項目

- `src/srt_sender.rs` の `congestion_window` フィールドと `set_congestion_window` メソッドは `LiveCc` とは独立しており、本修正の影響を受けない
- `src/srt_handshake.rs` の `add_congestion_extension` は SRT プロトコルのハンドシェイク拡張であり、`srt_congestion.rs` とは無関係
- `pbt/tests/prop_sender.rs` の `set_congestion_window` テストは `SenderBuffer` の API を直接テストしており、影響なし

### テスト戦略

`srt_congestion.rs` 内の `#[cfg(test)] mod tests`（9 テスト関数、約 200 行）はファイル削除とともに消える。これらのテストは `LiveCc` 単体のロジック検証であり、`SenderBuffer` や `SrtConnection` のテストカバレッジには影響しない。

削除後、`cargo test` で全テストが通過することを確認する。

### 他 issue との依存関係

issue #0016 (`fmt-fix-english-comments-and-test-messages`) の修正対象 7, 8 が `src/srt_congestion.rs:549-551, 586-588` を参照している。本 issue でファイルを削除した場合、#0016 の該当項目は不要になる。#0003 を先に対応すること。

### スコープ外

- `SenderBuffer.congestion_window` が初期値 16 で固定されている問題（輻輳制御が実質未実装）は本 issue のスコープ外。必要であれば別 issue で対応する
- `LiveCc` の `SrtConnection` への統合は新機能実装であり、本 issue のスコープ外

## 完了条件

- `src/srt_congestion.rs` が削除されていること
- `lib.rs` から `srt_congestion` に関する `mod` 宣言と `pub use` が削除されていること
- `cargo check` でコンパイルが通ること
- `cargo test` で全テストが通過すること
- `CHANGES.md` の `## develop` セクションに `[CHANGE]` エントリが追加されていること
- issue #0016 の修正対象 7, 8（`srt_congestion.rs` 参照箇所）が削除されていること

## 解決方法

1. `src/srt_congestion.rs` を全削除
2. `src/lib.rs` の `mod srt_congestion;` を削除
3. `src/lib.rs` の `pub use srt_congestion::{AckInfo, BandwidthMode, CongestionControl, LiveCc};` を削除
4. `CHANGES.md` の `### misc` に `[CHANGE]` エントリを追加
