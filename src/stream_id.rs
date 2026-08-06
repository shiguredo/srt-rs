//! SRT Stream ID と Access Control
//!
//! Stream ID は SRT ハンドシェイク時に Caller から Listener に送信される識別子。
//! Access Control 構文を使用することで、認証やリソース指定などの情報を構造化して送信できる。
//!
//! ## Access Control 構文
//!
//! ```text
//! #!::key1=value1,key2=value2,...
//! ```
//!
//! ## 標準キー
//!
//! - `u`: User Name (認証名)
//! - `r`: Resource Name (リソース名)
//! - `h`: Host Name (ホスト名)
//! - `s`: Session ID (セッション ID)
//! - `t`: Type (stream, file, auth)
//! - `m`: Mode (request, publish, bidirectional)
//!
//! ## 例
//!
//! ```
//! use shiguredo_srt::stream_id::{AccessControl, StreamMode, StreamType};
//!
//! let ac = AccessControl::parse("#!::u=admin,r=live/stream1").unwrap();
//! assert_eq!(ac.user_name(), Some("admin"));
//! assert_eq!(ac.resource_name(), Some("live/stream1"));
//! ```

use std::collections::HashMap;

/// Access Control 構文のプレフィックス
const ACCESS_CONTROL_PREFIX: &str = "#!::";

/// ストリームタイプ
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamType {
    /// ストリーム (デフォルト)
    #[default]
    Stream,
    /// ファイル転送
    File,
    /// 認証
    Auth,
}

impl StreamType {
    /// 文字列から変換
    #[expect(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "stream" => Some(Self::Stream),
            "file" => Some(Self::File),
            "auth" => Some(Self::Auth),
            _ => None,
        }
    }

    /// 文字列に変換
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stream => "stream",
            Self::File => "file",
            Self::Auth => "auth",
        }
    }
}

/// ストリームモード
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamMode {
    /// リクエスト (デフォルト): Caller がデータを受信
    #[default]
    Request,
    /// パブリッシュ: Caller がデータを送信
    Publish,
    /// 双方向
    Bidirectional,
}

impl StreamMode {
    /// 文字列から変換
    #[expect(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "request" => Some(Self::Request),
            "publish" => Some(Self::Publish),
            "bidirectional" => Some(Self::Bidirectional),
            _ => None,
        }
    }

    /// 文字列に変換
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Publish => "publish",
            Self::Bidirectional => "bidirectional",
        }
    }
}

/// Access Control パース結果
#[derive(Debug, Clone, Default)]
pub struct AccessControl {
    /// ユーザー名 (u)
    user_name: Option<String>,
    /// リソース名 (r)
    resource_name: Option<String>,
    /// ホスト名 (h)
    host_name: Option<String>,
    /// セッション ID (s)
    session_id: Option<String>,
    /// タイプ (t)
    stream_type: StreamType,
    /// モード (m)
    stream_mode: StreamMode,
    /// カスタムキー
    custom: HashMap<String, String>,
}

impl AccessControl {
    /// Access Control 構文をパース
    ///
    /// `#!::` プレフィックスがない場合や、フリーフォームの Stream ID の場合は None を返す。
    pub fn parse(stream_id: &str) -> Option<Self> {
        // プレフィックスをチェック
        let content = stream_id.strip_prefix(ACCESS_CONTROL_PREFIX)?;

        let mut ac = AccessControl::default();

        // カンマ区切りのキー・バリューペアをパース
        for pair in content.split(',') {
            if let Some((key, value)) = pair.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match key {
                    "u" => ac.user_name = Some(value.to_string()),
                    "r" => ac.resource_name = Some(value.to_string()),
                    "h" => ac.host_name = Some(value.to_string()),
                    "s" => ac.session_id = Some(value.to_string()),
                    "t" => {
                        if let Some(t) = StreamType::from_str(value) {
                            ac.stream_type = t;
                        }
                    }
                    "m" => {
                        if let Some(m) = StreamMode::from_str(value) {
                            ac.stream_mode = m;
                        }
                    }
                    _ => {
                        // カスタムキー
                        ac.custom.insert(key.to_string(), value.to_string());
                    }
                }
            }
        }

        Some(ac)
    }

    /// Access Control 構文を生成
    pub fn encode(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref u) = self.user_name {
            parts.push(format!("u={u}"));
        }
        if let Some(ref r) = self.resource_name {
            parts.push(format!("r={r}"));
        }
        if let Some(ref h) = self.host_name {
            parts.push(format!("h={h}"));
        }
        if let Some(ref s) = self.session_id {
            parts.push(format!("s={s}"));
        }
        if self.stream_type != StreamType::Stream {
            parts.push(format!("t={}", self.stream_type.as_str()));
        }
        if self.stream_mode != StreamMode::Request {
            parts.push(format!("m={}", self.stream_mode.as_str()));
        }
        for (key, value) in &self.custom {
            parts.push(format!("{key}={value}"));
        }

        format!("{ACCESS_CONTROL_PREFIX}{}", parts.join(","))
    }

    /// ユーザー名を取得
    pub fn user_name(&self) -> Option<&str> {
        self.user_name.as_deref()
    }

    /// リソース名を取得
    pub fn resource_name(&self) -> Option<&str> {
        self.resource_name.as_deref()
    }

    /// ホスト名を取得
    pub fn host_name(&self) -> Option<&str> {
        self.host_name.as_deref()
    }

    /// セッション ID を取得
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// ストリームタイプを取得
    pub fn stream_type(&self) -> StreamType {
        self.stream_type
    }

    /// ストリームモードを取得
    pub fn stream_mode(&self) -> StreamMode {
        self.stream_mode
    }

    /// カスタムキーを取得
    pub fn custom(&self, key: &str) -> Option<&str> {
        self.custom.get(key).map(|s| s.as_str())
    }
}

/// Access Control ビルダー
#[derive(Debug, Clone, Default)]
pub struct AccessControlBuilder {
    ac: AccessControl,
}

impl AccessControlBuilder {
    /// 新しいビルダーを作成
    pub fn new() -> Self {
        Self::default()
    }

    /// ユーザー名を設定
    pub fn user_name(mut self, name: impl Into<String>) -> Self {
        self.ac.user_name = Some(name.into());
        self
    }

    /// リソース名を設定
    pub fn resource_name(mut self, name: impl Into<String>) -> Self {
        self.ac.resource_name = Some(name.into());
        self
    }

    /// ホスト名を設定
    pub fn host_name(mut self, name: impl Into<String>) -> Self {
        self.ac.host_name = Some(name.into());
        self
    }

    /// セッション ID を設定
    pub fn session_id(mut self, id: impl Into<String>) -> Self {
        self.ac.session_id = Some(id.into());
        self
    }

    /// ストリームタイプを設定
    pub fn stream_type(mut self, t: StreamType) -> Self {
        self.ac.stream_type = t;
        self
    }

    /// ストリームモードを設定
    pub fn stream_mode(mut self, m: StreamMode) -> Self {
        self.ac.stream_mode = m;
        self
    }

    /// カスタムキーを設定
    pub fn custom(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.ac.custom.insert(key.into(), value.into());
        self
    }

    /// AccessControl を構築
    pub fn build(self) -> AccessControl {
        self.ac
    }

    /// Stream ID 文字列を構築
    pub fn build_stream_id(self) -> String {
        self.ac.encode()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let ac = AccessControl::parse("#!::u=admin,r=bluesbrothers1_hi")
            .expect("有効なストリーム ID のパースは成功する想定");
        assert_eq!(ac.user_name(), Some("admin"));
        assert_eq!(ac.resource_name(), Some("bluesbrothers1_hi"));
        assert_eq!(ac.stream_type(), StreamType::Stream);
        assert_eq!(ac.stream_mode(), StreamMode::Request);
    }

    #[test]
    fn test_parse_with_type_and_mode() {
        let ac = AccessControl::parse("#!::u=johnny,t=file,m=publish,r=results.csv")
            .expect("有効なストリーム ID のパースは成功する想定");
        assert_eq!(ac.user_name(), Some("johnny"));
        assert_eq!(ac.resource_name(), Some("results.csv"));
        assert_eq!(ac.stream_type(), StreamType::File);
        assert_eq!(ac.stream_mode(), StreamMode::Publish);
    }

    #[test]
    fn test_parse_with_host() {
        let ac = AccessControl::parse("#!::h=example.com,r=live/stream1")
            .expect("有効なストリーム ID のパースは成功する想定");
        assert_eq!(ac.host_name(), Some("example.com"));
        assert_eq!(ac.resource_name(), Some("live/stream1"));
    }

    #[test]
    fn test_parse_with_session() {
        let ac = AccessControl::parse("#!::s=abc123,r=temp")
            .expect("有効なストリーム ID のパースは成功する想定");
        assert_eq!(ac.session_id(), Some("abc123"));
        assert_eq!(ac.resource_name(), Some("temp"));
    }

    #[test]
    fn test_parse_with_custom() {
        let ac = AccessControl::parse("#!::u=test,myapp_key=value123")
            .expect("有効なストリーム ID のパースは成功する想定");
        assert_eq!(ac.user_name(), Some("test"));
        assert_eq!(ac.custom("myapp_key"), Some("value123"));
    }

    #[test]
    fn test_parse_no_prefix() {
        let ac = AccessControl::parse("just_a_stream_name");
        assert!(ac.is_none());
    }

    #[test]
    fn test_parse_wrong_prefix() {
        let ac = AccessControl::parse("#!:u=test");
        assert!(ac.is_none());
    }

    #[test]
    fn test_encode_basic() {
        let stream_id = AccessControlBuilder::new()
            .user_name("admin")
            .resource_name("live/stream1")
            .build_stream_id();
        assert!(stream_id.starts_with("#!::"));
        assert!(stream_id.contains("u=admin"));
        assert!(stream_id.contains("r=live/stream1"));
    }

    #[test]
    fn test_encode_with_mode() {
        let stream_id = AccessControlBuilder::new()
            .user_name("publisher")
            .resource_name("channel1")
            .stream_mode(StreamMode::Publish)
            .build_stream_id();
        assert!(stream_id.contains("m=publish"));
    }

    #[test]
    fn test_roundtrip() {
        let original = AccessControlBuilder::new()
            .user_name("test_user")
            .resource_name("my/resource")
            .host_name("example.com")
            .stream_type(StreamType::File)
            .stream_mode(StreamMode::Publish)
            .build();

        let encoded = original.encode();
        let parsed = AccessControl::parse(&encoded)
            .expect("エンコード済みストリーム ID のパースは成功する想定");

        assert_eq!(parsed.user_name(), original.user_name());
        assert_eq!(parsed.resource_name(), original.resource_name());
        assert_eq!(parsed.host_name(), original.host_name());
        assert_eq!(parsed.stream_type(), original.stream_type());
        assert_eq!(parsed.stream_mode(), original.stream_mode());
    }
}
