//! `rubo3t-core` — 与 MongoDB 版本无关的纯逻辑层。
//!
//! 本 crate 不依赖任何 GUI 或网络驱动，专注于：
//! - 领域模型（服务器 / 库 / 集合 / 文档 / 用户 / 索引）
//! - 连接与认证设置，以及把设置编译成 MongoDB 连接 URI
//! - **版本 / 特性适配层**：根据服务器版本决定支持的认证机制、wire protocol、命令与函数
//! - BSON <-> JSON（扩展 JSON）互转，兼容各版本文档表示
//! - 统一错误类型
//!
//! 这是 Rubo3T 重写中真正解决「原版只支持到 MongoDB 4.2」问题的核心：
//! 原版 Robo3T 1.4.4 使用旧版 C++ mongo 驱动 (`mongo::DBClientBase`)，
//! 该驱动硬编码了旧 wire protocol 与有限的认证机制。这里我们用一套
//! 明确的版本能力矩阵 + 官方 Rust 驱动（3.6–8.x）来覆盖 3.x 到最新版、
//! 副本集 / 分片集群 / Atlas(SRV) / RDS。

pub mod bson_ext;
pub mod domain;
pub mod error;
pub mod settings;
pub mod version;

/// 暴露 `bson` crate，便于上层用 `doc!` 等宏构造文档。
pub use bson;
/// 与 `mongodb` 驱动共用的 BSON 文档类型（两处为同一类型）。
pub use bson::Document;

pub use error::{RuboError, RuboResult};
pub use settings::{
    AuthSource, ConnectionMode, ConnectionSettings, CredentialSettings, Host, SshAuthMethod,
    SshSettings, SslSettings,
};
pub use version::{
    AuthMechanism, FeatureGate, ServerVersion, VersionCapability, MAX_SUPPORTED_VERSION,
    MIN_SUPPORTED_VERSION,
};
