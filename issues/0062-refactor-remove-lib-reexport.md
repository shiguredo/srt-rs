# lib.rs の re-export が shiguredo-rust 規約に違反する

- Created: 2026-08-16
- Branch: feature/refactor-remove-lib-reexport
- Polished: 2026-08-16

## 目的

`src/lib.rs` で 12 行にわたってすべての公開型を `pub use` で re-export している。shiguredo-rust 規約は「re-export は基本的にやらないこと。どうしても必要な場合は許可を得ること」と定めており、本実装は規約に違反する。

## 現状

```rust
pub use buf::{read_bytes, read_u8, ...};
pub use crypto::{CryptoContext, KeyFlag, ...};
pub use error::{Error, ErrorKind};
pub use srt_connection::{ConnectionEvent, ...};
pub use srt_handshake::{DEFAULT_FLOW_WINDOW, ...};
pub use srt_packet::{ControlPacket, ...};
pub use srt_receiver::{AckPacket, ...};
pub use srt_sender::{SenderBuffer, SenderStats};
pub use time::Timestamp;
```

## 設計方針

すべての `pub use` を削除し、利用側で `use shiguredo_srt::error::Error` のように元のモジュールから直接 import する形式に変更する。`examples/`、`crates/c-api/`、`pbt/`、`tests/` の全 import も併せて修正する。

## 完了条件

- `lib.rs` からすべての `pub use` が削除されていること
- `examples/`、`crates/c-api/`、`pbt/`、`tests/` の全 import が修正されていること
- `CHANGES.md` の `## develop` セクションの `misc` に `[UPDATE]` エントリが追加されていること
- `cargo test --workspace` で全テストが通過すること

## 解決方法

`src/lib.rs` からすべての `pub use` 行を削除する。`examples/`、`crates/c-api/`、`pbt/`、`tests/` の全 import を、`use shiguredo_srt::CryptoContext` から `use shiguredo_srt::crypto::CryptoContext` のように元のモジュールから直接 import する形式に修正する。
