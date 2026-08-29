# C API (crates/c-api) を削除して Rust ライブラリのみにする

- Created: 2026-08-29
- Completed: {YYYY-MM-DD} (例: 2024-07-01)
- Branch: feature/remove-c-api
- Polished: {YYYY-MM-DD} (例: 2024-07-15)

## 目的

`crates/c-api` は本体の公開 API を C ABI で覆す薄いラッパーだが、C 側の利用者はおらず、README も Rust の利用例しか記載していない。それでも crate を維持し続けることで以下の負担が恒久的に発生している。

- 裸の生ポインタ参照・`CStr::from_ptr`・`slice::from_raw_parts`・`Box::from_raw` を密集して抱えることになる (issue 0053 で対応した lint 抑制は crate レベルであり、将来追加される unsafe 関数も黙って抑止される)
- cbindgen による `crates/c-api/include/srt.h` をソースツリーにコミットし続ける必要がある。`build.rs` はビルドのたびに同じファイルへ書き戻すが、CI は生成物の差分検証を持たない
- リリースのたびに 8 プラットフォーム分の C アーティファクトをビルドする `upload-assets` ジョブが走る
- `crate-type = ["cdylib", "staticlib"]` のみで `rlib` を持たないため、`crates/c-api/tests/` に統合テストを書けない
- 本体の API 変更のたびに C ABI との手合わせ (明示値の維持、`From` 実装の追従) が発生する (例: issue 0061)

C API を本体と同等の品質に仕上げられるまではスコープを Rust に絞り、unsafe と ABI 管理のコストを履歴から切り離す。

## 現状

- `crates/c-api/` は `src/lib.rs` (`pub unsafe extern "C" fn` 13 個と `#[unsafe(no_mangle)]` の安全関数 `srt_version` 1 個)、`build.rs`、`cbindgen.toml`、`include/srt.h`、`Cargo.toml` (`shiguredo_srt_c_api`、`publish = false`) で構成される
- ルート `Cargo.toml` の `[workspace] members` に `crates/c-api` が含まれる
- `.github/workflows/release.yml` の `upload-assets` ジョブが `cargo build --release --package shiguredo_srt_c_api` でビルドし、`target/release/libsrt.{a,so,dylib}` (Windows は `srt.dll` / `srt.lib`) と `crates/c-api/include/srt.h` を同梱したアーカイブを 8 プラットフォーム分アップロードする。`publish` ジョブは `needs: [github-release, upload-assets]` としてこのジョブに依存している
- 公開済みリリース `2026.1.0-canary.1` には C アーティファクト 8 件が添付されている (`2026.1.0-canary.0` も同様)
- 本体 `src/` は c-api を参照しない (依存は c-api から本体への一方向)
- `examples/`、`pbt/`、`fuzz/`、`tests/` は c-api に依存しない

## 設計方針

- `crates/c-api` をディレクトリごと削除し、ルート `Cargo.toml` の `members` から外す。`crates/` は空になるためディレクトリも削除する
- `.github/workflows/release.yml` から `upload-assets` ジョブを削除し、`publish` の `needs` を `[github-release]` に更新する。以後のリリースは GitHub Release の作成と `cargo publish` のみとする
- 過去リリースに添付済みの C アーティファクトは削除しない (canary の実物であり、再度必要になれば git 履歴から復元できる)
- `Cargo.lock` を再生成する (cbindgen とその依存関係が消える)
- C API 由来の open issue (issue 0071) および C ABI を前提とする issue (issue 0061) の扱いは本 issue の対応後に別途判断する。本 issue の変更で他の issue ファイルを動かさない

## 完了条件

- `crates/` ディレクトリが存在しないこと
- `cargo metadata --no-deps` の workspace members に `shiguredo_srt_c_api` が含まれないこと
- `.github/workflows/release.yml` に `shiguredo_srt_c_api` および `srt.h` への参照が残っていないこと (grep で 0 件)
- `cargo fmt --all -- --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` が通過すること
- `CHANGES.md` の `## develop` に `[CHANGE]` で C API の削除が記録されていること

## テスト

削除対象 crate には統合テストが存在せず (crate-type 制約)、削除後の検証はワークスペース全体のビルド・lint・テストと `cargo metadata` の members 確認で足りる。テストの新規追加は不要。

## CHANGES.md

後方互換のない削除 (配布物から C アーティファクトが消える) ため、`## develop` 直下の `[CHANGE]` 列に

```
- [CHANGE] crates/c-api (C API) を削除し、Rust ライブラリのみの公開に絞り込む
  - @voluntas
```

を追加する。機能に影響する削除なので `### misc` には置かない。

## 相互作用

- issue 0053 (c-api の `#![allow]` を `#![expect]` に置換) で入れた抑制は本削除で消える
- issue 0061 (`ConnectionState::Conclusion` 削除) は C ABI の明示値維持を設計方針の前提としており、本 issue 後に前提が消える。0040 が除外した `Conclusion` の扱いを単純化できるため、0061 は刷新が必要になる
- issue 0071 (C API の crypto_salt 受け口) は対象 crate の削除により対応不要となる
- issue 0052 / 0072 は「C API 経由の Caller がハンドシェイクに失敗する」旨を根拠に含めるが、削除後は該当記述が古くなる
- issue 0062 (lib.rs の re-export 削除) は `crates/c-api` の import 修正を対象に含めているため、削除後は対象から外れる
