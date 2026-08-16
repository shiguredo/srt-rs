# add_millis の millis * 1000 がオーバーフローする

- Created: 2026-08-16
- Branch: feature/fix-add-millis-overflow
- Polished: 2026-08-16

## 目的

`src/time.rs` の `Timestamp::add_millis` メソッド内で、`millis * 1000` が `u64::MAX` を超える場合、debug ビルドでは panic、release ビルドでは wrap する。`add_millis` は `pub` メソッドとして公開されており、外部呼び出しで panic が発生する可能性がある。

`millis * 1000` が `u64::MAX` を超えるのは `millis > 18,446,744,073,709,551` (u64::MAX ÷ 1000 の商) の場合である。

## 現状

```rust
pub fn add_millis(&self, millis: u64) -> Self {
    self.add_micros(millis * 1000)
}
```

## 設計方針

`millis * 1000` を `millis.saturating_mul(1000)` に変更する。`add_micros` も内部で `saturating_add` を使用しているため、オーバーフロー時の saturate 動作に一貫性がある。saturate により、過大な millis が渡された場合も panic や wrap せず、`u64::MAX` (マイクロ秒) に張り付く。

## 完了条件

- `millis * 1000` が `millis.saturating_mul(1000)` に変更されていること
- オーバーフロー境界を超える `millis` を `add_millis` に渡しても panic せず、結果が `u64::MAX` マイクロ秒に saturate することを検証するテストが追加されていること (境界値: `millis` = 18,446,744,073,709,552)
- `CHANGES.md` の `## develop` セクションに `[FIX]` エントリが追加されていること
- `cargo test` で全テストが通過すること

## 解決方法

`src/time.rs` の `Timestamp::add_millis` メソッド内の `millis * 1000` を `millis.saturating_mul(1000)` に変更する。テストは `src/time.rs` 内の `#[cfg(test)]` モジュールに追加し、オーバーフロー境界 (`millis` = 18,446,744,073,709,552) で panic せず saturate することを検証する。

なお、open issue 0038 (`tests/` へのテスト追加) は `add_millis` の PBT (任意の `millis` に対する saturate 挙動) を計画しているため、本 issue の境界値テスト (単体テスト) と役割分担する。実装順は本 issue (バグ修正) が先である。
