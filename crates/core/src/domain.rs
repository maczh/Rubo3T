//! 领域模型：服务器、数据库、集合、索引、用户、文档。
//!
//! 这些是 UI 与数据访问层之间传递的纯数据结构，不依赖具体驱动。

use serde::{Deserialize, Serialize};

use crate::version::ServerVersion;

/// 集合类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollectionKind {
    Collection,
    View,
    Timeseries,
    Unknown,
}

impl CollectionKind {
    pub fn label(self) -> &'static str {
        match self {
            CollectionKind::Collection => "集合",
            CollectionKind::View => "视图",
            CollectionKind::Timeseries => "时序",
            CollectionKind::Unknown => "其他",
        }
    }
}

/// 数据库信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseInfo {
    pub name: String,
    pub size_on_disk: u64,
}

/// 集合 / 视图信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionInfo {
    pub name: String,
    pub kind: CollectionKind,
    pub size_bytes: u64,
    pub count: u64,
}

/// 索引信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexInfo {
    pub name: String,
    pub keys: serde_json::Value,
    pub unique: bool,
    pub background: bool,
    pub sparse: bool,
}

/// 用户信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub name: String,
    pub db: String,
    pub roles: Vec<String>,
}

/// 连接成功后的服务器概要信息。
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub version: ServerVersion,
    pub edition: String,
    pub host: String,
    /// 是否为副本集 / 分片集群（多节点拓扑）。
    pub is_cluster: bool,
    /// 检测到的能力（UI 展示用）。
    pub feature_summary: Vec<crate::version::VersionCapability>,
}
