# add_millis の millis * 1000 がオーバーフローする

- Priority: High
- Created: 2026-08-16
- Branch: feature/bug-fix-add-millis-overflow

## 目的

`src/time.rs` の `Timestamp::add_millis` メソッド内で、`millis * 1000` が `u64::MAX` を超える場合、debug ビルドでは panic、release ビルドでは wrap する。`add_millis` は `pub` メソッドとして公開されており、外部呼び出しで panic が発生する可能性がある。

## 現状

```rust
pub fn add_millis(&self, millis: u64) -> Self {
    self.add_micros(millis * 1000)
}
```

## 設計方針

`millis.saturating_mul(1000)` に変更する。`add_micros` も内部で `saturating_add` を使用しているため、一貫性がある。

## 完了条件

- `millis * 1000` が `millis.saturating_mul(1000)` に変更されていること
- `cargo test` で全テストが通過すること
