# relative_timestamp の start_time 未設定時の処理が unwrap_or(now) で意図が不明瞭

- Priority: Medium
- Created: 2026-05-14
- Completed: 2026-08-15
- Model: DeepSeek V4 Pro
- Branch: feature/fix-relative-timestamp-start-time
- Polished: 2026-08-15

## 目的

`src/srt_connection.rs` の `relative_timestamp` 関数で `start_time` が `None` の場合 `self.start_time.unwrap_or(now)` とし、結果的に `now - now = 0` になる。この挙動自体（0 を返すこと）は結果として正しいが、`unwrap_or(now)` による簡易的な実装では意図が不明瞭で、`start_time` が未設定のケースを意図的に処理していることがコードから読み取れない。Listener 側のハンドシェイク (`handle_handshake_listener`) では `send_induction_response` と `send_conclusion_response` の呼び出し時点で `start_time` が未設定 (両関数内で `relative_timestamp` が呼ばれる) のため、`unwrap_or(now)` に依存している。

## 現状

`relative_timestamp` 関数 (`src/srt_connection.rs`):

```rust
fn relative_timestamp(&self, now: Timestamp) -> u32 {
    let start = self.start_time.unwrap_or(now);
    (now.as_micros() - start.as_micros()) as u32
}
```

`handle_handshake_listener` 内の呼び出し順序:
1. `send_induction_response(now)` → `relative_timestamp(now)` → `start_time` は `None` → タイムスタンプ 0
2. `send_conclusion_response(now)` → `relative_timestamp(now)` → `start_time` はまだ `None` → タイムスタンプ 0
3. `self.start_time = Some(now)` がこの後で設定される

## 設計方針

`start_time` が `None` の場合は `relative_timestamp` が `0` を返すように変更する（`unwrap_or(now)` の代わりに `start_time.map_or(0, |s| now.as_micros().saturating_sub(s.as_micros())) as u32` とする）。この選択の根拠:

- 呼び出し元で `start_time` を保証する方法（ハンドシェイク開始時に `start_time` を設定する）は、`handle_handshake_listener` が複数のハンドシェイク試行を処理する可能性があるため、`start_time` の上書きタイミングの管理が複雑になる
- `0` を返す方式は防御的で、`start_time` が未設定のまま呼ばれるあらゆるケースをカバーする
- ハンドシェイクパケットのタイムスタンプは INDUCTION リクエスト (`hsreq_timestamp`) から TSBPD 時刻基準を計算するため、レスポンス側のタイムスタンプが 0 でも TSBPD の動作に影響しない

## 完了条件

- `relative_timestamp` が `start_time` が `None` の場合に `0` を返すこと (明示的に `map_or(0, ...)` で処理されていること)
- `send_induction_response` と `send_conclusion_response` の両方のハンドシェイクパケットでタイムスタンプが 0 になることが意図的な値であることを検証するテストが追加されていること
- `cargo test` で全テストが通過すること

## CHANGES.md

バグ修正のため、`[FIX]` エントリ (`[FIX] relative_timestamp が start_time 未設定時に 0 を返すよう修正する`。担当者行を付けて追加すること) を追加する。

## 解決方法

- `relative_timestamp` を `unwrap_or(now)` から `map_or(0, ...)` に変更し、`start_time` が `None` の場合に明示的に `0` を返すようにした
- `saturating_sub` を導入し、`start_time > now` 時の underflow を防止した
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリを追加した
