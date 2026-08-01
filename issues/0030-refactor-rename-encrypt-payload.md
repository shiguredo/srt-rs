# encrypt_payload 関数名が誤解を招く

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-rename-encrypt-payload
- Polished: 2026-08-01

## 目的

`src/crypto.rs` の `encrypt_payload` 関数名が「暗号化」を示唆するが、実際には復号化にも同一関数を使用している。AES-CTR の対称性を理由に同一関数で済ませているが、誤解を招く。

## 現状

`encrypt_payload` は `CryptoContext::encrypt` と `CryptoContext::decrypt` の両方から呼ばれる private 関数である。doc コメントは「AES-CTR でペイロードを暗号化/復号化」と正確なのに、関数名だけが暗号化に偏っている。呼び出し箇所は src/crypto.rs 内のみ（`encrypt` / `decrypt` / 単体テスト 2 箇所）で、外部クレートからの使用はない。また、tests/test_crypto.rs と pbt/tests/prop_crypto.rs のコメントにも `encrypt_payload` への言及がある。

## 設計方針

`encrypt_payload` を `apply_aes_ctr` にリネームする (closed/0017 の「他 issue との依存関係」で確定済みの名前)。操作内容 (AES-CTR を適用する) を正確に表す。テスト内の KAT 用独立実装 `aes_ctr_apply` (tests/test_crypto.rs) とは名前が酷似するが、`aes_ctr_apply` は KAT の独立実装としてリネーム対象外である。

なお、`apply_aes_ctr` 内のエラーメッセージ "encryption failed" (復号化経路からも到達しうる) も同様の暗号化バイアスがあるが、本 issue のスコープ外とする。

## 相互作用

- closed/0017 (AES-CTR IV 構築のバグ修正) は先に対応済みであり、関数名・シグネチャは不変のため競合しない
- #0031 (インラインテストの `tests/` への移動) と #0034 (PBT と重複する単体テストの削除) は本 issue の呼び出し箇所を含む単体テストを変更対象としうる。完了条件は「全呼び出し箇所が更新されていること」の汎用表現のため、移動・削除後の配置でも対応できる

## テスト

private 関数のリネームであり挙動は不変のため、新規テストは追加しない。既存の `cargo test` で担保する。

## CHANGES.md

機能に直接影響しない内部構造の変更のため、shiguredo-changelog 規約に従い `### misc` サブセクションに `[CHANGE]` エントリ (例: `[CHANGE] encrypt_payload を apply_aes_ctr に改名する`) を追加する。

## 完了条件

- `encrypt_payload` が `apply_aes_ctr` にリネームされ、全呼び出し箇所 (`encrypt` / `decrypt` / 単体テスト) が更新されていること
- src/・tests/・pbt/ 配下のソースコードとコメントに `encrypt_payload` への言及が残っていないこと (tests/test_crypto.rs と pbt/tests/prop_crypto.rs のコメント言及を含む。issues/ と CHANGES.md は対象外。fuzz/ と crates/ には現状言及がないため対象外)
- KAT 用独立実装 `aes_ctr_apply` がリネームされていないこと (名前の酷似による誤リネームの防止)
- `cargo test` で全テストが通過すること
- CHANGES.md の `### misc` セクションに `[CHANGE]` エントリが追加されていること
