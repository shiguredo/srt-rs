# 双方向通信で送受信の鍵が 1 つの CryptoContext を共有する設計を見直す

- Priority: High
- Created: 2026-08-30
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/change-split-crypto-context-tx-rx-keys
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

双方向通信において 1 つの `SrtConnection` の `CryptoContext` が送信 (encrypt) と受信 (decrypt) の鍵状態を兼ねているため、自分の KM リフレッシュサイクルの `decommission_old_key()` が、ピアからの受信復号にまだ必要な鍵をゼロクリアし得る設計を見直す。受信パケットの復号失敗はデータロスに直結する。

## 現状

- `src/crypto.rs` の `CryptoContext` は `current_key` / `sek_even` / `sek_odd` を 1 セットしか持たず、`encrypt` (送信) と `decrypt` (受信) が同じインスタンスを共有する
- `decommission_old_key()` は `current_key.other()` 側の鍵を `fill(0)` でゼロクリアする。これは自分の送信サイクル (2^25 + 4000 パケット) に基づく廃棄であり、ピアが同じタイミングで鍵を切り替えている保証はない (ピアの暗号化カウンタは独立して進む)
- 根拠資料: draft-sharabayko-srt.md「Encryption」セクション内「KM Refresh」サブセクション。even / odd の 2 鍵は Pre-Announcement Period の 2 倍分並行して生存することが意図されており、受信側は Data パケットの KK フィールドで鍵を判別して復号する。節構成・表現は将来変更される可能性がある
- 双方向の通信テスト (`tests/test_srt_connection.rs` の `test_bidirectional_communication` 等) は鍵リフレッシュ閾値 (2^25 - 4000 パケット) に達しないため、問題は顕在化しない

## 設計方針

送信用と受信用で鍵状態を分離する (例: `SrtConnection` に送信用と受信用の `CryptoContext` を持つ、または `CryptoContext` 内部で送信 / 受信の鍵セットを分ける) のが一案。libsrt (haicrypt) が鍵の寿命をどう管理しているかを調査してから設計を確定する。設計判断が必要なため、調査結果が出るまでは方針を固定しない。設計次第では公開 API やワイヤフォーマットには影響しない内部変更で済む見込みだが、確認したうえで判断する。

## 完了条件

- 双方向通信で一方が鍵リフレッシュを進めても、もう一方の受信復号が失敗しないこと
- 設計方針が確定し、双方向で複数サイクルの鍵リフレッシュを回すテストが追加されていること
- `cargo test` で全テストが通過すること
