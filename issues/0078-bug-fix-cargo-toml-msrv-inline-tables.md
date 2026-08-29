# MSRV 1.93 の cargo が examples の Cargo.toml を読めない

- Priority: High
- Created: 2026-08-29
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/fix-cargo-toml-msrv-inline-tables
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

ルート `Cargo.toml` は `rust-version = "1.93"` を宣言しているが、`examples/srt_caller/Cargo.toml` と `examples/srt_listener/Cargo.toml` の `tokio` 依存が複数行にまたがる inline table で書かれているため、cargo 1.93 ではワークスペース自体が読み込めない。宣言された MSRV が実質的に成立しておらず、1.93 を想定した利用側・CI の検証がすべて不可能な状態になっている。

```
error: newlines are unsupported in inline tables, expected nothing
  --> examples/srt_caller/Cargo.toml:13:10
   |
13 |   tokio = {
   |  __________^
```

複数行 inline table を許容する TOML の扱い (TOML 1.1 系の記法) は cargo 1.94 以降でのみ機能する。本リポジトリの CI とローカル開発は `rust-toolchain.toml` 経由の stable のみを使うため、この不整合はどの検証経路でも検出されない。

## 現状

- 複数行 inline table を使うのは `examples/srt_caller/Cargo.toml` と `examples/srt_listener/Cargo.toml` の `tokio = { ... }` の 2 箇所のみ (他のマニフェストは 1 行 inline table か通常 table)
- ルート `Cargo.toml` の `[workspace] members` は `crates/c-api`、`examples/srt_caller`、`examples/srt_listener`、`pbt` を含み、`fuzz` を除外する。メンバのどれか 1 つでもパース不能だと `cargo metadata` がワークスペース全体で失敗する (`pbt/Cargo.toml` を直接指定した場合も同じエラーになる)
- 実測結果:
  - `cargo +1.93.1 metadata --offline --no-deps -q` → 終了コード 101 (上記エラー)
  - `cargo +1.94.1 metadata --offline --no-deps -q` → 終了コード 0
- `rust-toolchain.toml` は `channel = "stable"` のみで、MSRV を検証するジョブは `.github/workflows/ci.yml` に存在しない

## 設計方針

マニフェスト側を MSRV 準拠の記法に直す (cargo 1.93 でパースできる形にする)。MSRV を 1.94 へ引き上げる案は、shiguredo-rust が MSRV を 1.93 と規定しているため採用しない。

- `tokio` の依存宣言を `[dependencies]` 配下の 1 行 inline table か、`[dependencies.tokio]` による通常 table に変更する。既存の他のメンバ (`tracing-subscriber = { version = "0.3", features = ["env-filter"] }`) と揃う 1 行 inline table を採用する
- tombi のフォーマット設定がこの記法を維持することを確認する (`prek run --all-files` で書き換えられないこと)
- MSRV を検証する CI ジョブの追加は本 issue の対象としない (リポジトリ設定の変更を伴う別作業)。ただし本修正だけでも再発は防げないため、MSRV 検証ジョブは別途立てる

## 完了条件

- `cargo +1.93.1 metadata --offline --no-deps -q` が終了コード 0 で通ること
- `cargo +1.93.1 check --workspace` が通ること (1.93 でビルドも成立することの確認)。依存クレート側の都合で通らない場合は、その旨とエラー内容を issue 本文に追記し、`metadata` 通過を必須条件、`check` 通過を別途判断とする
- 複数行 inline table がリポジトリ内のいずれのマニフェストにも残っていないこと
- `prek run --all-files` (tombi-format を含む) で書き換えが発生しないこと
- stable での既存検証 (`cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace`) が通過すること

## テスト

マニフェスト記法の修正であり、Rust コードのテスト追加は不要。検証は上記の cargo コマンドの終了コードで担保する。

## CHANGES.md

機能に影響しないマニフェスト記法の修正のため `### misc` に `[UPDATE]` で記載する。

```
- [UPDATE] examples の Cargo.toml を MSRV 1.93 でパースできる記法に修正する
  - @voluntas
```

## 相互作用

- issue 0080 (workspace メンバの `rust-version` 未設定) は同じマニフェスト群を触るため直列にする。MSRV を実際に成立させる記法の修正 (本 issue) が先
- issue 0076 (C API 削除) はメンバ一覧を変更するため、`[workspace] members` を編集する点で重なる。先後は問わないがコンフリクトに注意する
- MSRV 検証ジョブの追加は本 issue の対象外。無いままでは同種の不整合が再発する
