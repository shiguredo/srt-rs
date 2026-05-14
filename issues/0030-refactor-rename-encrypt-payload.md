# encrypt_payload 関数名が誤解を招く

- Priority: Low
- Created: 2026-05-14
- Model: DeepSeek V4 Pro
- Branch: feature/refactor-rename-encrypt-payload

## 目的

`src/crypto.rs:391` の `encrypt_payload` 関数名が「暗号化」を示唆するが、実際には復号化にも同一関数を使用している。AES-CTR の対称性を理由に同一関数で済ませているが、誤解を招く。

## 現状

```rust
fn encrypt_payload(sek, salt, packet_index, payload, key_length) { ... }
```

```rust
pub fn decrypt(&self, ...) -> Result<(), Error> {
    encrypt_payload(sek, &self.salt, packet_index, payload, self.key_length)
}
```

## 設計方針

`apply_aes_ctr` など、操作内容を正確に表す名前に変更する。

## 完了条件

- 関数名が操作内容を正確に反映していること
- `cargo test` で全テストが通過すること
