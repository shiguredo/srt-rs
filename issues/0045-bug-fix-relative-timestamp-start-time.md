# relative_timestamp が start_time=None 時にタイムスタンプが常時 0 になる

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/fix-relative-timestamp-start-time

## 目的

`src/srt_connection.rs:613-615` の `relative_timestamp` で `start_time` が `None` の場合 `self.start_time.unwrap_or(now)` とし、結果的に `now - now = 0` になる。Listener 側の INDUCTION レスポンス送信時など、`start_time` 未設定のまま呼ばれる可能性があり、相対タイムスタンプが常に 0 になる。

## 現状

```rust
fn relative_timestamp(&self, now: Timestamp) -> u32 {
    let start = self.start_time.unwrap_or(now);
    (now.as_micros() - start.as_micros()) as u32
}
```

## 設計方針

`start_time` が `None` の場合は `0` を返すようにするか、呼び出し元で `start_time` を保証する。

## 完了条件

- `start_time` 未設定時のタイムスタンプが適切な値になっていること
- `cargo test` で全テストが通過すること
