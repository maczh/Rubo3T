use std::fmt;

/// Rubo3T 统一错误类型。
#[derive(Debug)]
pub enum RuboError {
    /// MongoDB 驱动 / 服务端返回的错误。
    Mongo(String),
    /// 连接失败（握手、鉴权、网络）。
    Connection(String),
    /// 连接 URI 构造或参数错误。
    Config(String),
    /// 文档 JSON 解析失败（扩展 JSON 不合法等）。
    Parse(String),
    /// BSON 编解码错误。
    Bson(String),
    /// SSH 隧道错误。
    Ssh(String),
    /// 未连接。
    NotConnected,
    /// 通用错误。
    Other(String),
}

impl fmt::Display for RuboError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuboError::Mongo(s) => write!(f, "MongoDB 错误: {s}"),
            RuboError::Connection(s) => write!(f, "连接失败: {s}"),
            RuboError::Config(s) => write!(f, "配置错误: {s}"),
            RuboError::Parse(s) => write!(f, "解析错误: {s}"),
            RuboError::Bson(s) => write!(f, "BSON 错误: {s}"),
            RuboError::Ssh(s) => write!(f, "SSH 错误: {s}"),
            RuboError::NotConnected => write!(f, "尚未连接到任何服务器"),
            RuboError::Other(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for RuboError {}

// 注：`From<mongodb::error::Error>` 不放在 core 中（core 不依赖 mongodb），
// 改由 rubo3t-mongo crate 通过 `.map_err(|e| RuboError::Mongo(e.to_string()))` 手动转换。

impl From<bson::ser::Error> for RuboError {
    fn from(e: bson::ser::Error) -> Self {
        RuboError::Bson(e.to_string())
    }
}

impl From<bson::de::Error> for RuboError {
    fn from(e: bson::de::Error) -> Self {
        RuboError::Bson(e.to_string())
    }
}

impl From<serde_json::Error> for RuboError {
    fn from(e: serde_json::Error) -> Self {
        RuboError::Parse(e.to_string())
    }
}

impl From<std::io::Error> for RuboError {
    fn from(e: std::io::Error) -> Self {
        RuboError::Other(e.to_string())
    }
}

/// 统一 Result 别名。
pub type RuboResult<T> = std::result::Result<T, RuboError>;
