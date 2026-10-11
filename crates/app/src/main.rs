//! Rubo3T —— 用 Rust 重写的 MongoDB 管理工具（MVP）。
//!
//! 目标：在保持原 Robo3T 1.4.4 使用习惯的前提下，用官方 `mongodb` Rust 驱动
//! 覆盖 MongoDB 3.x → 8.x、副本集 / 分片集群 / Atlas(SRV) / RDS(DocumentDB)，
//! 并支持各版本认证机制与协议差异（差异集中由 `rubo3t-core` 的 `FeatureGate` 处理）。
//!
//! GUI 采用 egui（纯 Rust 即时模式、跨平台原生桌面），通过 tokio 运行时同步驱动异步驱动调用。
//!
//! 注意：egui 的闭包会捕获 `&mut self`，而嵌套闭包（如 `ui.columns` 内部的闭包）
//! 不能同时再借用 `&mut self`。因此渲染层采用「把数据/状态取到局部变量 → 闭包内
//! 只读写局部变量并收集事件 → 闭包外再把局部写回 self 并执行动作」的惯用模式。

mod i18n;

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use egui::{
    Align2, CentralPanel, CollapsingHeader, Color32, ComboBox, Context, DragValue, ScrollArea,
    SidePanel, TextEdit, TopBottomPanel, Vec2, Window,
};

use i18n::{trf, Lang};
use rubo3t_core::bson::doc;
use rubo3t_core::bson_ext;
use rubo3t_core::domain::{CollectionKind, IndexInfo as DomainIndex};
use rubo3t_core::settings::{
    ConnectionMode, ConnectionSettings, CredentialSettings, Host, SshAuthMethod, SshSettings,
    SslSettings,
};
use rubo3t_core::version::AuthMechanism;
use rubo3t_core::Document;
use rubo3t_mongo::MongoService;
use rubo3t_ssh::Tunnel;

// ---------------------------------------------------------------- 连接表单

#[derive(Clone, Copy, PartialEq)]
enum MechChoice {
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
    fn to_opt(self) -> Option<AuthMechanism> {
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
    fn from_opt(o: Option<AuthMechanism>) -> Self {
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
    fn tr(self, lang: Lang) -> &'static str {
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
struct ConnectionForm {
    name: String,
    mode: ConnectionMode,
    hosts_text: String,
    use_srv: bool,
    replica_set: String,
    auth_enabled: bool,
    user: String,
    password: String,
    auth_source: String,
    mechanism: MechChoice,
    ssl_enabled: bool,
    ssl_allow_invalid: bool,
    ssh_enabled: bool,
    ssh_host: String,
    ssh_port: u16,
    ssh_user: String,
    ssh_auth: SshAuthMethod,
    ssh_password: String,
    ssh_key: String,
    ssh_local_port: u16,
    ssh_remote_host: String,
    ssh_remote_port: u16,
    default_db: String,
    direct: bool,
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
        }
    }
}

impl ConnectionForm {
    fn to_settings(&self) -> ConnectionSettings {
        let hosts = self
            .hosts_text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .filter_map(Host::parse)
            .collect::<Vec<_>>();
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
                manually_visible_dbs: String::new(),
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
                passphrase: None,
                local_port: self.ssh_local_port,
                remote_host: self.ssh_remote_host.clone(),
                remote_port: self.ssh_remote_port,
            },
            ssl: SslSettings {
                enabled: self.ssl_enabled,
                allow_invalid_certs: self.ssl_allow_invalid,
                ca_file: None,
                cert_file: None,
                key_file: None,
            },
            default_database: if self.default_db.is_empty() {
                None
            } else {
                Some(self.default_db.clone())
            },
            direct_connection: self.direct,
        }
    }

    fn from_settings(s: &ConnectionSettings) -> Self {
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
            ssh_local_port: s.ssh.local_port,
            ssh_remote_host: s.ssh.remote_host.clone(),
            ssh_remote_port: s.ssh.remote_port,
            default_db: s.default_database.clone().unwrap_or_default(),
            direct: s.direct_connection,
        }
    }
}

// ---------------------------------------------------------------- 应用设置

#[derive(serde::Serialize, serde::Deserialize, Default)]
struct AppSettings {
    #[serde(default)]
    lang: Lang,
}

// ---------------------------------------------------------------- 应用状态

struct AppState {
    rt: tokio::runtime::Runtime,
    connections_path: PathBuf,
    settings_path: PathBuf,
    connections: Vec<ConnectionSettings>,
    lang: Lang,

    dlg_open: bool,
    dlg: ConnectionForm,
    dlg_selected: Option<usize>,

    service: Option<MongoService>,
    tunnel: Option<Tunnel>,
    server_info: Option<rubo3t_core::domain::ServerInfo>,
    active_name: String,

    databases: Vec<rubo3t_core::domain::DatabaseInfo>,
    collections: HashMap<String, Vec<rubo3t_core::domain::CollectionInfo>>,
    selected_db: Option<String>,
    selected_coll: Option<String>,

    filter_text: String,
    sort_text: String,
    limit: u64,
    skip: u64,
    documents: Vec<Document>,
    selected_doc: Option<usize>,
    selected_doc_text: String,
    query_msg: String,
    show_caps: bool,

    indexes: Vec<DomainIndex>,
    users: Vec<rubo3t_core::domain::UserInfo>,

    shell_input: String,
    shell_output: String,
    shell_db: String,

    toast_msg: Option<(String, bool)>,
}

impl AppState {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config_dir = dirs::config_dir().map(|p| p.join("Rubo3T")).unwrap_or_else(|| PathBuf::from("Rubo3T"));
        let _ = std::fs::create_dir_all(&config_dir);
        let connections_path = config_dir.join("connections.json");
        let settings_path = config_dir.join("settings.json");

        let settings = Self::load_settings(&settings_path);
        let connections = Self::load_connections(&connections_path);
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("无法创建 tokio 运行时");

        load_cjk_fonts(&cc.egui_ctx);

        Self {
            rt,
            connections_path,
            settings_path,
            connections,
            lang: settings.lang,
            dlg_open: false,
            dlg: ConnectionForm::default(),
            dlg_selected: None,
            service: None,
            tunnel: None,
            server_info: None,
            active_name: String::new(),
            databases: Vec::new(),
            collections: HashMap::new(),
            selected_db: None,
            selected_coll: None,
            filter_text: "{}".to_string(),
            sort_text: "{}".to_string(),
            limit: 50,
            skip: 0,
            documents: Vec::new(),
            selected_doc: None,
            selected_doc_text: String::new(),
            query_msg: String::new(),
            show_caps: false,
            indexes: Vec::new(),
            users: Vec::new(),
            shell_input: "{\"buildInfo\": 1}".to_string(),
            shell_output: String::new(),
            shell_db: "admin".to_string(),
            toast_msg: None,
        }
    }

    // ----- 持久化 -----

    fn load_connections(path: &PathBuf) -> Vec<ConnectionSettings> {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    fn save_connections(&self) {
        if let Ok(s) = serde_json::to_string_pretty(&self.connections) {
            let _ = std::fs::write(&self.connections_path, s);
        }
    }

    fn load_settings(path: &PathBuf) -> AppSettings {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => AppSettings::default(),
        }
    }

    fn save_settings(&self) {
        let s = AppSettings { lang: self.lang };
        if let Ok(json) = serde_json::to_string_pretty(&s) {
            let _ = std::fs::write(&self.settings_path, json);
        }
    }

    // ----- 连接管理 -----

    fn connect(&mut self, settings: ConnectionSettings) {
        self.tunnel = None;
        if settings.ssh.enabled {
            match Tunnel::start(&settings.ssh) {
                Ok(t) => self.tunnel = Some(t),
                Err(e) => {
                    self.toast(&format!("{}: {}", self.lang.tr("msg.ssh_failed"), e), true);
                    return;
                }
            }
        }
        match self.rt.block_on(MongoService::connect(&settings)) {
            Ok(svc) => {
                let info = svc.server_info().cloned();
                self.service = Some(svc);
                self.server_info = info.clone();
                self.active_name = settings.name.clone();
                let version_str = info
                    .as_ref()
                    .map(|i| i.version.to_string())
                    .unwrap_or_default();
                self.status(&trf(
                    self.lang,
                    "msg.connected",
                    &[settings.name.as_str(), version_str.as_str()],
                ));
                self.load_databases();
            }
            Err(e) => {
                self.tunnel = None;
                self.toast(&format!("{}: {}", self.lang.tr("msg.connect_failed"), e), true);
                self.status(&format!("{}: {}", self.lang.tr("msg.connect_failed"), e));
            }
        }
    }

    fn disconnect(&mut self) {
        self.service = None;
        self.tunnel = None;
        self.server_info = None;
        self.databases.clear();
        self.collections.clear();
        self.selected_db = None;
        self.selected_coll = None;
        self.documents.clear();
        self.selected_doc = None;
        self.selected_doc_text.clear();
        self.indexes.clear();
        self.users.clear();
        self.status(self.lang.tr("msg.disconnected"));
    }

    fn status(&mut self, msg: &str) {
        self.query_msg = msg.to_string();
    }

    fn toast(&mut self, msg: &str, is_err: bool) {
        self.toast_msg = Some((msg.to_string(), is_err));
    }

    // ----- 数据加载 -----

    fn load_databases(&mut self) {
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.list_databases()) {
                Ok(dbs) => {
                    self.databases = dbs;
                    let names: Vec<String> =
                        self.databases.iter().map(|d| d.name.clone()).collect();
                    for name in &names {
                        self.load_collections(name);
                    }
                }
                Err(e) => self.toast(&format!("{}: {}", self.lang.tr("msg.load_dbs_failed"), e), true),
            }
        }
    }

    fn load_collections(&mut self, db: &str) {
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.list_collections(db)) {
                Ok(cols) => {
                    self.collections.insert(db.to_string(), cols);
                }
                Err(e) => self.toast(&format!("{}: {}", self.lang.tr("msg.load_colls_failed"), e), true),
            }
        }
    }

    fn select_collection(&mut self, db: &str, coll: &str) {
        self.selected_db = Some(db.to_string());
        self.selected_coll = Some(coll.to_string());
        self.filter_text = "{}".to_string();
        self.sort_text = "{}".to_string();
        self.run_query();
    }

    fn run_query(&mut self) {
        let (db, coll) = match (&self.selected_db, &self.selected_coll) {
            (Some(d), Some(c)) => (d.clone(), c.clone()),
            _ => {
                self.status(self.lang.tr("query.select_collection_first"));
                return;
            }
        };
        let filter = match bson_ext::parse_filter_text(&self.filter_text) {
            Ok(d) => d,
            Err(e) => {
                self.status(&trf(self.lang, "msg.parse.filter", &[&e.to_string()]));
                return;
            }
        };
        let sort = match bson_ext::parse_filter_text(&self.sort_text) {
            Ok(d) => d,
            Err(e) => {
                self.status(&trf(self.lang, "msg.parse.sort", &[&e.to_string()]));
                return;
            }
        };
        if let Some(svc) = &self.service {
            match self
                .rt
                .block_on(svc.find(&db, &coll, &filter, &sort, self.limit, self.skip))
            {
                Ok(docs) => {
                    self.documents = docs;
                    self.selected_doc = None;
                    self.selected_doc_text.clear();
                    self.status(&trf(
                        self.lang,
                        "msg.query_returned",
                        &[&self.documents.len().to_string()],
                    ));
                }
                Err(e) => self.status(&format!("{}: {}", self.lang.tr("msg.query_failed"), e)),
            }
        }
    }

    fn select_document(&mut self, i: usize) {
        if let Some(doc) = self.documents.get(i) {
            match bson_ext::document_to_text(doc) {
                Ok(t) => self.selected_doc_text = t,
                Err(e) => self.toast(&format!("{}: {}", self.lang.tr("msg.doc_convert_failed"), e), true),
            }
            self.selected_doc = Some(i);
        }
    }

    fn save_document(&mut self) {
        let (db, coll) = match (&self.selected_db, &self.selected_coll) {
            (Some(d), Some(c)) => (d.clone(), c.clone()),
            _ => {
                self.toast(self.lang.tr("msg.select_collection_first"), true);
                return;
            }
        };
        let new_doc = match bson_ext::parse_document_text(&self.selected_doc_text) {
            Ok(d) => d,
            Err(e) => {
                self.toast(&format!("{}: {}", self.lang.tr("msg.parse.doc"), e), true);
                return;
            }
        };
        let id = match new_doc.get("_id").cloned() {
            Some(id) => id,
            None => {
                self.toast(self.lang.tr("doc.no_id"), true);
                return;
            }
        };
        let mut upd = new_doc.clone();
        upd.remove("_id");
        let filter = doc! { "_id": id };
        let update = doc! { "$set": upd };
        if let Some(svc) = &self.service {
            match self
                .rt
                .block_on(svc.update_one(&db, &coll, &filter, &update, false))
            {
                Ok(()) => {
                    self.toast(self.lang.tr("doc.saved"), false);
                    self.run_query();
                }
                Err(e) => self.toast(&format!("{}: {}", self.lang.tr("msg.save_doc_failed"), e), true),
            }
        }
    }

    fn delete_document(&mut self) {
        let (db, coll) = match (&self.selected_db, &self.selected_coll) {
            (Some(d), Some(c)) => (d.clone(), c.clone()),
            _ => {
                self.toast(self.lang.tr("msg.select_collection_first"), true);
                return;
            }
        };
        let id = match self
            .selected_doc
            .and_then(|i| self.documents.get(i))
            .and_then(|d| d.get("_id").cloned())
        {
            Some(id) => id,
            None => {
                self.toast(self.lang.tr("msg.select_doc_first"), true);
                return;
            }
        };
        let filter = doc! { "_id": id };
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.delete_one(&db, &coll, &filter)) {
                Ok(()) => {
                    self.toast(self.lang.tr("doc.deleted"), false);
                    self.run_query();
                }
                Err(e) => self.toast(&format!("{}: {}", self.lang.tr("msg.delete_doc_failed"), e), true),
            }
        }
    }

    fn insert_document(&mut self) {
        let (db, coll) = match (&self.selected_db, &self.selected_coll) {
            (Some(d), Some(c)) => (d.clone(), c.clone()),
            _ => {
                self.toast(self.lang.tr("msg.select_collection_first"), true);
                return;
            }
        };
        let new_doc = match bson_ext::parse_document_text(&self.selected_doc_text) {
            Ok(d) => d,
            Err(e) => {
                self.toast(&format!("{}: {}", self.lang.tr("msg.parse.doc"), e), true);
                return;
            }
        };
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.insert_one(&db, &coll, &new_doc)) {
                Ok(()) => {
                    self.toast(self.lang.tr("doc.inserted"), false);
                    self.run_query();
                }
                Err(e) => self.toast(&format!("{}: {}", self.lang.tr("msg.insert_doc_failed"), e), true),
            }
        }
    }

    fn run_shell(&mut self) {
        let cmd = match bson_ext::parse_document_text(&self.shell_input) {
            Ok(d) => d,
            Err(e) => {
                self.shell_output = format!("{}: {}", self.lang.tr("msg.parse.doc"), e);
                return;
            }
        };
        let db = if self.shell_db.is_empty() {
            self.selected_db.clone().unwrap_or_else(|| "admin".into())
        } else {
            self.shell_db.clone()
        };
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.run_command(&db, &cmd)) {
                Ok(res) => match bson_ext::document_to_text(&res) {
                    Ok(t) => self.shell_output = t,
                    Err(e) => self.shell_output = format!("{}: {}", self.lang.tr("msg.doc_convert_failed"), e),
                },
                Err(e) => self.shell_output = format!("{}: {}", self.lang.tr("msg.query_failed"), e),
            }
        }
    }

    fn load_indexes(&mut self) {
        let (db, coll) = match (&self.selected_db, &self.selected_coll) {
            (Some(d), Some(c)) => (d.clone(), c.clone()),
            _ => {
                self.toast(self.lang.tr("msg.select_collection_first"), true);
                return;
            }
        };
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.list_indexes(&db, &coll)) {
                Ok(i) => self.indexes = i,
                Err(e) => self.toast(&format!("{}: {}", self.lang.tr("index.load_failed"), e), true),
            }
        }
    }

    fn load_users(&mut self) {
        let db = match &self.selected_db {
            Some(d) => d.clone(),
            None => {
                self.toast(self.lang.tr("msg.select_collection_first"), true);
                return;
            }
        };
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.list_users(&db)) {
                Ok(u) => self.users = u,
                Err(e) => self.toast(&format!("{}: {}", self.lang.tr("user.load_failed"), e), true),
            }
        }
    }
}

impl eframe::App for AppState {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        self.render_menu(ctx);
        self.render_explorer(ctx);
        self.render_central(ctx);
        self.render_shell(ctx);
        self.render_dialog(ctx);
        self.render_toast(ctx);
    }
}

// ---------------------------------------------------------------- 渲染：菜单

impl AppState {
    fn render_menu(&mut self, ctx: &Context) {
        let lang = self.lang;
        let server_info = self.server_info.clone();
        let mut act_manage = false;
        let mut act_disconnect = false;
        let mut act_caps = false;
        let mut act_lang = false;

        TopBottomPanel::top("top").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button(lang.tr("menu.connect")).clicked() {
                    act_manage = true;
                }
                if self.service.is_some() && ui.button(lang.tr("menu.disconnect")).clicked() {
                    act_disconnect = true;
                }
                ui.separator();
                if let Some(info) = &server_info {
                    ui.label(format!(
                        "● {}: {}  |  MongoDB {}  {}",
                        lang.tr("status.connected"),
                        self.active_name,
                        info.version,
                        if info.is_cluster { lang.tr("status.cluster") } else { "" }
                    ));
                    if ui.button(lang.tr("menu.version_caps")).clicked() {
                        act_caps = true;
                    }
                } else {
                    ui.label(format!("● {}", lang.tr("status.not_connected")));
                }
                ui.separator();
                if ui.button(lang.label()).clicked() {
                    act_lang = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_err = self.query_msg.contains("失败")
                        || self.query_msg.contains("错误")
                        || self.query_msg.contains("failed")
                        || self.query_msg.contains("error");
                    let color = if is_err { Color32::RED } else { Color32::GRAY };
                    ui.colored_label(color, &self.query_msg);
                });
            });
        });

        if act_manage {
            self.dlg = ConnectionForm::default();
            self.dlg_selected = None;
            self.dlg_open = true;
        }
        if act_disconnect {
            self.disconnect();
        }
        if act_caps {
            self.show_caps = !self.show_caps;
        }
        if act_lang {
            self.lang = match self.lang {
                Lang::Zh => Lang::En,
                Lang::En => Lang::Zh,
            };
            self.save_settings();
        }
    }

    fn render_explorer(&mut self, ctx: &Context) {
        let lang = self.lang;
        let databases = self.databases.clone();
        let collections = self.collections.clone();
        let sel_db = self.selected_db.clone();
        let sel_coll = self.selected_coll.clone();
        let connected = self.service.is_some();
        let mut clicked: Option<(String, String)> = None;

        SidePanel::left("explorer")
            .resizable(true)
            .default_width(260.0)
            .show(ctx, |ui| {
                ui.heading(lang.tr("explorer.title"));
                if !connected {
                    ui.label(lang.tr("explorer.not_connected"));
                    return;
                }
                ui.separator();
                for db in &databases {
                    CollapsingHeader::new(db.name.as_str()).show(ui, |ui| {
                        if let Some(cols) = collections.get(&db.name) {
                            if cols.is_empty() {
                                ui.label(lang.tr("explorer.no_collections"));
                            }
                            for c in cols {
                                let label = match c.kind {
                                    CollectionKind::View => format!("▣ {}", c.name),
                                    CollectionKind::Timeseries => format!("◷ {}", c.name),
                                    _ => format!("▤ {}", c.name),
                                };
                                let selected = sel_coll.as_deref() == Some(&c.name)
                                    && sel_db.as_deref() == Some(&db.name);
                                if ui.selectable_label(selected, label).clicked() {
                                    clicked = Some((db.name.clone(), c.name.clone()));
                                }
                            }
                        }
                    });
                }
            });

        if let Some((d, c)) = clicked {
            self.select_collection(&d, &c);
        }
    }

    fn render_central(&mut self, ctx: &Context) {
        let lang = self.lang;
        if self.service.is_none() {
            CentralPanel::default().show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.heading(lang.tr("app.title"));
                    ui.label(lang.tr("app.subtitle"));
                    ui.add_space(10.0);
                    if ui.button(lang.tr("welcome.open_connect")).clicked() {
                        self.dlg = ConnectionForm::default();
                        self.dlg_selected = None;
                        self.dlg_open = true;
                    }
                });
            });
            return;
        }

        let show_caps = self.show_caps;
        let server_info = self.server_info.clone();
        let mut filter = self.filter_text.clone();
        let mut sort = self.sort_text.clone();
        let mut limit = self.limit;
        let documents = self.documents.clone();
        let indexes = self.indexes.clone();
        let users = self.users.clone();
        let selected_doc_idx = self.selected_doc;
        let mut doc_text = self.selected_doc_text.clone();

        let mut act_run = false;
        let mut act_new = false;
        let mut act_save = false;
        let mut act_delete = false;
        let mut act_insert = false;
        let mut act_index = false;
        let mut act_user = false;
        let mut clicked_doc: Option<usize> = None;

        CentralPanel::default().show(ctx, |ui| {
            if show_caps {
                if let Some(info) = &server_info {
                    ui.group(|ui| {
                        ui.heading(format!("{} (MongoDB {})", lang.tr("menu.version_caps"), info.version));
                        ui.horizontal_wrapped(|ui| {
                            for cap in &info.feature_summary {
                                let txt = format!(
                                    "{} {} ({})",
                                    if cap.supported { "✓" } else { "✗" },
                                    cap.name,
                                    cap.since
                                );
                                let c = if cap.supported {
                                    Color32::GREEN
                                } else {
                                    Color32::DARK_GRAY
                                };
                                ui.colored_label(c, txt);
                                ui.separator();
                            }
                        });
                    });
                }
            }
            ui.horizontal(|ui| {
                ui.label(lang.tr("query.filter"));
                ui.add(TextEdit::singleline(&mut filter).desired_width(220.0));
                ui.label(lang.tr("query.sort"));
                ui.add(TextEdit::singleline(&mut sort).desired_width(160.0));
                ui.label(lang.tr("query.limit"));
                ui.add(DragValue::new(&mut limit).range(1..=1000));
                if ui.button(lang.tr("query.run")).clicked() {
                    act_run = true;
                }
                if ui.button(lang.tr("query.new_doc")).clicked() {
                    act_new = true;
                }
            });
            ui.separator();
            ui.columns(2, |cols| {
                cols[0].vertical(|ui| {
                    ui.label(format!("{} ({})", lang.tr("query.doc_list"), documents.len()));
                    ScrollArea::vertical().show(ui, |ui| {
                        for (i, doc) in documents.iter().enumerate() {
                            let preview = bson_ext::document_to_text(doc)
                                .map(|t| t.chars().take(80).collect::<String>())
                                .unwrap_or_default();
                            let selected = selected_doc_idx == Some(i);
                            if ui.selectable_label(selected, preview).clicked() {
                                clicked_doc = Some(i);
                            }
                        }
                    });
                });
                cols[1].vertical(|ui| {
                    ui.horizontal(|ui| {
                        if ui.button(lang.tr("doc.save")).clicked() {
                            act_save = true;
                        }
                        if ui.button(lang.tr("doc.delete")).clicked() {
                            act_delete = true;
                        }
                        if ui.button(lang.tr("doc.insert")).clicked() {
                            act_insert = true;
                        }
                        if ui.button(lang.tr("index.title")).clicked() {
                            act_index = true;
                        }
                        if ui.button(lang.tr("user.title")).clicked() {
                            act_user = true;
                        }
                    });
                    ScrollArea::vertical().show(ui, |ui| {
                        ui.add(TextEdit::multiline(&mut doc_text).code_editor().desired_rows(18));
                    });
                    if !indexes.is_empty() {
                        ui.collapsing(format!("{} ({})", lang.tr("index.title"), indexes.len()), |ui| {
                            for idx in &indexes {
                                ui.label(format!(
                                    "• {}  keys={} {}",
                                    idx.name,
                                    serde_json::to_string(&idx.keys).unwrap_or_default(),
                                    if idx.unique { "[唯一]" } else { "" }
                                ));
                            }
                        });
                    }
                    if !users.is_empty() {
                        ui.collapsing(format!("{} ({})", lang.tr("user.title"), users.len()), |ui| {
                            for u in &users {
                                ui.label(format!("• {} {} {}", u.name, lang.tr("user.roles"), u.roles.join(", ")));
                            }
                        });
                    }
                });
            });
        });

        self.filter_text = filter;
        self.sort_text = sort;
        self.limit = limit;
        self.selected_doc_text = doc_text;
        if act_run {
            self.run_query();
        }
        if act_new {
            self.selected_doc = None;
            self.selected_doc_text = "{\n  \"_id\": {}\n}".to_string();
        }
        if let Some(i) = clicked_doc {
            self.select_document(i);
        }
        if act_save {
            self.save_document();
        }
        if act_delete {
            self.delete_document();
        }
        if act_insert {
            self.insert_document();
        }
        if act_index {
            self.load_indexes();
        }
        if act_user {
            self.load_users();
        }
    }

    fn render_shell(&mut self, ctx: &Context) {
        let lang = self.lang;
        let mut shell_db = self.shell_db.clone();
        let mut shell_in = self.shell_input.clone();
        let mut shell_out = self.shell_output.clone();
        let mut act_run = false;

        TopBottomPanel::bottom("shell")
            .resizable(true)
            .default_height(170.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(lang.tr("shell.label"));
                    ui.add(TextEdit::singleline(&mut shell_db).desired_width(120.0));
                    ui.label(lang.tr("shell.db"));
                    if ui.button(lang.tr("shell.run")).clicked() {
                        act_run = true;
                    }
                    ui.label(lang.tr("shell.tip"));
                });
                ui.horizontal(|ui| {
                    ui.add(
                        TextEdit::multiline(&mut shell_in).code_editor()
                            .desired_rows(3)
                            .desired_width(f32::INFINITY),
                    );
                });
                ScrollArea::vertical().show(ui, |ui| {
                    ui.add(
                        TextEdit::multiline(&mut shell_out)
                            .desired_rows(6)
                            .interactive(false),
                    );
                });
            });

        self.shell_db = shell_db;
        self.shell_input = shell_in;
        self.shell_output = shell_out;
        if act_run {
            self.run_shell();
        }
    }

    fn render_dialog(&mut self, ctx: &Context) {
        if !self.dlg_open {
            return;
        }
        let lang = self.lang;
        let mut dlg_open = self.dlg_open;
        let connections = self.connections.clone();
        let mut dlg = self.dlg.clone();
        let mut dlg_selected = self.dlg_selected;
        let mut act_new = false;
        let mut act_save_mod = false;
        let mut act_connect = false;
        let mut act_delete = false;
        let mut act_save_new = false;
        let mut act_direct = false;
        let mut sel_clicked: Option<usize> = None;

        Window::new(lang.tr("menu.connect"))
            .open(&mut dlg_open)
            .resizable(true)
            .default_width(620.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(lang.tr("dialog.saved_connections"));
                    ui.add_space(4.0);
                    for (i, c) in connections.iter().enumerate() {
                        let selected = dlg_selected == Some(i);
                        if ui.selectable_label(selected, c.name.as_str()).clicked() {
                            sel_clicked = Some(i);
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button(lang.tr("dialog.new")).clicked() {
                        act_new = true;
                    }
                    if let Some(_i) = dlg_selected {
                        if ui.button(lang.tr("dialog.save_mod")).clicked() {
                            act_save_mod = true;
                        }
                        if ui.button(lang.tr("dialog.connect")).clicked() {
                            act_connect = true;
                        }
                        if ui.button(lang.tr("dialog.delete")).clicked() {
                            act_delete = true;
                        }
                    } else {
                        if ui.button(lang.tr("dialog.save_new")).clicked() {
                            act_save_new = true;
                        }
                        if ui.button(lang.tr("dialog.direct")).clicked() {
                            act_direct = true;
                        }
                    }
                });
                ui.separator();
                ScrollArea::vertical().show(ui, |ui| {
                    render_form_into(&mut dlg, ui, lang);
                });
            });

        if let Some(i) = sel_clicked {
            dlg_selected = Some(i);
            dlg = ConnectionForm::from_settings(&connections[i]);
        }
        if act_new {
            dlg = ConnectionForm::default();
            dlg_selected = None;
        }
        if act_save_mod {
            if let Some(i) = dlg_selected {
                let s = dlg.to_settings();
                if rubo3t_core::settings::validate(&s).is_ok() {
                    self.connections[i] = s;
                    self.save_connections();
                    self.toast(self.lang.tr("msg.save_ok"), false);
                } else {
                    self.toast(&format!("{} ({})", self.lang.tr("msg.save_failed"), self.lang.tr("msg.check_name_host")), true);
                }
            }
        }
        if act_connect {
            if let Some(i) = dlg_selected {
                let s = self.connections[i].clone();
                self.connect(s);
                dlg_open = false;
            }
        }
        if act_delete {
            if let Some(i) = dlg_selected {
                self.connections.remove(i);
                self.save_connections();
                dlg_selected = None;
            }
        }
        if act_save_new {
            let s = dlg.to_settings();
            if rubo3t_core::settings::validate(&s).is_ok() {
                self.connections.push(s);
                self.save_connections();
                self.toast(self.lang.tr("msg.save_new_ok"), false);
            } else {
                self.toast(self.lang.tr("msg.save_new_failed"), true);
            }
        }
        if act_direct {
            let s = dlg.to_settings();
            self.connect(s);
            dlg_open = false;
        }
        self.dlg = dlg;
        self.dlg_selected = dlg_selected;
        self.dlg_open = dlg_open;
    }

    fn render_toast(&mut self, ctx: &Context) {
        let lang = self.lang;
        let toast = self.toast_msg.clone();
        if let Some((msg, is_err)) = toast {
            let cleared = Window::new(lang.tr("toast.title"))
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.colored_label(
                        if is_err { Color32::RED } else { Color32::GREEN },
                        &msg,
                    );
                    ui.button(lang.tr("common.ok")).clicked()
                });
            if cleared.and_then(|r| r.inner).unwrap_or(false) {
                self.toast_msg = None;
            }
        }
    }
}

/// 在局部 `ConnectionForm` 上渲染连接表单（不借用 self，避免嵌套可变借用）。
fn render_form_into(dlg: &mut ConnectionForm, ui: &mut egui::Ui, lang: Lang) {
    ui.horizontal(|ui| {
        ui.label(lang.tr("common.name"));
        ui.add(TextEdit::singleline(&mut dlg.name).desired_width(200.0));
        ui.label(lang.tr("common.type"));
        let mut mode = dlg.mode;
        ComboBox::from_label("")
            .selected_text(match mode {
                ConnectionMode::Standalone => lang.tr("form.mode.standalone"),
                ConnectionMode::ReplicaSet => lang.tr("form.mode.replica"),
                ConnectionMode::ShardCluster => lang.tr("form.mode.shard"),
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut mode, ConnectionMode::Standalone, lang.tr("form.mode.standalone"));
                ui.selectable_value(&mut mode, ConnectionMode::ReplicaSet, lang.tr("form.mode.replica"));
                ui.selectable_value(&mut mode, ConnectionMode::ShardCluster, lang.tr("form.mode.shard"));
            });
        dlg.mode = mode;
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut dlg.use_srv, lang.tr("form.use_srv"));
        ui.checkbox(&mut dlg.direct, lang.tr("form.direct"));
    });
    ui.label(lang.tr("form.hosts"));
    ui.add(TextEdit::multiline(&mut dlg.hosts_text).code_editor().desired_rows(3));
    if dlg.mode == ConnectionMode::ReplicaSet {
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.replica_set"));
            ui.add(TextEdit::singleline(&mut dlg.replica_set));
        });
    }
    ui.separator();
    ui.heading(lang.tr("form.auth"));
    ui.checkbox(&mut dlg.auth_enabled, lang.tr("form.auth.enable"));
    if dlg.auth_enabled {
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.user"));
            ui.add(TextEdit::singleline(&mut dlg.user));
            ui.label(lang.tr("form.password"));
            ui.add(TextEdit::singleline(&mut dlg.password));
        });
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.auth_db"));
            ui.add(TextEdit::singleline(&mut dlg.auth_source).desired_width(120.0));
            ui.label(lang.tr("form.mechanism"));
            let mut mech = dlg.mechanism;
            ComboBox::from_label("")
                .selected_text(mech.tr(lang))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut mech, MechChoice::Auto, MechChoice::Auto.tr(lang));
                    ui.selectable_value(&mut mech, MechChoice::MongoDbCr, MechChoice::MongoDbCr.tr(lang));
                    ui.selectable_value(&mut mech, MechChoice::ScramSha1, MechChoice::ScramSha1.tr(lang));
                    ui.selectable_value(&mut mech, MechChoice::ScramSha256, MechChoice::ScramSha256.tr(lang));
                    ui.selectable_value(&mut mech, MechChoice::Plain, MechChoice::Plain.tr(lang));
                    ui.selectable_value(&mut mech, MechChoice::X509, MechChoice::X509.tr(lang));
                    ui.selectable_value(&mut mech, MechChoice::GssApi, MechChoice::GssApi.tr(lang));
                    ui.selectable_value(&mut mech, MechChoice::AwsIam, MechChoice::AwsIam.tr(lang));
                });
            dlg.mechanism = mech;
        });
        ui.label(lang.tr("form.mechanism.tip"));
    }
    ui.separator();
    ui.heading(lang.tr("form.ssl"));
    ui.checkbox(&mut dlg.ssl_enabled, lang.tr("form.ssl.enable"));
    if dlg.ssl_enabled {
        ui.checkbox(&mut dlg.ssl_allow_invalid, lang.tr("form.ssl.allow_invalid"));
    }
    ui.separator();
    ui.heading(lang.tr("form.ssh"));
    ui.checkbox(&mut dlg.ssh_enabled, lang.tr("form.ssh.enable"));
    if dlg.ssh_enabled {
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.ssh.bastion"));
            ui.add(TextEdit::singleline(&mut dlg.ssh_host));
            ui.label(lang.tr("common.port"));
            ui.add(DragValue::new(&mut dlg.ssh_port).range(1..=65535));
        });
        ui.horizontal(|ui| {
            ui.label(lang.tr("common.user"));
            ui.add(TextEdit::singleline(&mut dlg.ssh_user));
            let mut auth = dlg.ssh_auth;
            ComboBox::from_label(lang.tr("form.ssh.auth"))
                .selected_text(match auth {
                    SshAuthMethod::Password => lang.tr("form.ssh.password_label"),
                    SshAuthMethod::PublicKey => lang.tr("form.ssh.key"),
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut auth, SshAuthMethod::PublicKey, lang.tr("form.ssh.key"));
                    ui.selectable_value(&mut auth, SshAuthMethod::Password, lang.tr("form.ssh.password_label"));
                });
            dlg.ssh_auth = auth;
        });
        match dlg.ssh_auth {
            SshAuthMethod::PublicKey => {
                ui.horizontal(|ui| {
                    ui.label(lang.tr("form.ssh.private_key"));
                    ui.add(TextEdit::singleline(&mut dlg.ssh_key));
                });
            }
            SshAuthMethod::Password => {
                ui.horizontal(|ui| {
                    ui.label(lang.tr("form.password"));
                    ui.add(TextEdit::singleline(&mut dlg.ssh_password));
                });
            }
        }
        ui.horizontal(|ui| {
            ui.label(lang.tr("form.ssh.local_port"));
            ui.add(DragValue::new(&mut dlg.ssh_local_port).range(1..=65535));
            ui.label(lang.tr("form.ssh.remote_host"));
            ui.add(TextEdit::singleline(&mut dlg.ssh_remote_host));
            ui.label(lang.tr("form.ssh.remote_port"));
            ui.add(DragValue::new(&mut dlg.ssh_remote_port).range(1..=65535));
        });
    }
}

/// 加载系统 CJK 字体并注入 egui，解决中文显示成 tofu 方块的问题。
///
/// 优先查找 Windows 自带的微软雅黑 / SimHei，回退到 Noto Sans CJK SC / PingFang SC /
/// Hiragino Sans GB / WenQuanYi Micro Hei 等系统字体。找到后插入到 Proportional 和
/// Monospace 字体族的 fallback 链中。
fn load_cjk_fonts(ctx: &egui::Context) {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let query = fontdb::Query {
        families: &[
            fontdb::Family::Name("Microsoft YaHei"),
            fontdb::Family::Name("Microsoft YaHei UI"),
            fontdb::Family::Name("SimHei"),
            fontdb::Family::Name("Noto Sans CJK SC"),
            fontdb::Family::Name("PingFang SC"),
            fontdb::Family::Name("Source Han Sans SC"),
            fontdb::Family::Name("Hiragino Sans GB"),
            fontdb::Family::Name("WenQuanYi Micro Hei"),
            fontdb::Family::SansSerif,
        ],
        weight: fontdb::Weight::NORMAL,
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };
    let id = match db.query(&query) {
        Some(id) => id,
        None => return,
    };
    let face = match db.face(id) {
        Some(f) => f,
        None => return,
    };
    let bytes: Vec<u8> = match &face.source {
        fontdb::Source::File(path) => std::fs::read(path).unwrap_or_default(),
        fontdb::Source::Binary(blob) => blob.as_ref().as_ref().to_vec(),
        #[allow(unreachable_patterns)]
        _ => return,
    };
    if bytes.is_empty() {
        return;
    }
    let font_name = face
        .families
        .first()
        .map(|(n, _)| n.clone())
        .unwrap_or_else(|| "cjk".to_string());
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert(font_name.clone(), egui::FontData::from_owned(bytes));
    if let Some(prop) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        prop.insert(0, font_name.clone());
    }
    if let Some(mono) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        mono.push(font_name);
    }
    ctx.set_fonts(fonts);
}

// ---------------------------------------------------------------- 入口

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size(Vec2::new(1280.0, 820.0)),
        ..Default::default()
    };
    eframe::run_native(
        "Rubo3T",
        options,
        Box::new(|cc| Ok(Box::new(AppState::new(cc)))),
    )
}
