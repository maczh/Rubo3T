//! 应用状态与业务逻辑。
//!
//! 将状态从 UI 渲染层分离，避免 egui 闭包借用冲突。

use std::collections::HashMap;
use std::path::PathBuf;

use rubo3t_core::bson::doc;
use rubo3t_core::bson_ext;
use rubo3t_core::domain::{CollectionInfo, DatabaseInfo, IndexInfo, ServerInfo, UserInfo};
use rubo3t_core::settings::ConnectionSettings;
use rubo3t_core::Document;
use rubo3t_mongo::MongoService;
use rubo3t_ssh::Tunnel;

use crate::form::ConnectionForm;
use crate::i18n::{trf, Lang};

/// 结果视图模式（对应 Robo3T 的 Table/Tree/Text/Custom）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ViewMode {
    #[default]
    Table,
    Tree,
    Text,
    Custom,
}

impl ViewMode {
    pub fn label(self, lang: Lang) -> &'static str {
        match self {
            ViewMode::Table => lang.tr("view.table"),
            ViewMode::Tree => lang.tr("view.tree"),
            ViewMode::Text => lang.tr("view.text"),
            ViewMode::Custom => lang.tr("view.custom"),
        }
    }
}

/// 单个查询标签页状态。
#[derive(Clone)]
pub struct QueryTab {
    pub id: usize,
    pub title: String,
    pub db: String,
    pub coll: String,
    pub filter: String,
    pub sort: String,
    pub limit: u64,
    pub skip: u64,
    pub documents: Vec<Document>,
    pub selected_doc: Option<usize>,
    pub doc_text: String,
    pub view_mode: ViewMode,
    pub query_time_ms: u64,
    pub query_msg: String,
}

impl QueryTab {
    pub fn new(id: usize, db: impl Into<String>, coll: impl Into<String>) -> Self {
        let db = db.into();
        let coll = coll.into();
        let title = format!("{} / {}", db, coll);
        Self {
            id,
            title,
            db,
            coll,
            filter: "{}".to_string(),
            sort: "{}".to_string(),
            limit: 50,
            skip: 0,
            documents: Vec::new(),
            selected_doc: None,
            doc_text: String::new(),
            view_mode: ViewMode::Table,
            query_time_ms: 0,
            query_msg: String::new(),
        }
    }
}

/// 连接弹窗当前所处子界面。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ConnDlgMode {
    /// 连接列表（MongoDB Connections）。
    #[default]
    List,
    /// 单个连接设置（Connection Settings）。
    Edit,
}

/// 应用全局状态。
pub struct AppState {
    pub rt: tokio::runtime::Runtime,
    pub connections_path: PathBuf,
    pub settings_path: PathBuf,
    pub connections: Vec<ConnectionSettings>,
    pub lang: Lang,

    // 设置持久化
    pub app_settings: AppSettings,

    // 连接管理弹窗
    pub conn_dlg_open: bool,
    pub conn_dlg: ConnectionForm,
    pub conn_dlg_selected: Option<usize>,
    pub conn_dlg_mode: ConnDlgMode,

    // 当前连接
    pub service: Option<MongoService>,
    pub tunnel: Option<Tunnel>,
    pub server_info: Option<ServerInfo>,
    pub active_name: String,

    // Explorer
    pub databases: Vec<DatabaseInfo>,
    pub collections: HashMap<String, Vec<CollectionInfo>>,
    pub expanded_dbs: HashMap<String, bool>,

    // 工作区标签页
    pub tabs: Vec<QueryTab>,
    pub active_tab: usize,
    pub next_tab_id: usize,

    // 用于渲染的「当前 tab 影子字段」，每帧从 active tab 同步。
    pub filter_text: String,
    pub sort_text: String,
    pub limit: u64,
    pub skip: u64,
    pub documents: Vec<Document>,
    pub selected_doc: Option<usize>,
    pub selected_doc_text: String,
    pub current_view_mode: ViewMode,
    pub current_query_msg: String,

    // 能力面板
    pub show_caps: bool,

    // 文档查看弹窗
    pub view_doc_open: bool,
    pub view_doc_title: String,
    pub view_doc_text: String,

    // 右键菜单状态
    pub context_doc_idx: Option<usize>,
    pub context_menu_pos: Option<egui::Pos2>,

    // 索引/用户
    pub indexes: Vec<IndexInfo>,
    pub users: Vec<UserInfo>,

    // 底部 Shell
    pub shell_input: String,
    pub shell_output: String,
    pub shell_db: String,

    // 日志
    pub logs: Vec<String>,
    pub show_logs: bool,

    // Toast
    pub toast_msg: Option<(String, bool)>,

    // 选项菜单
    pub options_menu_open: bool,

    // 图标是否已在 egui 上下文注册（每帧 update 开头注册一次）
    pub icon_registered: bool,
}

#[derive(serde::Serialize, serde::Deserialize, Default, Clone)]
pub struct AppSettings {
    #[serde(default)]
    pub lang: Lang,
    #[serde(default)]
    pub view_mode: ViewMode,
    #[serde(default)]
    pub auto_expand: bool,
    #[serde(default)]
    pub line_numbers: bool,
    #[serde(default)]
    pub auto_exec: bool,
    #[serde(default)]
    pub load_mongorc: bool,
    #[serde(default)]
    pub disable_conn_shortcuts: bool,
}

impl serde::Serialize for ViewMode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(match self {
            ViewMode::Table => "table",
            ViewMode::Tree => "tree",
            ViewMode::Text => "text",
            ViewMode::Custom => "custom",
        })
    }
}

impl<'de> serde::Deserialize<'de> for ViewMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "tree" => ViewMode::Tree,
            "text" => ViewMode::Text,
            "custom" => ViewMode::Custom,
            _ => ViewMode::Table,
        })
    }
}

impl AppState {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let config_dir = dirs::config_dir()
            .map(|p| p.join("Rubo3T"))
            .unwrap_or_else(|| PathBuf::from("Rubo3T"));
        let _ = std::fs::create_dir_all(&config_dir);
        let connections_path = config_dir.join("connections.json");
        let settings_path = config_dir.join("settings.json");

        let app_settings = Self::load_settings(&settings_path);
        let connections = Self::load_connections(&connections_path);
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("无法创建 tokio 运行时");

        crate::font::load_cjk_fonts(&cc.egui_ctx);

        let mut state = Self {
            rt,
            connections_path,
            settings_path,
            connections,
            lang: app_settings.lang,
            app_settings: app_settings.clone(),
            conn_dlg_open: false,
            conn_dlg: ConnectionForm::default(),
            conn_dlg_selected: None,
            conn_dlg_mode: ConnDlgMode::List,
            service: None,
            tunnel: None,
            server_info: None,
            active_name: String::new(),
            databases: Vec::new(),
            collections: HashMap::new(),
            expanded_dbs: HashMap::new(),
            tabs: Vec::new(),
            active_tab: 0,
            next_tab_id: 1,
            filter_text: "{}".to_string(),
            sort_text: "{}".to_string(),
            limit: 50,
            skip: 0,
            documents: Vec::new(),
            selected_doc: None,
            selected_doc_text: String::new(),
            current_view_mode: app_settings.view_mode,
            current_query_msg: String::new(),
            show_caps: false,
            view_doc_open: false,
            view_doc_title: String::new(),
            view_doc_text: String::new(),
            context_doc_idx: None,
            context_menu_pos: None,
            indexes: Vec::new(),
            users: Vec::new(),
            shell_input: "{\"buildInfo\": 1}".to_string(),
            shell_output: String::new(),
            shell_db: "admin".to_string(),
            logs: Vec::new(),
            show_logs: true,
            toast_msg: None,
            options_menu_open: false,
            icon_registered: false,
        };
        state.log("Rubo3T 已启动");
        state
    }

    // ----- 持久化 -----

    pub fn load_connections(path: &PathBuf) -> Vec<ConnectionSettings> {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Vec::new(),
        }
    }

    pub fn save_connections(&self) {
        if let Ok(s) = serde_json::to_string_pretty(&self.connections) {
            let _ = std::fs::write(&self.connections_path, s);
        }
    }

    pub fn load_settings(path: &PathBuf) -> AppSettings {
        match std::fs::read_to_string(path) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => AppSettings::default(),
        }
    }

    pub fn save_settings(&self) {
        let mut s = self.app_settings.clone();
        s.lang = self.lang;
        s.view_mode = self.current_view_mode;
        if let Ok(json) = serde_json::to_string_pretty(&s) {
            let _ = std::fs::write(&self.settings_path, json);
        }
    }

    // ----- 日志 -----

    pub fn log(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        self.logs.push(msg);
        if self.logs.len() > 200 {
            self.logs.remove(0);
        }
    }

    // ----- 连接管理 -----

    pub fn connect(&mut self, settings: ConnectionSettings) {
        self.tunnel = None;
        if settings.ssh.enabled {
            match Tunnel::start(&settings.ssh) {
                Ok(t) => self.tunnel = Some(t),
                Err(e) => {
                    self.toast(&trf(self.lang, "msg.ssh_failed", &[&e.to_string()]), true);
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
                self.log(&trf(
                    self.lang,
                    "msg.connected",
                    &[&settings.name, &version_str],
                ));
                self.load_databases();
                // 打开第一个数据库的欢迎标签页
                if self.tabs.is_empty() {
                    self.tabs.push(QueryTab::new(self.next_tab_id, "", self.lang.tr("welcome.title")));
                    self.active_tab = 0;
                }
            }
            Err(e) => {
                self.tunnel = None;
                self.toast(&trf(self.lang, "msg.connect_failed", &[&e.to_string()]), true);
                self.status(&trf(self.lang, "msg.connect_failed", &[&e.to_string()]));
                self.log(&trf(self.lang, "msg.connect_failed", &[&e.to_string()]));
            }
        }
    }

    pub fn disconnect(&mut self) {
        self.service = None;
        self.tunnel = None;
        self.server_info = None;
        self.databases.clear();
        self.collections.clear();
        self.expanded_dbs.clear();
        self.tabs.clear();
        self.active_tab = 0;
        self.documents.clear();
        self.selected_doc = None;
        self.selected_doc_text.clear();
        self.indexes.clear();
        self.users.clear();
        self.status(self.lang.tr("msg.disconnected"));
        self.log(self.lang.tr("msg.disconnected"));
    }

    pub fn status(&mut self, msg: &str) {
        self.current_query_msg = msg.to_string();
    }

    pub fn toast(&mut self, msg: &str, is_err: bool) {
        self.toast_msg = Some((msg.to_string(), is_err));
    }

    // ----- 数据加载 -----

    pub fn load_databases(&mut self) {
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
                Err(e) => self.toast(&trf(self.lang, "msg.load_dbs_failed", &[&e.to_string()]), true),
            }
        }
    }

    pub fn load_collections(&mut self, db: &str) {
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.list_collections(db)) {
                Ok(cols) => {
                    self.collections.insert(db.to_string(), cols);
                }
                Err(e) => self.toast(&trf(self.lang, "msg.load_colls_failed", &[&e.to_string()]), true),
            }
        }
    }

    /// 选中集合并打开/激活对应标签页。
    pub fn select_collection(&mut self, db: &str, coll: &str) {
        if let Some(pos) = self.tabs.iter().position(|t| t.db == db && t.coll == coll) {
            self.active_tab = pos;
            self.sync_from_active_tab();
        } else {
            let id = self.next_tab_id;
            self.next_tab_id += 1;
            let mut tab = QueryTab::new(id, db, coll);
            tab.view_mode = self.current_view_mode;
            self.tabs.push(tab);
            self.active_tab = self.tabs.len() - 1;
            self.sync_from_active_tab();
            self.run_query();
        }
    }

    /// 把当前工作区字段写回 active tab。
    pub fn sync_to_active_tab(&mut self) {
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.filter = self.filter_text.clone();
            tab.sort = self.sort_text.clone();
            tab.limit = self.limit;
            tab.skip = self.skip;
            tab.documents = self.documents.clone();
            tab.selected_doc = self.selected_doc;
            tab.doc_text = self.selected_doc_text.clone();
            tab.view_mode = self.current_view_mode;
            tab.query_msg = self.current_query_msg.clone();
        }
    }

    /// 从 active tab 同步到当前工作区字段。
    pub fn sync_from_active_tab(&mut self) {
        if let Some(tab) = self.tabs.get(self.active_tab) {
            self.filter_text = tab.filter.clone();
            self.sort_text = tab.sort.clone();
            self.limit = tab.limit;
            self.skip = tab.skip;
            self.documents = tab.documents.clone();
            self.selected_doc = tab.selected_doc;
            self.selected_doc_text = tab.doc_text.clone();
            self.current_view_mode = tab.view_mode;
            self.current_query_msg = tab.query_msg.clone();
        }
    }

    pub fn run_query(&mut self) {
        let (db, coll) = match self.current_collection() {
            Some(x) => x,
            None => {
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
            let start = std::time::Instant::now();
            match self
                .rt
                .block_on(svc.find(&db, &coll, &filter, &sort, self.limit, self.skip))
            {
                Ok(docs) => {
                    self.documents = docs;
                    self.selected_doc = None;
                    self.selected_doc_text.clear();
                    let elapsed = start.elapsed().as_millis() as u64;
                    self.status(&trf(
                        self.lang,
                        "msg.query_returned",
                        &[&self.documents.len().to_string()],
                    ));
                    self.log(&format!("{}: {} docs, {} ms", self.lang.tr("query.run"), self.documents.len(), elapsed));
                }
                Err(e) => self.status(&trf(self.lang, "msg.query_failed", &[&e.to_string()])),
            }
        }
        self.sync_to_active_tab();
    }

    pub fn current_collection(&self) -> Option<(String, String)> {
        self.tabs.get(self.active_tab).and_then(|t| {
            if t.db.is_empty() || t.coll.is_empty() {
                None
            } else {
                Some((t.db.clone(), t.coll.clone()))
            }
        })
    }

    pub fn select_document(&mut self, i: usize) {
        if let Some(doc) = self.documents.get(i) {
            match bson_ext::document_to_text(doc) {
                Ok(t) => self.selected_doc_text = t,
                Err(e) => self.toast(&trf(self.lang, "msg.doc_convert_failed", &[&e.to_string()]), true),
            }
            self.selected_doc = Some(i);
        }
    }

    pub fn open_view_document(&mut self, i: usize) {
        if let Some(doc) = self.documents.get(i) {
            match bson_ext::document_to_text(doc) {
                Ok(t) => {
                    self.view_doc_title = format!("{} # {}", self.lang.tr("doc.view"), i + 1);
                    self.view_doc_text = t;
                    self.view_doc_open = true;
                }
                Err(e) => self.toast(&trf(self.lang, "msg.doc_convert_failed", &[&e.to_string()]), true),
            }
        }
    }

    pub fn save_document(&mut self) {
        let (db, coll) = match self.current_collection() {
            Some(x) => x,
            None => {
                self.toast(self.lang.tr("msg.select_collection_first"), true);
                return;
            }
        };
        let new_doc = match bson_ext::parse_document_text(&self.selected_doc_text) {
            Ok(d) => d,
            Err(e) => {
                self.toast(&trf(self.lang, "msg.parse.doc", &[&e.to_string()]), true);
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
                Err(e) => self.toast(&trf(self.lang, "msg.save_doc_failed", &[&e.to_string()]), true),
            }
        }
    }

    pub fn delete_document(&mut self, idx: Option<usize>) {
        let (db, coll) = match self.current_collection() {
            Some(x) => x,
            None => {
                self.toast(self.lang.tr("msg.select_collection_first"), true);
                return;
            }
        };
        let id = match idx
            .or(self.selected_doc)
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
                Err(e) => self.toast(&trf(self.lang, "msg.delete_doc_failed", &[&e.to_string()]), true),
            }
        }
    }

    pub fn insert_document(&mut self) {
        let (db, coll) = match self.current_collection() {
            Some(x) => x,
            None => {
                self.toast(self.lang.tr("msg.select_collection_first"), true);
                return;
            }
        };
        let new_doc = match bson_ext::parse_document_text(&self.selected_doc_text) {
            Ok(d) => d,
            Err(e) => {
                self.toast(&trf(self.lang, "msg.parse.doc", &[&e.to_string()]), true);
                return;
            }
        };
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.insert_one(&db, &coll, &new_doc)) {
                Ok(()) => {
                    self.toast(self.lang.tr("doc.inserted"), false);
                    self.run_query();
                }
                Err(e) => self.toast(&trf(self.lang, "msg.insert_doc_failed", &[&e.to_string()]), true),
            }
        }
    }

    pub fn run_shell(&mut self) {
        let cmd = match bson_ext::parse_document_text(&self.shell_input) {
            Ok(d) => d,
            Err(e) => {
                self.shell_output = trf(self.lang, "msg.parse.doc", &[&e.to_string()]);
                return;
            }
        };
        let db = if self.shell_db.is_empty() {
            self.current_collection().map(|(d, _)| d).unwrap_or_else(|| "admin".into())
        } else {
            self.shell_db.clone()
        };
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.run_command(&db, &cmd)) {
                Ok(res) => match bson_ext::document_to_text(&res) {
                    Ok(t) => self.shell_output = t,
                    Err(e) => self.shell_output = trf(self.lang, "msg.doc_convert_failed", &[&e.to_string()]),
                },
                Err(e) => self.shell_output = trf(self.lang, "msg.query_failed", &[&e.to_string()]),
            }
        }
    }

    pub fn load_indexes(&mut self) {
        let (db, coll) = match self.current_collection() {
            Some(x) => x,
            None => {
                self.toast(self.lang.tr("msg.select_collection_first"), true);
                return;
            }
        };
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.list_indexes(&db, &coll)) {
                Ok(i) => self.indexes = i,
                Err(e) => self.toast(&trf(self.lang, "index.load_failed", &[&e.to_string()]), true),
            }
        }
    }

    pub fn load_users(&mut self) {
        let db = match self.current_collection().map(|(d, _)| d) {
            Some(d) => d,
            None => {
                self.toast(self.lang.tr("msg.select_collection_first"), true);
                return;
            }
        };
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.list_users(&db)) {
                Ok(u) => self.users = u,
                Err(e) => self.toast(&trf(self.lang, "user.load_failed", &[&e.to_string()]), true),
            }
        }
    }

    pub fn close_tab(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        self.tabs.remove(idx);
        if self.tabs.is_empty() {
            self.tabs.push(QueryTab::new(self.next_tab_id, "", self.lang.tr("welcome.title")));
            self.next_tab_id += 1;
            self.active_tab = 0;
        } else if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        self.sync_from_active_tab();
    }
}
