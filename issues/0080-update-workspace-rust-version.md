# workspace メンバの Cargo.toml に rust-version が無い

- Priority: Low
- Created: 2026-08-29
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/update-workspace-rust-version
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

shiguredo-rust 規約は「`Cargo.toml` の `rust-version` に `"1.93"` を明記すること」を求めているが、記載があるのは公開 crate であるルート `Cargo.toml` だけである。開発用メンバ (`examples/`、`pbt/`、`fuzz/`) にも MSRV を宣言しておくと、toolchain を切り替えて走らせたときに cargo 側で MSRV 違反が明示的にエラーになる。issue 0078 で判明した「MSRV 1.93 の cargo ではワークスペース自体が読み込めない」と種の不整合も、MSRV 宣言があれば cargo により早期に検出できる。

## 現状

- `rust-version = "1.93"` はルート `Cargo.toml` の `[package]` にのみ存在する
- ルート `Cargo.toml` の `[workspace]` セクションは `members` と `exclude` のみで、`[workspace.package]` は存在しない (つまり継承の受け口が無い)
- `rust-version` を持たないメンバ: `examples/srt_caller/Cargo.toml`、`examples/srt_listener/Cargo.toml`、`pbt/Cargo.toml` (いずれも `publish = false`)
- `fuzz/Cargo.toml` は `publish = false` かつルートの `[workspace] exclude` 対象で、`[package.metadata] cargo-fuzz = true` を持つ
- 全メンバが `edition = "2024"` を個別に宣言している (`edition` も重複記載だが、本 issue の対象外)

## 設計方針

- ルート `Cargo.toml` に `[workspace.package]` を新設し、`rust-version = "1.93"` を置く。ルートの `[package]` 側の `rust-version` は `rust-version = { workspace = true }` に置き換える
- ワークスペースメンバーである `examples/srt_caller`、`examples/srt_listener`、`pbt` の `[package]` に `rust-version = { workspace = true }` を追加する
- `fuzz` はワークスペースから除外されているため継承が使えない。`fuzz/Cargo.toml` に `rust-version = "1.93"` を直接記載する
- `edition` は変更しない (1 issue 1 目的)。依存バージョンの指定方法も変更しない

## 完了条件

- ルート `Cargo.toml` に `[workspace.package]` の `rust-version = "1.93"` があり、ルートの `[package]` がそれを継承していること
- `examples/srt_caller`、`examples/srt_listener`、`pbt` の各 `[package]` が `rust-version = { workspace = true }` を持つこと
- `fuzz/Cargo.toml` に `rust-version = "1.93"` が直接記載されていること
- `cargo metadata --no-deps` で各パッケージの `rust_version` が `1.93` として解決されること
- `cargo check --workspace` / `cargo test --workspace` / `cargo clippy --workspace --all-targets -- -D warnings` が通過すること
- `cargo +nightly fuzz build` 対象の `fuzz` パッケージが壊れていないこと (MSRV 記載のみでビルド挙動は変わらないことの確認)
- `prek run --all-files` (tombi-lint / tombi-format を含む) が通過すること

## テスト

マニフェストのメタデータ追加であり、Rust コードのテスト追加は不要。検証は `cargo metadata --no-deps` での `rust_version` 解決結果と、既存のビルド・lint・テストの通過で担保する。

## CHANGES.md

機能に影響しない設定変更のため `### misc` に `[UPDATE]` で記載する。

```
- [UPDATE] workspace メンバの Cargo.toml に rust-version を追加する
  - @voluntas
```

## 相互作用

- issue 0076 (C API 削除) より後に行う。`crates/c-api` には `rust-version` を追加しない (削除予定のため)
- issue 0078 (MSRV 1.93 で examples の Cargo.toml が読めない) を先にマージしておく。MSRV が実質破綻した状態で宣言を増やすと、cargo がより厳しくエラーを出すだけになる
- MSRV を検証する CI ジョブの追加は本 issue の対象外 (issue 0078 に別途記載)
