# SYN Cookie のデフォルト値が 0 で Cookie 検証が実質無効

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-syn-cookie-default

## 目的

`src/srt_connection.rs:828` で `self.syn_cookie = self.options.syn_cookie.unwrap_or(0)` としており、`syn_cookie` が未設定の場合 Cookie=0 で送受信され、CONCLUSION リクエストの検証 `hs.syn_cookie != self.syn_cookie` は `0 != 0` で常に通過する。ハンドシェイク Cookie の DoS 防御機能が実質無効。

## 優先度根拠

攻撃者が CONCLUSION リクエストを偽装可能になり、サービス拒否攻撃のリスクがある。ただし攻撃には有効な INDUCTION レスポンスの受信が必要で、実際の悪用難易度は高い。

## 現状

```rust
// src/srt_connection.rs:828
self.syn_cookie = self.options.syn_cookie.unwrap_or(0);
```

## 設計方針

`syn_cookie` が未設定の場合は乱数生成する。`ConnectionOptions` に `#[cfg(test)]` 用のテストモードを設け、テスト時は固定値を使用可能にする。

## 完了条件

- `syn_cookie` 未設定時に乱数が生成されること
- 既存テストが引き続き通過すること
- `cargo test` で全テストが通過すること
