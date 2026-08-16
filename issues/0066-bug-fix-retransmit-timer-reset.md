# handle_timer の Retransmit タイマーが処理後に再設定されない

- Priority: High
- Created: 2026-08-16
- Branch: feature/bug-fix-retransmit-timer-reset

## 目的

`src/srt_connection.rs` の `handle_timer` メソッド内で、`Ack`、`Nak`、`Keepalive` タイマーは処理後に `SetTimer` で再設定されるが、`Retransmit` タイマーだけ再設定されない。タイマー駆動の再送を期待する場合、1 回のタイマー発火で停止する。

## 現状

```rust
TimerId::Retransmit => {
    if self.state == ConnectionState::Connected {
        self.process_retransmit(now);
    }
}
```

他のタイマー (`Ack`、`Nak`、`Keepalive`) は処理後に `SetTimer` で次のタイマーを設定している。

## 設計方針

再送タイマーを再設定するか、タイマー駆動の再送が不要であることをコメントで明示する。`Retransmit` は `handle_nak` から即時呼び出しされるため、NAK 駆動では問題ないが、タイマー駆動の再送が期待されるシナリオでは再設定が必要。

## 完了条件

- `Retransmit` タイマーの再設定が追加されるか、タイマー駆動の再送が不要であることをコメントで明示すること
- `cargo test` で全テストが通過すること
