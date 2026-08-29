# CI と Makefile の cargo clippy に --all-targets が無く prek と不一致

- Priority: Medium
- Created: 2026-08-29
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/update-ci-clippy-all-targets
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

ローカルフック (prek) と CI と Makefile で `cargo clippy` の対象ターゲットが食い違っており、`tests/`・`pbt/`・`examples/` の lint が CI を素通りしている。結果として、テストコードに警告を混入させても CI は緑になり、マージ後にローカルの `pre-push` フックで初めて落ちる、という検証順序の逆転が起きる。

さらに issue 0053 の完了条件のように「`cargo clippy --workspace --all-targets -- -D warnings` が通過すること」を要求する issue ほど、CI 単体ではその条件を検証できない。

## 現状

- `.github/workflows/ci.yml` の `ci` ジョブ: `cargo fmt --all --check` → `cargo check --workspace` → `cargo test --workspace` → `cargo clippy --workspace -- -D warnings` (`--all-targets` なし)
- `Makefile` の `clippy` ターゲット: `cargo clippy --workspace -- -D warnings` (`--all-targets` なし)
- `prek.toml` の `cargo-clippy` フック: `cargo clippy --workspace --all-targets -- -D warnings` (`--all-targets` あり)。`cargo test` は `pre-push` ステージのみ
- `--all-targets` が無いと `tests/*.rs`、`pbt/tests/*.rs`、`examples/` の各ターゲットが lint されない。実際のところ、issue 0049 で `tests/test_crypto.rs` に追加したテストコードは CI の clippy 対象外であった (ローカルでは prek により検出されていた)
- `crates/c-api` は `crate-type = ["cdylib", "staticlib"]` のみで `--all-targets` を付けても lint 対象は lib ターゲットだけ (テストバイナリを持たない)

## 設計方針

CI と Makefile を prek と同じコマンドに寄せる。lint 対象を広げる方向の変更であり、コードの挙動は変わらない。

- `.github/workflows/ci.yml` の clippy ステップを `cargo clippy --workspace --all-targets -- -D warnings` に変更する
- `Makefile` の `clippy` ターゲットを同じコマンドに合わせる (`make clippy` が CI と同じ結果になるようにする)
- `cargo check --workspace` 側は変更しない (対象を広げるなら別途判断する。clippy が `--all-targets` を通れば check の目的は大部分カバーされる)
- 変更前に、ローカルで `cargo clippy --workspace --all-targets -- -D warnings` が通ることを確認する。既存コードに警告がある場合は、その警告を本 issue の変更内で直すか、別の修正 issue に立てるかを判断して取り込む

## 完了条件

- `.github/workflows/ci.yml` の clippy コマンドに `--all-targets` が付いていること
- `Makefile` の `clippy` ターゲットが同じコマンドになっていること
- `cargo clippy --workspace --all-targets -- -D warnings` がローカルで通過すること
- CI の `ci` ジョブが 4 プラットフォーム (ubuntu-24.04 / ubuntu-24.04-arm / windows-2025 / macos-26) すべてで通過すること
- `CHANGES.md` の `## develop` に該当エントリが追加されていること

## CHANGES.md

CI・ビルド設定の変更であり機能へ直接の影響はないため `### misc` に `[UPDATE]` で記載する。

```
- [UPDATE] CI と Makefile の cargo clippy に --all-targets を追加する
  - @voluntas
```

## 相互作用

- issue 0078 (MSRV 1.93 で examples の Cargo.toml が読めない) とは別問題。同じマニフェスト群を触るため、MSRV を実際に成立させる記法修正 (0078) が先。clippy を `--all-targets` 化すると、MSRV 非対応の記法による警告も CI に拾われる可能性が高まる
- issue 0076 (C API 削除) が先に入る場合、`--all-targets` 化の影響対象から c-api が消える。先後は問わない
- prek のフック定義は変更しない (既に正しいコマンドになっている)
