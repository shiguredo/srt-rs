# MSRV を検証する CI ジョブがない

- Priority: Medium
- Created: 2026-08-29
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/update-ci-msrv-check
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

`Cargo.toml` は `rust-version = "1.93"` を宣言しているが、CI は stable のみで検証するため、MSRV を超えた要求 (コードの言語機能、マニフェストの記法、依存クレートの MSRV) が混入しても誰にも検出されない。issue 0078 で判明した「複数行 inline table のため cargo 1.93 ではワークスペース自体がパース不能」という不整合は、MSRV 検証ジョブがあれば起票前に落ちていた。宣言している MSRV を実際に検証する経路を追加する。

## 現状

- `.github/workflows/ci.yml` の `ci` ジョブは matrix で 4 プラットフォームを回すが、`rustup update stable` に加え `shiguredo/github-actions/.github/actions/rust-cache@main` を `toolchain: stable` で使っており、MSRV ツールチェーンでのビルドは行わない
- `Makefile`・`prek.toml` も同様に既定ツールチェーン (stable) を使う
- `rust-toolchain.toml` は `channel = "stable"` のみ。MSRV を参照する箇所は `Cargo.toml` の `rust-version = "1.93"` だけである
- 実測 (issue 0078 の検証時): MSRV 1.93 の cargo では `cargo +1.93.1 metadata --offline --no-deps` が失敗し、`cargo +1.94.1` では成功する。ワークスペース全体がロードできない状態を CI は検出できていない
- MSRV 検証の経路がないため、`rust-version` の記載自体が実態と切り離されている (workspace メンバには `rust-version` が無く、ルートのみ。issue 0080)

## 設計方針

- `.github/workflows/ci.yml` に `MSRV` ジョブを 1 本追加する。runner はビルドツールを必要とするため既存 `ci` ジョブと同じ `ubuntu-24.04` を使う (`ubuntu-slim` は要件を満たさない可能性が高い)。マトリクスは回さない (MSRV は 1 バージョンで足りる)
- toolchain の準備は GitHub-hosted runner の `rustup` を使い、外部 action は追加しない (shiguredo-github-actions 規約「Rust toolchain の準備だけを目的とした外部 action は追加しないこと」)

  ```yaml
  - run: rustup toolchain install 1.93 --profile minimal
  - run: cargo +1.93 check --workspace
  - run: cargo +1.93 test --workspace
  ```

- 依存クレートが MSRV より新しい rustc を要求する場合は、その最小要求バージョンを実測で特定し、shiguredo-rust が規定する MSRV 1.93 を維持できるかを判断する。維持できない場合は MSRV を引き上げる変更を別 issue として切り出し、本 issue の PR に混ぜない
- MSRV の値をワークフローに直書きする際は、`Cargo.toml` の `rust-version` とズレたら気づけるようにする。ワークフロー内のコメントで対応元を明示するか、`Cargo.toml` から読み取る形にする。読み取りに外部ツールを使わないこと (`cargo metadata` か `grep`/`sed` 程度の shell で足りる)

## 完了条件

- `.github/workflows/ci.yml` に MSRV ツールチェーンで `cargo check --workspace` と `cargo test --workspace` を実行するジョブが存在すること
- そのジョブが MSRV 未対応の記法・コードに対して実際に失敗することを確認できること (一時的に MSRV 超えの記法を持ち込んだ検証をローカルまたは一時ブランチで実施し、結果を PR 本文に書く)
- MSRV ツールチェーンの準備に新規の外部 action を追加していないこと
- 既存の `ci` ジョブ (4 プラットフォームの stable) を壊していないこと。PR の CI が緑になること
- `CHANGES.md` の `## develop` に該当エントリが追加されていること

## CHANGES.md

CI 設定の変更であり機能へ直接の影響はないため `### misc` に `[UPDATE]` で記載する。

```
- [UPDATE] MSRV で cargo check / cargo test を検証する CI ジョブを追加する
  - @voluntas
```

## 相互作用

- issue 0078 (MSRV 1.93 で examples の Cargo.toml が読めない) が先。0078 が未対応のままだと本ジョブは即失敗する
- issue 0080 (workspace メンバの `rust-version` 未設定) が後続。メンバにも MSRV が宣言されると、本ジョブの検証対象がより正確になる
- issue 0079 (CI と Makefile の clippy に `--all-targets` が無い) と同じ `.github/workflows/ci.yml` を触る。併せて 1 ブランチにまとめず、それぞれ独立した PR で対応する
