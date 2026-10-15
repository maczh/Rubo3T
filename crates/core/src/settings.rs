//! 连接设置与 URI 构造。
//!
//! 原版 Robo3T 的连接设置（`ConnectionSettings` / `CredentialSettings` /
//! `SshSettings` / `SslSettings`）只覆盖到 MongoDB 4.2 的认证机制。这里扩展为
//! 完整覆盖 3.x–8.x 的所有认证方式、副本集 / 分片集群 / Atlas(SRV)、TLS、SSH 隧道。

use serde::{Deserialize, Serialize};

use crate::error::RuboResult;
use crate::version::{AuthMechanism, ServerVersion};

/// 连接拓扑类型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ConnectionMode {
    /// 单节点（standalone）。
    #[default]
    Standalone,
    /// 副本集（提供多个种子节点 + replicaSet 名称）。
    ReplicaSet,
    /// 分片集群（多个 mongos 入口）。
    ShardCluster,
}

impl ConnectionMode {
    pub fn label(self) -> &'static str {
        match self {
            ConnectionMode::Standalone => "单节点",
            ConnectionMode::ReplicaSet => "副本集",
            ConnectionMode::ShardCluster => "分片集群",
        }
    }
}

/// 单个主机:端口。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Host {
    pub host: String,
    pub port: u16,
}

impl Host {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }

    /// 解析 `host:port` 字符串；缺省端口回退到 27017。
    pub fn parse(s: &str) -> Option<Host> {
        let s = s.trim();
        let (host, port) = if let Some(tail) = s.strip_prefix('[') {
            let (address, suffix) = tail.split_once(']')?;
            address.parse::<std::net::Ipv6Addr>().ok()?;
            let port = if suffix.is_empty() {
                27017
            } else {
                suffix.strip_prefix(':')?.parse().ok()?
            };
            (address.to_owned(), port)
        } else if s.parse::<std::net::Ipv6Addr>().is_ok() {
            (s.to_owned(), 27017)
        } else {
            match s.rsplit_once(':') {
                Some((h, p)) => (h.to_owned(), p.parse().ok()?),
                None => (s.to_owned(), 27017),
            }
        };
        if host.is_empty() || port == 0 {
            return None;
        }
        Some(Host::new(host, port))
    }
}

impl std::fmt::Display for Host {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.host.contains(':') && !self.host.starts_with('[') {
            write!(f, "[{}]:{}", self.host, self.port)
        } else {
            write!(f, "{}:{}", self.host, self.port)
        }
    }
}

/// 认证来源（认证库）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AuthSource {
    /// 默认 admin。
    #[default]
    Admin,
    /// 当前数据库。
    Current,
}

/// 凭据设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialSettings {
    pub enabled: bool,
    pub user_name: String,
    pub password: String,
    /// 认证库（如 admin）。空字符串表示按机制默认（admin / $external）。
    pub auth_source: String,
    /// 显式认证机制；`None` 表示让驱动/版本适配层自动选择。
    pub mechanism: Option<AuthMechanism>,
    /// 手动指定可见数据库列表（逗号分隔）。
    pub manually_visible_dbs: String,
}

impl Default for CredentialSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            user_name: String::new(),
            password: String::new(),
            auth_source: "admin".to_string(),
            mechanism: None,
            manually_visible_dbs: String::new(),
        }
    }
}

/// TLS / SSL 设置。
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SslSettings {
    pub enabled: bool,
    /// 允许自签名 / 无效证书（对应 `tlsAllowInvalidCertificates`）。
    pub allow_invalid_certs: bool,
    /// CA 证书文件路径（`tlsCAFile`）。
    pub ca_file: Option<String>,
    /// 客户端证书（`tlsCertificateFile`）。
    pub cert_file: Option<String>,
    /// 客户端私钥（`tlsCertificateKeyFile`）；原版 PEM 模式将证书与私钥合并为一个 PEM 文件。
    pub key_file: Option<String>,
    /// 认证方式：`allow_invalid_certs == true` 对应原版 "Self-signed Certificate"，
    /// `false` 对应 "Use CA Certificate"。
    /// PEM 模式开关（"Use PEM Cert./Key:" 复选框）。
    #[serde(default)]
    pub use_pem: bool,
    /// PEM 私钥口令（`tlsCertificateKeyFilePassword`）。
    #[serde(default)]
    pub pem_passphrase: Option<String>,
    /// 每次连接都询问 PEM 口令。
    #[serde(default)]
    pub ask_pem_passphrase: bool,
    /// 高级选项（展开 CRL / 无效主机名）。
    #[serde(default)]
    pub advanced_options: bool,
    /// 证书撤销列表文件（`tlsCRLFile`）。
    #[serde(default)]
    pub crl_file: Option<String>,
    /// 允许无效/不匹配的主机名（`tlsAllowInvalidHostnames`）。
    #[serde(default)]
    pub allow_invalid_hostnames: bool,
}

/// SSH 认证方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SshAuthMethod {
    #[default]
    Password,
    PublicKey,
}

/// SSH 隧道设置。
///
/// 通过 SSH 跳板机把本地端口转发到远端 MongoDB（常用于 AWS DocumentDB / RDS）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshSettings {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth_method: SshAuthMethod,
    pub password: String,
    pub private_key_path: Option<String>,
    pub passphrase: Option<String>,
    /// 每次连接都询问密码 / 口令（原版 "Ask for password/passphrase each time"）。
    #[serde(default)]
    pub ask_password_each_time: bool,
    /// 本地监听端口（转发入口）。
    pub local_port: u16,
    /// 远端 MongoDB 主机（经跳板机可达）。
    pub remote_host: String,
    /// 远端 MongoDB 端口。
    pub remote_port: u16,
}

impl Default for SshSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            host: "bastion.example.com".to_string(),
            port: 22,
            user: "ec2-user".to_string(),
            auth_method: SshAuthMethod::Password,
            password: String::new(),
            private_key_path: None,
            passphrase: None,
            ask_password_each_time: false,
            local_port: 0, // 0 = 由系统随机分配空闲端口
            remote_host: "docdb-cluster.cluster-xxxx.docdb.amazonaws.com".to_string(),
            remote_port: 27017,
        }
    }
}

/// 完整连接设置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionSettings {
    pub name: String,
    pub mode: ConnectionMode,
    /// 种子节点列表（副本集 / 分片集群可多个；单节点 1 个）。
    pub hosts: Vec<Host>,
    /// 使用 `mongodb+srv://`（仅一个 SRV 域名，Atlas / DocumentDB 常用）。
    pub use_srv: bool,
    /// 副本集名称（mode = ReplicaSet 时）。
    pub replica_set_name: Option<String>,
    pub credential: CredentialSettings,
    pub ssh: SshSettings,
    pub ssl: SslSettings,
    /// 默认打开的数据库。
    pub default_database: Option<String>,
    /// 直连（绕过种子发现，用于单节点 / 排错）。
    pub direct_connection: bool,
}

impl Default for ConnectionSettings {
    fn default() -> Self {
        Self {
            name: "新连接".to_string(),
            mode: ConnectionMode::Standalone,
            hosts: vec![Host::new("localhost", 27017)],
            use_srv: false,
            replica_set_name: None,
            credential: CredentialSettings::default(),
            ssh: SshSettings::default(),
            ssl: SslSettings::default(),
            default_database: None,
            direct_connection: false,
        }
    }
}

impl ConnectionSettings {
    /// 构造一个本地默认连接（便于开发 / 演示）。
    pub fn local_default() -> Self {
        Self {
            name: "Local (27017)".to_string(),
            hosts: vec![Host::new("127.0.0.1", 27017)],
            ..Default::default()
        }
    }

    /// 若通过 SSH 隧道，则连接目标改为本地转发端口。
    fn effective_hosts(&self) -> Vec<Host> {
        if self.ssh.enabled {
            vec![Host::new("127.0.0.1", self.ssh.local_port)]
        } else {
            self.hosts.clone()
        }
    }

    /// 把凭据编译进 URI（用户名、密码、authMechanism、authSource）。
    fn append_credentials(&self, uri: &mut String) {
        let cred = &self.credential;
        if !cred.enabled {
            return;
        }
        let mechanism = cred.mechanism.unwrap_or_else(default_mechanism_for_uri);
        // X.509 / 部分机制不需要密码
        if !mechanism.is_passwordless() && !cred.user_name.is_empty() {
            push_userinfo(uri, &cred.user_name, &cred.password);
        } else if mechanism.is_passwordless() && !cred.user_name.is_empty() {
            // X.509 用户名为证书主题，可作 URL 用户部分
            push_userinfo(uri, &cred.user_name, "");
        }
    }

    fn query_params(&self) -> Vec<String> {
        let mut q = Vec::new();
        if let Some(rs) = &self.replica_set_name {
            if !rs.is_empty() {
                q.push(format!("replicaSet={}", encode(rs)));
            }
        }
        let cred = &self.credential;
        if cred.enabled {
            if let Some(m) = cred.mechanism {
                q.push(format!("authMechanism={}", m.uri_value()));
            }
            let source = if cred
                .mechanism
                .map(|m| m.uses_external_source())
                .unwrap_or(false)
            {
                "$external".to_string()
            } else if cred.auth_source.is_empty() {
                "admin".to_string()
            } else {
                cred.auth_source.clone()
            };
            q.push(format!("authSource={}", encode(&source)));
        }
        if self.ssl.enabled {
            q.push("tls=true".to_string());
            if self.ssl.allow_invalid_certs {
                q.push("tlsAllowInvalidCertificates=true".to_string());
            }
            if let Some(hostnames) = if self.ssl.allow_invalid_hostnames {
                Some("true")
            } else {
                None
            } {
                q.push(format!("tlsAllowInvalidHostnames={}", hostnames));
            }
            if self.ssl.use_pem {
                if let Some(cert) = &self.ssl.cert_file {
                    q.push(format!("tlsCertificateKeyFile={}", encode(cert)));
                }
                if let Some(pass) = &self.ssl.pem_passphrase {
                    if !pass.is_empty() {
                        q.push(format!("tlsCertificateKeyFilePassword={}", encode(pass)));
                    }
                }
            }
            if let Some(ca) = &self.ssl.ca_file {
                q.push(format!("tlsCAFile={}", encode(ca)));
            }
            if let Some(crl) = &self.ssl.crl_file {
                if !crl.is_empty() {
                    q.push(format!("tlsCRLFile={}", encode(crl)));
                }
            }
        }
        if self.direct_connection {
            q.push("directConnection=true".to_string());
        }
        q
    }

    /// 生成完整连接 URI。
    ///
    /// - SRV：使用 `mongodb+srv://host/...`（单域名，副本集自动发现）。
    /// - 普通：多个 `host:port` 逗号拼接。
    /// - SSH 开启时：目标被改写为本地转发端口（隧道由调用方先行建立）。
    pub fn build_uri(&self) -> String {
        let scheme = if self.use_srv {
            "mongodb+srv"
        } else {
            "mongodb"
        };
        let mut uri = format!("{}://", scheme);

        if self.use_srv {
            // SRV 仅取第一个 host 的 host 部分（域名），不带端口
            if let Some(h) = self.hosts.first() {
                if self.credential.enabled && !self.credential.user_name.is_empty() {
                    push_userinfo(
                        &mut uri,
                        &self.credential.user_name,
                        &self.credential.password,
                    );
                }
                uri.push_str(&h.host);
            }
        } else {
            self.append_credentials(&mut uri);
            let hosts = self.effective_hosts();
            let joined = hosts
                .iter()
                .map(|h| h.to_string())
                .collect::<Vec<_>>()
                .join(",");
            uri.push_str(&joined);
        }

        let q = self.query_params();
        if !q.is_empty() {
            uri.push('/');
            uri.push('?');
            uri.push_str(&q.join("&"));
        }
        uri
    }
}

/// 未显式指定机制时，URI 中省略 `authMechanism` 让官方驱动按版本协商
/// （驱动默认对 4.0+ 使用 SCRAM-SHA-256 / SHA-1）。
fn default_mechanism_for_uri() -> AuthMechanism {
    AuthMechanism::ScramSha256
}

/// 向 URI 追加 `user:password@`（已做最小 percent-encode）。
fn push_userinfo(uri: &mut String, user: &str, pass: &str) {
    uri.push_str(&encode(user));
    if !pass.is_empty() {
        uri.push(':');
        uri.push_str(&encode(pass));
    }
    uri.push('@');
}

/// 最小 percent-encode，覆盖 URI 中的保留字符。
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => {
                out.push('%');
                out.push_str(&format!("{:02X}", b));
            }
        }
    }
    out
}

/// 把连接设置持久化为 JSON 文本。
pub fn serialize_connection(c: &ConnectionSettings) -> RuboResult<String> {
    Ok(serde_json::to_string_pretty(c)?)
}

/// 从 JSON 文本解析连接设置。
pub fn deserialize_connection(s: &str) -> RuboResult<ConnectionSettings> {
    Ok(serde_json::from_str(s)?)
}

/// 校验连接设置是否可用于建立连接（轻量检查，不触网）。
pub fn validate(c: &ConnectionSettings) -> RuboResult<()> {
    if c.name.trim().is_empty() {
        return Err(crate::error::RuboError::Config("连接名称不能为空".into()));
    }
    if c.use_srv {
        if c.hosts.is_empty() || c.hosts[0].host.is_empty() {
            return Err(crate::error::RuboError::Config(
                "SRV 连接需要填写域名".into(),
            ));
        }
    } else if c.hosts.is_empty() {
        return Err(crate::error::RuboError::Config("至少需要一个主机".into()));
    }
    if c.ssh.enabled && (c.ssh.host.is_empty() || c.ssh.remote_host.is_empty()) {
        return Err(crate::error::RuboError::Config(
            "SSH 隧道需要填写跳板机与远端地址".into(),
        ));
    }
    Ok(())
}

/// 给未指定版本的 UI 提示：根据机制建议的最小版本。
pub fn recommended_min_version_for_mechanism(m: AuthMechanism) -> Option<ServerVersion> {
    match m {
        AuthMechanism::ScramSha256 => Some(ServerVersion::new(4, 0, 0)),
        AuthMechanism::ScramSha1 => Some(ServerVersion::new(3, 0, 0)),
        AuthMechanism::MongoDbCr => Some(ServerVersion::new(2, 4, 0)),
        AuthMechanism::Plain
        | AuthMechanism::X509
        | AuthMechanism::GssApi
        | AuthMechanism::AwsIam => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::AuthMechanism;

    #[test]
    fn ipv6_hosts_roundtrip() {
        for (input, expected) in [
            ("[::1]:27018", "[::1]:27018"),
            ("::1", "[::1]:27017"),
            ("[::1]", "[::1]:27017"),
        ] {
            assert_eq!(Host::parse(input).unwrap().to_string(), expected);
        }
        for input in ["localhost:0", "[::1]:0", "[invalid]:27017", "[::1]junk"] {
            assert!(Host::parse(input).is_none());
        }
    }

    #[test]
    fn tls_client_certificate_keeps_ca_file() {
        let mut c = ConnectionSettings::local_default();
        c.ssl.enabled = true;
        c.ssl.use_pem = true;
        c.ssl.cert_file = Some("client.pem".into());
        c.ssl.ca_file = Some("ca.pem".into());
        let uri = c.build_uri();
        assert!(uri.contains("tlsCAFile=ca.pem"));
        assert!(uri.contains("tlsCertificateKeyFile=client.pem"));
    }

    #[test]
    fn host_parse() {
        assert_eq!(
            Host::parse("localhost:27017"),
            Some(Host::new("localhost", 27017))
        );
        assert_eq!(Host::parse("mongo"), Some(Host::new("mongo", 27017)));
        assert_eq!(Host::parse(""), None);
    }

    #[test]
    fn standalone_uri_no_auth() {
        let c = ConnectionSettings::local_default();
        assert_eq!(c.build_uri(), "mongodb://127.0.0.1:27017");
    }

    #[test]
    fn uri_with_scram256() {
        let mut c = ConnectionSettings::local_default();
        c.credential.enabled = true;
        c.credential.user_name = "admin".into();
        c.credential.password = "pw@d".into();
        c.credential.mechanism = Some(AuthMechanism::ScramSha256);
        c.credential.auth_source = "admin".into();
        assert!(c.build_uri().contains("/?authMechanism="));
        assert_eq!(
            c.build_uri().replace("/?", "?"),
            "mongodb://admin:pw%40d@127.0.0.1:27017?authMechanism=SCRAM-SHA-256&authSource=admin"
        );
    }

    #[test]
    fn uri_replica_set() {
        let c = ConnectionSettings {
            name: "rs".into(),
            mode: ConnectionMode::ReplicaSet,
            hosts: vec![
                Host::new("a", 27017),
                Host::new("b", 27017),
                Host::new("c", 27017),
            ],
            replica_set_name: Some("rs0".into()),
            ..Default::default()
        };
        assert!(c.build_uri().contains("/?replicaSet="));
        assert_eq!(
            c.build_uri().replace("/?", "?"),
            "mongodb://a:27017,b:27017,c:27017?replicaSet=rs0"
        );
    }

    #[test]
    fn uri_srv_with_tls() {
        let c = ConnectionSettings {
            name: "atlas".into(),
            use_srv: true,
            hosts: vec![Host::new("cluster0.mongodb.net", 0)],
            credential: CredentialSettings {
                enabled: true,
                user_name: "u".into(),
                password: "p".into(),
                ..Default::default()
            },
            ssl: SslSettings {
                enabled: true,
                ..Default::default()
            },
            ..Default::default()
        };
        let uri = c.build_uri();
        assert!(uri.starts_with("mongodb+srv://u:p@cluster0.mongodb.net"));
        assert!(uri.contains("tls=true"));
    }

    #[test]
    fn uri_ssh_rewrites_target() {
        let mut c = ConnectionSettings::local_default();
        c.ssh.enabled = true;
        c.ssh.local_port = 27018;
        assert_eq!(c.build_uri(), "mongodb://127.0.0.1:27018");
    }

    #[test]
    fn uri_external_mechanism() {
        let mut c = ConnectionSettings::local_default();
        c.credential.enabled = true;
        c.credential.mechanism = Some(AuthMechanism::Plain);
        c.credential.user_name = "ldapuser".into();
        c.credential.password = "secret".into();
        assert!(c.build_uri().contains("authSource=%24external"));
        assert!(c.build_uri().contains("authMechanism=PLAIN"));
    }
}
