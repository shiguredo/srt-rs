# add_millis の millis * 1000 がオーバーフローする

- Created: 2026-08-16
- Completed: 2026-08-29
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

- `src/time.rs` の `Timestamp::add_millis` 内の `millis * 1000` を `millis.saturating_mul(1000)` に変更した。`///` には「換算値と元のマイクロ秒値の合計が `u64` の範囲に収まらない場合は panic せず `u64::MAX` マイクロ秒で飽和する」という契約を記載し、`//` にはエラーを返さず飽和させる理由 (`add_micros` も `saturating_add` を使う型の一貫性) を書いた
- `src/time.rs` 内の `#[cfg(test)]` モジュールに境界値テスト 3 件を追加した
  - `test_add_millis_saturates_over_boundary`: 乗算が溢れる境界 (`millis` = 18,446,744,073,709,552) と `millis` = `u64::MAX` の極値で、panic せず結果が `u64::MAX` になること
  - `test_add_millis_saturates_on_sum_overflow`: 換算値が範囲内でも元のマイクロ秒値との合計が溢れる経路で飽和すること
  - `test_add_millis_below_boundary_is_exact`: 境界より 1 小さい値では飽和せず `(u64::MAX / 1000) * 1000` の正確な値になること (飽和が早すぎる実装の検出)、および飽和領域外で元の値が 0 以外となるケース
- 変異検証で回帰検出を確認した。`saturating_mul` を元に戻すと 1 件目が debug では panic、release では wrap 値 (391) で失敗する。換算係数を `1_000_000` に壊すと 3 件目が、`saturating_add` を `+` に壊すと 1・2 件目が失敗する
- open issue 0038 の PBT (任意の `millis` に対する換算の等価性) と役割分担するため、本 issue は境界値・極値の単体テストに限定した。実装順は本 issue が先
- 関連する別経路として、`u64::MAX` 近傍の `now` を受信側に渡すと `src/srt_receiver.rs` の TSBPD 配信時刻計算 (非飽和の `+`) で debug は panic、release は wrap する問題が残る (旧来からの既知経路)。本 issue のスコープ外として別途 issue を立てる
