//! 版本与特性适配层。
//!
//! 原版 Robo3T 1.4.4 受限于旧版 C++ 驱动，仅支持到 MongoDB 4.2，且认证机制
//! 写死为 SCRAM-SHA-1 / SCRAM-SHA-256 / MONGODB-CR。本模块以「能力矩阵」的
//! 方式描述每个服务器版本支持的：认证机制、wire protocol、命令/聚合阶段/函数，
//! 并提供根据版本自动选择认证机制、特性门控的 API。

use std::cmp::Ordering;
use std::fmt;

use serde::de;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Rubo3T 显式支持的最小 / 最大版本。
pub const MIN_SUPPORTED_VERSION: ServerVersion = ServerVersion {
    major: 3,
    minor: 0,
    patch: 0,
};
pub const MAX_SUPPORTED_VERSION: ServerVersion = ServerVersion {
    major: 8,
    minor: 0,
    patch: 0,
};

/// 服务器版本。支持 `Ord`，可直接用于 `>=` 比较。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ServerVersion {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

impl ServerVersion {
    pub const fn new(major: u16, minor: u16, patch: u16) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// 解析形如 `4.2.0`、`4.2`、`4.2.0-rc3`、`4.4.1-ent` 的版本字符串。
    /// 取前三个数字段；忽略后缀（rc / ent / 等）。
    pub fn parse(s: &str) -> Option<ServerVersion> {
        let core = s.split(['-', '+', '_']).next()?;
        let mut parts = core.split('.');
        let major = parts.next()?.parse::<u16>().ok()?;
        let minor = parts
            .next()
            .and_then(|x| x.parse::<u16>().ok())
            .unwrap_or(0);
        let patch = parts
            .next()
            .and_then(|x| x.parse::<u16>().ok())
            .unwrap_or(0);
        Some(ServerVersion::new(major, minor, patch))
    }
}

impl PartialOrd for ServerVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ServerVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(other.major, other.minor, other.patch))
    }
}

impl fmt::Display for ServerVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// MongoDB 认证机制。
///
/// 覆盖原版没有的现代机制：SCRAM-SHA-256（4.0+ 默认）、
/// PLAIN（LDAP/AD，Atlas/RDS）、X.509、GSSAPI（Kerberos）、
/// MONGODB-AWS（Atlas / AWS DocumentDB / RDS）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthMechanism {
    /// 旧版挑战-响应（MongoDB 4.0 起已移除）。
    MongoDbCr,
    /// SCRAM-SHA-1（3.0+，4.0 前的默认）。
    ScramSha1,
    /// SCRAM-SHA-256（4.0+ 默认，更现代）。
    ScramSha256,
    /// PLAIN（LDAP / Active Directory，使用 $external）。
    Plain,
    /// X.509 证书认证（使用 $external，无需密码）。
    X509,
    /// GSSAPI / Kerberos。
    GssApi,
    /// MONGODB-AWS（IAM，Atlas / DocumentDB / RDS）。
    AwsIam,
}

impl Serialize for AuthMechanism {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.uri_value())
    }
}

impl<'de> Deserialize<'de> for AuthMechanism {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        match s.as_str() {
            "MONGODB-CR" => Ok(AuthMechanism::MongoDbCr),
            "SCRAM-SHA-1" => Ok(AuthMechanism::ScramSha1),
            "SCRAM-SHA-256" => Ok(AuthMechanism::ScramSha256),
            "PLAIN" => Ok(AuthMechanism::Plain),
            "MONGODB-X509" => Ok(AuthMechanism::X509),
            "GSSAPI" => Ok(AuthMechanism::GssApi),
            "MONGODB-AWS" => Ok(AuthMechanism::AwsIam),
            other => Err(de::Error::custom(format!("未知认证机制: {other}"))),
        }
    }
}

impl AuthMechanism {
    /// 连接 URI 中使用的 `authMechanism` 值。
    pub fn uri_value(self) -> &'static str {
        match self {
            AuthMechanism::MongoDbCr => "MONGODB-CR",
            AuthMechanism::ScramSha1 => "SCRAM-SHA-1",
            AuthMechanism::ScramSha256 => "SCRAM-SHA-256",
            AuthMechanism::Plain => "PLAIN",
            AuthMechanism::X509 => "MONGODB-X509",
            AuthMechanism::GssApi => "GSSAPI",
            AuthMechanism::AwsIam => "MONGODB-AWS",
        }
    }

    /// 该机制是否把认证库固定为 `$external`。
    pub fn uses_external_source(self) -> bool {
        matches!(
            self,
            AuthMechanism::Plain | AuthMechanism::X509 | AuthMechanism::GssApi
        )
    }

    /// 该机制是否不需要密码（X.509）。
    pub fn is_passwordless(self) -> bool {
        matches!(self, AuthMechanism::X509)
    }
}

impl fmt::Display for AuthMechanism {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.uri_value())
    }
}

/// 根据服务器版本 + 用户显式选择，解析出最终使用的认证机制。
///
/// 规则：
/// - 用户显式指定 → 直接采用（除非与版本冲突，调用方仍应提示）。
/// - 未指定：
///   - v >= 4.0 → SCRAM-SHA-256（首选），兼容 SCRAM-SHA-1
///   - 3.0 <= v < 4.0 → SCRAM-SHA-1（MONGODB-CR 在 3.0 已废弃，4.0 移除）
///   - v < 3.0 → MONGODB-CR
pub fn resolve_auth_mechanism(
    version: ServerVersion,
    explicit: Option<AuthMechanism>,
) -> AuthMechanism {
    if let Some(m) = explicit {
        return m;
    }
    if version >= ServerVersion::new(4, 0, 0) {
        AuthMechanism::ScramSha256
    } else if version >= ServerVersion::new(3, 0, 0) {
        AuthMechanism::ScramSha1
    } else {
        AuthMechanism::MongoDbCr
    }
}

/// 特性门控：给定服务器版本，回答「是否支持某能力」。
///
/// 这是对「原版硬编码旧协议」的抽象——所有版本差异都集中在此，
/// 上层 UI / 命令层只问 `FeatureGate`，不关心具体版本号。
#[derive(Debug, Clone, Copy)]
pub struct FeatureGate {
    version: ServerVersion,
}

impl FeatureGate {
    pub fn new(version: ServerVersion) -> Self {
        Self { version }
    }

    pub fn version(&self) -> ServerVersion {
        self.version
    }

    /// 该版本使用的默认认证机制（未显式指定时）。
    pub fn default_auth_mechanism(&self) -> AuthMechanism {
        resolve_auth_mechanism(self.version, None)
    }

    /// 是否支持 SCRAM-SHA-256（4.0+）。
    pub fn supports_scram_sha256(&self) -> bool {
        self.version >= ServerVersion::new(4, 0, 0)
    }

    /// 是否仍支持已废弃的 MONGODB-CR（< 4.0）。
    pub fn supports_mongodb_cr(&self) -> bool {
        self.version < ServerVersion::new(4, 0, 0)
    }

    /// 是否支持会话（3.6+，驱动用 OP_MSG）。
    pub fn supports_sessions(&self) -> bool {
        self.version >= ServerVersion::new(3, 6, 0)
    }

    /// 是否支持可重试写入 / 读关注（3.6+）。
    pub fn supports_retryable_writes(&self) -> bool {
        self.version >= ServerVersion::new(3, 6, 0)
    }

    /// 是否支持 change streams（3.6+）。
    pub fn supports_change_streams(&self) -> bool {
        self.version >= ServerVersion::new(3, 6, 0)
    }

    /// 是否支持 zstd 压缩（4.2+）。
    pub fn supports_zstd(&self) -> bool {
        self.version >= ServerVersion::new(4, 2, 0)
    }

    /// 聚合阶段 $merge（4.2+）。
    pub fn supports_merge(&self) -> bool {
        self.version >= ServerVersion::new(4, 2, 0)
    }

    /// 聚合阶段 $unionWith（4.4+）。
    pub fn supports_union_with(&self) -> bool {
        self.version >= ServerVersion::new(4, 4, 0)
    }

    /// 聚合阶段 $lookup 子查询（3.6+）。
    pub fn supports_lookup_subquery(&self) -> bool {
        self.version >= ServerVersion::new(3, 6, 0)
    }

    /// 聚合阶段 $graphLookup 的 connectToNonMongos（4.2+）。
    pub fn supports_graph_lookup_non_mongos(&self) -> bool {
        self.version >= ServerVersion::new(4, 2, 0)
    }

    /// 可查询加密（Queryable Encryption，7.0+）。
    pub fn supports_queryable_encryption(&self) -> bool {
        self.version >= ServerVersion::new(7, 0, 0)
    }

    /// OIDC 认证（7.0+）。
    pub fn supports_oidc(&self) -> bool {
        self.version >= ServerVersion::new(7, 0, 0)
    }

    /// 时间序列集合（5.0+）。
    pub fn supports_timeseries(&self) -> bool {
        self.version >= ServerVersion::new(5, 0, 0)
    }

    /// 集群 / 分片相关的特性（始终支持，单节点也可作为副本集成员）。
    pub fn supports_replica_set(&self) -> bool {
        self.version >= ServerVersion::new(3, 0, 0)
    }

    /// 返回面向 UI 的能力摘要。
    pub fn summary(&self) -> Vec<VersionCapability> {
        let mut out = Vec::new();
        out.push(VersionCapability::new(
            "SCRAM-SHA-256",
            self.supports_scram_sha256(),
            "4.0+",
        ));
        out.push(VersionCapability::new(
            "SCRAM-SHA-1",
            self.version >= ServerVersion::new(3, 0, 0),
            "3.0+",
        ));
        out.push(VersionCapability::new(
            "MONGODB-CR",
            self.supports_mongodb_cr(),
            "< 4.0",
        ));
        out.push(VersionCapability::new(
            "会话 / OP_MSG",
            self.supports_sessions(),
            "3.6+",
        ));
        out.push(VersionCapability::new(
            "Change Streams",
            self.supports_change_streams(),
            "3.6+",
        ));
        out.push(VersionCapability::new(
            "zstd 压缩",
            self.supports_zstd(),
            "4.2+",
        ));
        out.push(VersionCapability::new("$merge", self.supports_merge(), "4.2+"));
        out.push(VersionCapability::new(
            "$unionWith",
            self.supports_union_with(),
            "4.4+",
        ));
        out.push(VersionCapability::new(
            "时间序列",
            self.supports_timeseries(),
            "5.0+",
        ));
        out.push(VersionCapability::new(
            "OIDC 认证",
            self.supports_oidc(),
            "7.0+",
        ));
        out.push(VersionCapability::new(
            "可查询加密",
            self.supports_queryable_encryption(),
            "7.0+",
        ));
        out
    }
}

/// 单个能力条目（用于 UI 展示版本差异）。
#[derive(Debug, Clone)]
pub struct VersionCapability {
    pub name: &'static str,
    pub supported: bool,
    pub since: &'static str,
}

impl VersionCapability {
    pub const fn new(name: &'static str, supported: bool, since: &'static str) -> Self {
        Self {
            name,
            supported,
            since,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_versions() {
        assert_eq!(
            ServerVersion::parse("4.2.0"),
            Some(ServerVersion::new(4, 2, 0))
        );
        assert_eq!(
            ServerVersion::parse("4.2"),
            Some(ServerVersion::new(4, 2, 0))
        );
        assert_eq!(
            ServerVersion::parse("4.2.0-rc3"),
            Some(ServerVersion::new(4, 2, 0))
        );
        assert_eq!(
            ServerVersion::parse("4.4.1-ent"),
            Some(ServerVersion::new(4, 4, 1))
        );
        assert_eq!(
            ServerVersion::parse("7.0.0"),
            Some(ServerVersion::new(7, 0, 0))
        );
    }

    #[test]
    fn version_ordering() {
        assert!(ServerVersion::new(4, 2, 0) < ServerVersion::new(4, 4, 0));
        assert!(ServerVersion::new(3, 6, 0) < ServerVersion::new(4, 0, 0));
        assert!(ServerVersion::new(8, 0, 0) >= ServerVersion::new(7, 0, 0));
    }

    #[test]
    fn default_mechanism_by_version() {
        assert_eq!(
            resolve_auth_mechanism(ServerVersion::new(7, 0, 0), None),
            AuthMechanism::ScramSha256
        );
        assert_eq!(
            resolve_auth_mechanism(ServerVersion::new(4, 0, 0), None),
            AuthMechanism::ScramSha256
        );
        assert_eq!(
            resolve_auth_mechanism(ServerVersion::new(3, 6, 0), None),
            AuthMechanism::ScramSha1
        );
        assert_eq!(
            resolve_auth_mechanism(ServerVersion::new(2, 6, 0), None),
            AuthMechanism::MongoDbCr
        );
    }

    #[test]
    fn explicit_mechanism_wins() {
        assert_eq!(
            resolve_auth_mechanism(
                ServerVersion::new(7, 0, 0),
                Some(AuthMechanism::AwsIam)
            ),
            AuthMechanism::AwsIam
        );
    }

    #[test]
    fn feature_gate_matrix() {
        let v42 = FeatureGate::new(ServerVersion::new(4, 2, 0));
        assert!(v42.supports_scram_sha256());
        assert!(v42.supports_merge());
        assert!(!v42.supports_union_with());
        assert!(!v42.supports_timeseries());

        let v44 = FeatureGate::new(ServerVersion::new(4, 4, 0));
        assert!(v44.supports_union_with());

        let v70 = FeatureGate::new(ServerVersion::new(7, 0, 0));
        assert!(v70.supports_oidc());
        assert!(v70.supports_queryable_encryption());

        let v30 = FeatureGate::new(ServerVersion::new(3, 0, 0));
        assert!(v30.supports_mongodb_cr());
        assert!(!v30.supports_scram_sha256());
        assert!(!v30.supports_change_streams());
    }
}
