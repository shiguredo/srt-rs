# srt_caller の例で provide_new_sek のエラーを再試行する

- Priority: Low
- Created: 2026-08-30
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/refactor-example-provide-new-sek-retry
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

`examples/srt_caller` が `provide_new_sek` のエラーをログするだけで再試行しないため、ライブラリの契約 (エラー後も `KeyRefreshNeeded` イベントは再発行されず、利用者が再試行する責任を持つ) を模範的に示す形に修正する。公式サンプルは利用者が契約を理解するための参照実装である。

## 現状

- `examples/srt_caller/src/main.rs` は `KeyRefreshNeeded` イベントを受信すると乱数で新しい SEK を生成して `provide_new_sek` を呼び、`if let Err(e) = ...` で `error!` ログを出すのみで再試行しない
- `SrtConnection::provide_new_sek` のドキュメントには「イベントは 1 サイクルに 1 回しか発行されないため、このメソッドがエラーを返した場合もイベントは再発行されない。利用者はエラーを処理して再度呼び出すこと」と記載されている
- エラーになるのは SEK 長の不一致などであり、発生確率は低い。ただし発生した場合にサンプルの挙動は契約に沿わない

## 設計方針

エラー時にリトライする形へ修正する。example を複雑にしすぎない範囲で、契約 (エラー後に利用者が再試行する) を満たす実装例を示す。リトライ方針 (同 SEK での即時再試行、新しい SEK での再試行のいずれが自然か) は `provide_new_sek` のエラー条件を踏まえて決める。

## 完了条件

- `examples/srt_caller` が `provide_new_sek` のエラー後に契約に沿った処理 (再試行) を行うこと
- example のビルドが通ること
