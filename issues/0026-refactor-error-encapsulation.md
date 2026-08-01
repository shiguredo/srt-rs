# error.rs の Error をカプセル化し、Debug と Display を分離する

- Priority: Medium
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/change-error-encapsulation
- Polished: 2026-08-01

## 目的

`src/error.rs` の `Error` 構造体の全フィールド (`kind`, `reason`, `location`, `backtrace`) が `pub` で露出している。また `Debug` 実装が `Display` に委譲しており (`write!(f, "{self}")`)、RUST_BACKTRACE 設定時には `unwrap()` などのパニック出力 (`Debug` 経由) と `eprintln!` などの表示 (`Display` 経由) の両方にバックトレースが含まれる。パニック時は Rust ランタイム自身もバックトレースを出力するため、二重表示になる実害がある。バックトレースは RUST_BACKTRACE 環境変数が設定されていない場合には取得されない (error.rs の `Backtrace::capture` の挙動)。

## 優先度根拠

内部表現をカプセル化せず公開することで、将来の内部構造変更が破壊的変更になる。`Debug` と `Display` の区別がないことは Rust の慣習に反し、RUST_BACKTRACE 設定時にバックトレースが二重表示される実害がある。

## 現状

```rust
pub struct Error {
    pub kind: ErrorKind,
    pub reason: String,
    pub location: &'static Location<'static>,
    pub backtrace: Backtrace,
}
```

```rust
impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")  // Display に委譲
    }
}
```

`Display` 実装は `BacktraceStatus::Captured` のときバックトレースを出力する。`Error` と `ErrorKind` は lib.rs で re-export されており、`tests/test_error.rs` は `result.unwrap_err().kind` でフィールドに直接アクセスしている。

## 設計方針

1. `Error` のフィールド (`kind`, `reason`, `location`, `backtrace`) を非公開にし、アクセサ `kind(&self) -> ErrorKind` / `reason(&self) -> &str` / `location(&self) -> &'static Location<'static>` / `backtrace(&self) -> &Backtrace` を提供する
2. `Debug` 実装を構造的表現 (フィールド名付き) に変更し、バックトレースは含めない。`Display` 実装からもバックトレース出力を除外し、ユーザ向けのメッセージ (エラー種別・理由・位置) に絞る。`backtrace` フィールドはアクセサ経由で取得可能なまま保持する
3. フィールド非公開は公開 API の後方互換のない変更であるため、CHANGES.md に `[CHANGE]` エントリを追記する (closed/0019 の前例に従う。例: `[CHANGE] Error のフィールドを非公開化し、Debug / Display からバックトレース出力を除外する`)
4. 既存の `tests/test_error.rs` の `result.unwrap_err().kind` フィールドアクセス (リポジトリ内で唯一のフィールド直接アクセス) を `kind()` アクセサ経由に修正する
5. なお、`ErrorKind` の `#[non_exhaustive]` (shiguredo-rust 規約違反) は本 issue のスコープ外とし、別 issue で対応する

## 完了条件

- `Error` のフィールドが非公開になっていること
- `kind(&self) -> ErrorKind` / `reason(&self) -> &str` / `location(&self) -> &'static Location<'static>` / `backtrace(&self) -> &Backtrace` アクセサが提供されていること
- `Debug` 出力が構造的表現 (フィールド名付き) になっていることを検証するテストが追加されていること
- `Debug` / `Display` 出力にバックトレースが含まれないことを検証するテストが追加されていること (出力文字列にバックトレースの表現 (現状の `Display` 実装では `Backtrace:\n`) が含まれないことを確認する。RUST_BACKTRACE=1 の環境で実行する。CI (.github/workflows/ci.yml) のテストステップにも RUST_BACKTRACE=1 を設定すること。RUST_BACKTRACE 未設定の環境では `Backtrace::capture()` がバックトレースを取得せず、実装が誤っていてもテストが通るため。また、テスト冒頭で `backtrace()` アクセサの `status()` が `BacktraceStatus::Captured` であることを確認し、検証の前提が成立していることを担保すること)
- `cargo test` で全テストが通過すること
- `cargo clippy --workspace -- -D warnings` と `cargo build --workspace` が通ること
- CHANGES.md の `## develop` セクションに `[CHANGE]` エントリが追加されていること
