# items_after_test_module を解消するためシーケンス比較関数を mod tests の前へ移動する

- Priority: Low
- Created: 2026-05-14
- Completed: 2026-05-30
- Polished: 2026-05-30
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-move-sequence-funcs-before-tests

## 目的

`src/srt_packet.rs` で `#[cfg(test)] mod tests` ブロックの後ろに `pub(crate)` 関数 `sequence_less_than` と `sequence_greater_than` が定義されており、clippy の `items_after_test_module` 違反となっている。この 2 関数を `mod tests` の前へ移動して違反を解消する。

注: 本 issue は作成時点では cargo fmt 違反 5 箇所と clippy `manual_range_contains` も対象としていたが、いずれも対応済みのため削除した (fmt は commit d7220b5、`manual_range_contains` は commit 791b8c4)。残作業は `items_after_test_module` の 1 件のみである。

## 優先度根拠

機能には一切影響しない、関数の定義位置に関するリント違反である。さらに CI の clippy ゲート (`.github/workflows/ci.yml` の `cargo clippy --workspace -- -D warnings`) は `--all-targets` を付けないため、この違反を捕捉しない。`#[cfg(test)] mod tests` はテストターゲットでのみコンパイルされ、`items_after_test_module` は `--all-targets` 時にのみ発火するためである。実際、現状でも CI は緑であり、本違反は develop をブロックしていない。違反は開発者がローカルで `cargo clippy --all-targets` を実行したときにのみ表面化する。機能影響なし・CI 非ブロックのため Low とする。

## 現状

`src/srt_packet.rs` の構造 (行番号は将来変わりうるため構造で示す):

- `impl ControlPacket` の `encoded_size` を最後に `impl` ブロックが閉じる
- その直後に `#[cfg(test)] mod tests { ... }` (単体テスト)
- さらにその後ろに `pub(crate) fn sequence_less_than` と `pub(crate) fn sequence_greater_than` の 2 関数 (それぞれ doc コメント付き) が定義されている

`cargo clippy --workspace --all-targets -- -D warnings` を実行すると、この 2 関数に対して `items after a test module` 違反が発火する。

## 設計方針

`sequence_less_than` と `sequence_greater_than` の 2 関数を、**直前の doc コメントごと** `#[cfg(test)] mod tests` ブロックの**前** (直前の `impl` ブロックを閉じる `}` の直後) へ移動する。clippy の help (move the items to before the test module was defined) に従う。

- 移動対象はこの 2 関数のみ。`mod tests` 以降に他の項目 (関数・型・定数) は存在しない (移動後はファイル末尾が `mod tests` で終わる)
- 両関数のシグネチャ・可視性 (`pub(crate)`) は不変。`sequence_greater_than` が同一モジュール内で `sequence_less_than` を呼ぶが、Rust は同一モジュール内の定義順に依存しないため、移動順序による影響はない
- `#[allow]` / `#[expect]` による握り潰しは行わない。コード配置で正しく解消する

## 相互作用

- `sequence_less_than` / `sequence_greater_than` は `src/srt_sender.rs` と `src/srt_receiver.rs` から `use crate::srt_packet::...` 経由で参照されている。本変更は定義の物理位置をファイル内で移すだけでシグネチャ・可視性を変えないため、これらの呼び出し側には一切影響しない
- 同じく `sequence_less_than` を使う #0018 (`find_deliverable_seq` ラップアラウンド, Priority High) とは、関数定義の移動と呼び出しのため競合しない。番号順で #0018 を先に対応してもよいし、本 issue を先に対応してもよい

## テスト

関数の定義位置をファイル内で移動するだけで挙動は不変のため、新規テストは追加しない。`sequence_less_than` の挙動は既存の単体テスト (`src/srt_sender.rs` の `test_sequence_less_than`) と PBT で担保される。

## CHANGES.md

CHANGES.md の `### misc` には既に `[CHANGE] sequence_less_than / sequence_greater_than を srt_packet.rs に集約する` のエントリがある。本変更はその集約作業の配置調整 (test module の前へ移す) の範囲内であり、純粋なリント対応で機能に影響しないため、新規エントリは追加しない。

## 後方互換

公開 API・シグネチャ・可視性 (`pub(crate)`) すべて不変。`pub(crate)` のためクレート外には元々非公開であり、後方互換上の懸念はない。

## 完了条件

- `src/srt_packet.rs` の `sequence_less_than` / `sequence_greater_than` が `#[cfg(test)] mod tests` の前に定義されていること
- `cargo clippy --workspace --all-targets -- -D warnings` が通過すること (`items_after_test_module` が解消されていること)
- `cargo test` で全テストが通過すること

注: 完了条件の clippy コマンドは `--all-targets` を付ける。CI ゲートはこれを付けず本違反を検証しない (理由は優先度根拠を参照) ため、修正後の確認はローカルで行うこと。CI ゲート自体への `--all-targets` 追加は本 issue のスコープ外とし、必要なら別 issue で扱う。

## 解決方法

`src/srt_packet.rs` の `pub(crate) fn sequence_less_than` と `pub(crate) fn sequence_greater_than` の 2 関数を、直前の doc コメントごと `#[cfg(test)] mod tests` ブロックの**前** (`impl ControlPacket` を閉じる `}` の直後) へ移動した。設計方針どおり純粋な移動であり、関数本体・シグネチャ・可視性 (`pub(crate)`)・doc コメントはバイト単位で不変。移動後はファイル末尾が `mod tests` で終わり、test module の後ろに項目は残っていない。

- 変更ファイル: `src/srt_packet.rs` のみ (11 行の移動)
- `#[allow]` / `#[expect]` による握り潰しは行わず、コード配置で `items_after_test_module` を解消した
- 呼び出し元 (`src/srt_receiver.rs`, `src/srt_sender.rs`) は `use crate::srt_packet::...` でパス解決しておりファイル内位置に依存しないため影響なし
- 新規テストは追加しない (挙動不変のため)。`sequence_less_than` の挙動は既存の単体テスト (`src/srt_sender.rs` の `test_sequence_less_than`) で担保される

確認:

- `cargo clippy --workspace --all-targets -- -D warnings` 通過 (`items_after_test_module` 解消)
- `cargo test --workspace` 全通過 (267 件)
- `cargo fmt --all -- --check` 差分なし
