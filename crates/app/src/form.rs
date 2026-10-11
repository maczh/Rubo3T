//! 连接表单中间结构。
//!
//! 在 UI 表单与持久化 `ConnectionSettings` 之间做转换，保留当前编辑状态。

use rubo3t_core::settings::{
    ConnectionMode, ConnectionSettings, CredentialSettings, Host, SshAuthMethod, SshSettings,
    SslSettings,
};
use rubo3t_core::version::AuthMechanism;

#[derive(Clone, Copy, PartialEq)]
pub enum MechChoice {
    Auto,
    MongoDbCr,
    ScramSha1,
    ScramSha256,
    Plain,
    X509,
    GssApi,
    AwsIam,
}

impl MechChoice {
    pub fn to_opt(self) -> Option<AuthMechanism> {
        match self {
            MechChoice::Auto => None,
            MechChoice::MongoDbCr => Some(AuthMechanism::MongoDbCr),
            MechChoice::ScramSha1 => Some(AuthMechanism::ScramSha1),
            MechChoice::ScramSha256 => Some(AuthMechanism::ScramSha256),
            MechChoice::Plain => Some(AuthMechanism::Plain),
            MechChoice::X509 => Some(AuthMechanism::X509),
            MechChoice::GssApi => Some(AuthMechanism::GssApi),
            MechChoice::AwsIam => Some(AuthMechanism::AwsIam),
        }
    }
    pub fn from_opt(o: Option<AuthMechanism>) -> Self {
        match o {
            None => MechChoice::Auto,
            Some(AuthMechanism::MongoDbCr) => MechChoice::MongoDbCr,
            Some(AuthMechanism::ScramSha1) => MechChoice::ScramSha1,
            Some(AuthMechanism::ScramSha256) => MechChoice::ScramSha256,
            Some(AuthMechanism::Plain) => MechChoice::Plain,
            Some(AuthMechanism::X509) => MechChoice::X509,
            Some(AuthMechanism::GssApi) => MechChoice::GssApi,
            Some(AuthMechanism::AwsIam) => MechChoice::AwsIam,
        }
    }
    pub fn tr(self, lang: crate::i18n::Lang) -> &'static str {
        match self {
            MechChoice::Auto => lang.tr("mech.auto"),
            MechChoice::MongoDbCr => lang.tr("mech.mongodbcr"),
            MechChoice::ScramSha1 => lang.tr("mech.scramsha1"),
            MechChoice::ScramSha256 => lang.tr("mech.scramsha256"),
            MechChoice::Plain => lang.tr("mech.plain"),
            MechChoice::X509 => lang.tr("mech.x509"),
            MechChoice::GssApi => lang.tr("mech.gssapi"),
            MechChoice::AwsIam => lang.tr("mech.awsiam"),
        }
    }
}

#[derive(Clone)]
pub struct ConnectionForm {
    pub name: String,
    pub mode: ConnectionMode,
    pub hosts_text: String,
    pub use_srv: bool,
    pub replica_set: String,
    pub auth_enabled: bool,
    pub user: String,
    pub password: String,
    pub auth_source: String,
    pub mechanism: MechChoice,
    pub ssl_enabled: bool,
    pub ssl_allow_invalid: bool,
    pub ssh_enabled: bool,
    pub ssh_host: String,
    pub ssh_port: u16,
    pub ssh_user: String,
    pub ssh_auth: SshAuthMethod,
    pub ssh_password: String,
    pub ssh_key: String,
    pub ssh_local_port: u16,
    pub ssh_remote_host: String,
    pub ssh_remote_port: u16,
    pub default_db: String,
    pub direct: bool,
    /// 连接页：单节点主机（与 port_input 拼成 host:port）。
    pub host_input: String,
    /// 连接页：单节点端口。
    pub port_input: String,
    /// SSH 口令（Passphrase）。
    pub ssh_passphrase: String,
    /// TLS CA 证书文件路径。
    pub ssl_ca_file: String,
    /// TLS 客户端证书文件路径。
    pub ssl_cert_file: String,
    /// TLS 客户端私钥/口令文件路径。
    pub ssl_key_file: String,
    /// 认证：手动指定可见数据库开关（"Manually specify visible databases"）。
    pub auth_visible_dbs_enabled: bool,
    /// 认证：手动指定可见数据库列表（逗号分隔）。
    pub manually_visible_dbs: String,
    /// SSH：每次都询问密码/口令（"Ask for password/passphrase each time"）。
    pub ssh_ask_password_each_time: bool,
    /// TLS：PEM 模式开关（"Use PEM Cert./Key:"）。
    pub ssl_use_pem: bool,
    /// TLS：PEM 私钥口令。
    pub ssl_pem_passphrase: String,
    /// TLS：每次都询问 PEM 口令。
    pub ssl_ask_pem_passphrase: bool,
    /// TLS：高级选项（展开 CRL / 无效主机名）。
    pub ssl_advanced_options: bool,
    /// TLS：CRL 文件路径。
    pub ssl_crl_file: String,
    /// TLS：允许无效/不匹配主机名。
    pub ssl_allow_invalid_hostnames: bool,
    /// 密码明文切换（眼睛图标）。
    pub show_auth_password: bool,
    pub show_ssh_password: bool,
    pub show_ssh_passphrase: bool,
    pub show_pem_passphrase: bool,
    /// From URI 输入框内容。
    pub uri_text: String,
    /// 设置弹窗当前激活的标签页索引（Connection/Auth/SSH/TLS/Advanced）。
    pub tab_index: usize,
}

impl Default for ConnectionForm {
    fn default() -> Self {
        Self {
            name: "新连接".into(),
            mode: ConnectionMode::Standalone,
            hosts_text: "127.0.0.1:27017".into(),
            use_srv: false,
            replica_set: String::new(),
            auth_enabled: false,
            user: String::new(),
            password: String::new(),
            auth_source: "admin".into(),
            mechanism: MechChoice::Auto,
            ssl_enabled: false,
            ssl_allow_invalid: false,
            ssh_enabled: false,
            ssh_host: "bastion.example.com".into(),
            ssh_port: 22,
            ssh_user: "ec2-user".into(),
            ssh_auth: SshAuthMethod::PublicKey,
            ssh_password: String::new(),
            ssh_key: String::new(),
            ssh_local_port: 27018,
            ssh_remote_host: "docdb.cluster.amazonaws.com".into(),
            ssh_remote_port: 27017,
            default_db: String::new(),
            direct: false,
            host_input: "127.0.0.1".into(),
            port_input: "27017".into(),
            ssh_passphrase: String::new(),
            ssl_ca_file: String::new(),
            ssl_cert_file: String::new(),
            ssl_key_file: String::new(),
            auth_visible_dbs_enabled: false,
            manually_visible_dbs: String::new(),
            ssh_ask_password_each_time: false,
            ssl_use_pem: false,
            ssl_pem_passphrase: String::new(),
            ssl_ask_pem_passphrase: false,
            ssl_advanced_options: false,
            ssl_crl_file: String::new(),
            ssl_allow_invalid_hostnames: false,
            show_auth_password: false,
            show_ssh_password: false,
            show_ssh_passphrase: false,
            show_pem_passphrase: false,
            uri_text: String::new(),
            tab_index: 0,
        }
    }
}

impl ConnectionForm {
    pub fn to_settings(&self) -> ConnectionSettings {
        // 单节点用 host_input:port_input；副本集/分片用多行 hosts_text。
        let hosts = match self.mode {
            ConnectionMode::Standalone => {
                let mut v = Vec::new();
                let s = format!("{}:{}", self.host_input.trim(), self.port_input.trim());
                if let Some(h) = Host::parse(&s) {
                    v.push(h);
                }
                v
            }
            _ => self
                .hosts_text
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .filter_map(Host::parse)
                .collect::<Vec<_>>(),
        };
        ConnectionSettings {
            name: self.name.clone(),
            mode: self.mode,
            hosts,
            use_srv: self.use_srv,
            replica_set_name: if self.replica_set.is_empty() {
                None
            } else {
                Some(self.replica_set.clone())
            },
            credential: CredentialSettings {
                enabled: self.auth_enabled,
                user_name: self.user.clone(),
                password: self.password.clone(),
                auth_source: if self.auth_source.is_empty() {
                    "admin".into()
                } else {
                    self.auth_source.clone()
                },
                mechanism: self.mechanism.to_opt(),
                manually_visible_dbs: if self.auth_visible_dbs_enabled {
                    self.manually_visible_dbs.clone()
                } else {
                    String::new()
                },
            },
            ssh: SshSettings {
                enabled: self.ssh_enabled,
                host: self.ssh_host.clone(),
                port: self.ssh_port,
                user: self.ssh_user.clone(),
                auth_method: self.ssh_auth,
                password: self.ssh_password.clone(),
                private_key_path: if self.ssh_key.is_empty() {
                    None
                } else {
                    Some(self.ssh_key.clone())
                },
                passphrase: if self.ssh_passphrase.is_empty() {
                    None
                } else {
                    Some(self.ssh_passphrase.clone())
                },
                ask_password_each_time: self.ssh_ask_password_each_time,
                local_port: self.ssh_local_port,
                remote_host: self.ssh_remote_host.clone(),
                remote_port: self.ssh_remote_port,
            },
            ssl: SslSettings {
                enabled: self.ssl_enabled,
                allow_invalid_certs: self.ssl_allow_invalid,
                ca_file: if self.ssl_ca_file.is_empty() {
                    None
                } else {
                    Some(self.ssl_ca_file.clone())
                },
                cert_file: if self.ssl_cert_file.is_empty() {
                    None
                } else {
                    Some(self.ssl_cert_file.clone())
                },
                key_file: if self.ssl_key_file.is_empty() {
                    None
                } else {
                    Some(self.ssl_key_file.clone())
                },
                use_pem: self.ssl_use_pem,
                pem_passphrase: if self.ssl_pem_passphrase.is_empty() {
                    None
                } else {
                    Some(self.ssl_pem_passphrase.clone())
                },
                ask_pem_passphrase: self.ssl_ask_pem_passphrase,
                advanced_options: self.ssl_advanced_options,
                crl_file: if self.ssl_crl_file.is_empty() {
                    None
                } else {
                    Some(self.ssl_crl_file.clone())
                },
                allow_invalid_hostnames: self.ssl_allow_invalid_hostnames,
            },
            default_database: if self.default_db.is_empty() {
                None
            } else {
                Some(self.default_db.clone())
            },
            direct_connection: self.direct,
        }
    }

    pub fn from_settings(s: &ConnectionSettings) -> Self {
        Self {
            name: s.name.clone(),
            mode: s.mode,
            hosts_text: s
                .hosts
                .iter()
                .map(|h| h.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
            use_srv: s.use_srv,
            replica_set: s.replica_set_name.clone().unwrap_or_default(),
            auth_enabled: s.credential.enabled,
            user: s.credential.user_name.clone(),
            password: s.credential.password.clone(),
            auth_source: s.credential.auth_source.clone(),
            mechanism: MechChoice::from_opt(s.credential.mechanism),
            ssl_enabled: s.ssl.enabled,
            ssl_allow_invalid: s.ssl.allow_invalid_certs,
            ssh_enabled: s.ssh.enabled,
            ssh_host: s.ssh.host.clone(),
            ssh_port: s.ssh.port,
            ssh_user: s.ssh.user.clone(),
            ssh_auth: s.ssh.auth_method,
            ssh_password: s.ssh.password.clone(),
            ssh_key: s.ssh.private_key_path.clone().unwrap_or_default(),
            ssh_passphrase: s.ssh.passphrase.clone().unwrap_or_default(),
            ssh_ask_password_each_time: s.ssh.ask_password_each_time,
            ssh_local_port: s.ssh.local_port,
            ssh_remote_host: s.ssh.remote_host.clone(),
            ssh_remote_port: s.ssh.remote_port,
            default_db: s.default_database.clone().unwrap_or_default(),
            direct: s.direct_connection,
            auth_visible_dbs_enabled: !s.credential.manually_visible_dbs.is_empty(),
            manually_visible_dbs: s.credential.manually_visible_dbs.clone(),
            host_input: s
                .hosts
                .first()
                .map(|h| h.host.clone())
                .unwrap_or_else(|| "127.0.0.1".into()),
            port_input: s
                .hosts
                .first()
                .map(|h| h.port.to_string())
                .unwrap_or_else(|| "27017".into()),
            ssl_ca_file: s.ssl.ca_file.clone().unwrap_or_default(),
            ssl_cert_file: s.ssl.cert_file.clone().unwrap_or_default(),
            ssl_key_file: s.ssl.key_file.clone().unwrap_or_default(),
            ssl_use_pem: s.ssl.use_pem,
            ssl_pem_passphrase: s.ssl.pem_passphrase.clone().unwrap_or_default(),
            ssl_ask_pem_passphrase: s.ssl.ask_pem_passphrase,
            ssl_advanced_options: s.ssl.advanced_options,
            ssl_crl_file: s.ssl.crl_file.clone().unwrap_or_default(),
            ssl_allow_invalid_hostnames: s.ssl.allow_invalid_hostnames,
            show_auth_password: false,
            show_ssh_password: false,
            show_ssh_passphrase: false,
            show_pem_passphrase: false,
            uri_text: String::new(),
            tab_index: 0,
        }
    }

    /// 解析 MongoDB URI 并回填到表单（best-effort），对应原版 "From URI" 按钮。
    pub fn apply_uri(&mut self, uri: &str) {
        let s = uri.trim();
        if s.is_empty() {
            return;
        }
        let rest = if let Some(r) = s.strip_prefix("mongodb+srv://") {
            self.use_srv = true;
            r
        } else if let Some(r) = s.strip_prefix("mongodb://") {
            self.use_srv = false;
            r
        } else {
            return;
        };

        let (auth_host, query) = match rest.split_once('?') {
            Some((a, q)) => (a, Some(q)),
            None => (rest, None),
        };
        let (host_part, db) = match auth_host.split_once('/') {
            Some((h, d)) => (h, if d.is_empty() { None } else { Some(d) }),
            None => (auth_host, None),
        };
        let host_only = if let Some((userinfo, h)) = host_part.rsplit_once('@') {
            if let Some((u, p)) = userinfo.split_once(':') {
                self.auth_enabled = true;
                self.user = u.to_string();
                self.password = p.to_string();
            } else if !userinfo.is_empty() {
                self.auth_enabled = true;
                self.user = userinfo.to_string();
            }
            h
        } else {
            host_part
        };

        let hosts: Vec<&str> = host_only.split(',').filter(|h| !h.is_empty()).collect();
        if hosts.is_empty() {
            return;
        }
        self.direct = false;
        if self.use_srv {
            let domain = hosts[0].split(':').next().unwrap_or(hosts[0]);
            self.hosts_text = domain.to_string();
            self.host_input = domain.to_string();
            self.port_input = "27017".to_string();
            self.mode = ConnectionMode::Standalone;
        } else if hosts.len() > 1 {
            self.mode = ConnectionMode::ReplicaSet;
            self.hosts_text = hosts.iter().map(|h| h.to_string()).collect::<Vec<_>>().join("\n");
            if let Some(first) = hosts.first() {
                let (h, p) = first.rsplit_once(':').unwrap_or((first, "27017"));
                self.host_input = h.to_string();
                self.port_input = p.to_string();
            }
        } else {
            self.mode = ConnectionMode::Standalone;
            self.hosts_text = hosts[0].to_string();
            let (h, p) = hosts[0].rsplit_once(':').unwrap_or((hosts[0], "27017"));
            self.host_input = h.to_string();
            self.port_input = p.to_string();
        }
        if let Some(d) = db {
            self.default_db = d.to_string();
        }
        if let Some(q) = query {
            for pair in q.split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    match k {
                        "authSource" => self.auth_source = v.to_string(),
                        "authMechanism" => {
                            self.mechanism = match v.to_uppercase().as_str() {
                                "SCRAM-SHA-1" => MechChoice::ScramSha1,
                                "SCRAM-SHA-256" => MechChoice::ScramSha256,
                                "MONGODB-CR" => MechChoice::MongoDbCr,
                                _ => MechChoice::Auto,
                            };
                        }
                        "tls" => {
                            if v == "true" {
                                self.ssl_enabled = true;
                            }
                        }
                        "tlsAllowInvalidCertificates" => {
                            if v == "true" {
                                self.ssl_allow_invalid = true;
                            }
                        }
                        "replicaSet" => {
                            self.replica_set = v.to_string();
                            if !self.use_srv {
                                self.mode = ConnectionMode::ReplicaSet;
                            }
                        }
                        "directConnection" => self.direct = v.eq_ignore_ascii_case("true"),
                        _ => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn importing_multi_host_uri_clears_stale_direct_connection() {
        let mut form = ConnectionForm {
            direct: true,
            ..Default::default()
        };

        form.apply_uri(
            "mongodb://user:pass@first.example:27017,second.example:27017/dev",
        );

        let settings = form.to_settings();
        assert_eq!(settings.hosts.len(), 2);
        assert_eq!(settings.mode, ConnectionMode::ReplicaSet);
        assert!(!settings.direct_connection);
        assert!(!settings.build_uri().contains("directConnection=true"));
    }

    #[test]
    fn importing_direct_connection_option_preserves_its_value() {
        let mut form = ConnectionForm::default();

        form.apply_uri("mongodb://localhost:27017/dev?directConnection=true");

        assert!(form.to_settings().direct_connection);
    }
}
