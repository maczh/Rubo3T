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
    /// 当前编辑器中的脚本文本（Robo3T 风格：`db.<coll>.find({...})`）。
    pub script: String,
    /// 脚本是否被修改过（未保存/未执行），用于标签标题前缀 "* "。
    pub dirty: bool,
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
            script: String::new(),
            dirty: false,
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
    pub script_text: String,
    pub limit: u64,
    pub skip: u64,
    /// 分页「跳过」输入框文本（独立持久，避免每次按键都触发查询）。
    pub skip_input: String,
    /// 分页「批量大小」输入框文本。
    pub batch_input: String,
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

    // 文档编辑/插入弹窗
    pub edit_doc_open: bool,
    pub edit_doc_title: String,
    pub insert_doc_open: bool,

    // 索引/用户只读弹窗
    pub indexes_dlg_open: bool,
    pub indexes_dlg_db: String,
    pub indexes_dlg_coll: String,
    pub users_dlg_open: bool,
    pub users_dlg_db: String,

    // 关于 / 测试 / 文件选择弹窗
    pub about_open: bool,
    pub test_open: bool,
    pub test_result: String,
    pub browse_open: bool,
    /// 文件选择目标：0=CA 1=PEM Cert 2=PEM Key 3=SSH Private Key
    pub browse_target: u8,
    pub browse_path: String,
    /// 连接列表「删除」确认弹窗待删除索引。
    pub confirm_remove_idx: Option<usize>,

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
            script_text: String::new(),
            limit: 50,
            skip: 0,
            skip_input: "0".to_string(),
            batch_input: "50".to_string(),
            documents: Vec::new(),
            selected_doc: None,
            selected_doc_text: String::new(),
            current_view_mode: app_settings.view_mode,
            current_query_msg: String::new(),
            show_caps: false,
            view_doc_open: false,
            view_doc_title: String::new(),
            view_doc_text: String::new(),
            edit_doc_open: false,
            edit_doc_title: String::new(),
            insert_doc_open: false,
            indexes_dlg_open: false,
            indexes_dlg_db: String::new(),
            indexes_dlg_coll: String::new(),
            users_dlg_open: false,
            users_dlg_db: String::new(),
            about_open: false,
            test_open: false,
            test_result: String::new(),
            browse_open: false,
            browse_target: 0,
            browse_path: String::new(),
            confirm_remove_idx: None,
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
                    self.tabs.push(QueryTab::new(self.next_tab_id, "", ""));
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

    /// 测试连接（§12 Connection Settings 的 Test 按钮）：建立临时 service 校验连通性，
    /// 不影响当前已激活连接；结果写入 test_result 并由测试弹窗展示。
    pub fn test_connection(&mut self, settings: ConnectionSettings) {
        match self.rt.block_on(MongoService::connect(&settings)) {
            Ok(svc) => {
                let version = svc
                    .server_info()
                    .cloned()
                    .map(|i| i.version.to_string())
                    .unwrap_or_default();
                let msg = trf(self.lang, "msg.connected", &[&settings.name, &version]);
                self.test_result = msg.clone();
                self.toast(&msg, false);
            }
            Err(e) => {
                let msg = trf(self.lang, "msg.connect_failed", &[&e.to_string()]);
                self.test_result = msg.clone();
                self.toast(&msg, true);
            }
        }
        self.test_open = true;
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
            tab.script = format!("db.{}.find({{}})", coll);
            tab.dirty = false;
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
            tab.script = self.script_text.clone();
            tab.dirty = tab.dirty || tab.script != self.script_text;
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
            self.script_text = tab.script.clone();
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
        let parsed = match parse_script(&self.script_text) {
            Ok(p) => p,
            Err(e) => {
                self.status(&format!("Failed to execute script. Error: {}", e));
                return;
            }
        };
        let db = match self.tabs.get(self.active_tab).map(|t| t.db.clone()) {
            Some(d) if !d.is_empty() => d,
            _ => {
                self.status(self.lang.tr("query.select_collection_first"));
                return;
            }
        };
        let coll = if parsed.coll.is_empty() {
            match self.current_collection() {
                Some((_, c)) => c,
                None => {
                    self.status(self.lang.tr("query.select_collection_first"));
                    return;
                }
            }
        } else {
            parsed.coll.clone()
        };
        self.limit = parsed.limit;
        self.skip = parsed.skip;
        self.execute_query(&db, &coll, &parsed.filter, &parsed.sort, self.limit, self.skip);
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.dirty = false;
            tab.limit = self.limit;
            tab.skip = self.skip;
            tab.coll = coll;
        }
    }

    /// 分页刷新：重新解析脚本的 filter/sort，但使用当前分页控件（skip/limit）而非脚本里的 .skip()/.limit()。
    pub fn run_query_paged(&mut self) {
        let parsed = match parse_script(&self.script_text) {
            Ok(p) => p,
            Err(e) => {
                self.status(&format!("Failed to execute script. Error: {}", e));
                return;
            }
        };
        let db = match self.tabs.get(self.active_tab).map(|t| t.db.clone()) {
            Some(d) if !d.is_empty() => d,
            _ => {
                self.status(self.lang.tr("query.select_collection_first"));
                return;
            }
        };
        let coll = if parsed.coll.is_empty() {
            match self.current_collection() {
                Some((_, c)) => c,
                None => {
                    self.status(self.lang.tr("query.select_collection_first"));
                    return;
                }
            }
        } else {
            parsed.coll.clone()
        };
        self.execute_query(&db, &coll, &parsed.filter, &parsed.sort, self.limit, self.skip);
    }

    /// 真正的查询执行：调用 mongo 服务，回填 documents / query_time_ms 到 active tab。
    fn execute_query(
        &mut self,
        db: &str,
        coll: &str,
        filter: &Document,
        sort: &Document,
        limit: u64,
        skip: u64,
    ) {
        if let Some(svc) = &self.service {
            let start = std::time::Instant::now();
            match self
                .rt
                .block_on(svc.find(db, coll, filter, sort, limit, skip))
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
                    self.log(&format!(
                        "{}: {} docs, {} ms",
                        self.lang.tr("query.run"),
                        self.documents.len(),
                        elapsed
                    ));
                    if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                        tab.documents = self.documents.clone();
                        tab.query_time_ms = elapsed;
                    }
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

    /// 打开数据库级别「索引」只读弹窗，并加载该库首个集合的索引。
    pub fn open_indexes_dialog(&mut self, db: &str, coll: &str) {
        self.indexes_dlg_db = db.to_string();
        self.indexes_dlg_coll = coll.to_string();
        self.indexes_dlg_open = true;
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.list_indexes(db, coll)) {
                Ok(i) => self.indexes = i,
                Err(e) => self.toast(&trf(self.lang, "index.load_failed", &[&e.to_string()]), true),
            }
        }
    }

    /// 打开数据库级别「用户」只读弹窗，并加载该库用户。
    pub fn open_users_dialog(&mut self, db: &str) {
        self.users_dlg_db = db.to_string();
        self.users_dlg_open = true;
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.list_users(db)) {
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
            self.tabs.push(QueryTab::new(self.next_tab_id, "", ""));
            self.next_tab_id += 1;
            self.active_tab = 0;
        } else if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        }
        self.sync_from_active_tab();
    }

    /// 关闭除 `idx` 之外的所有标签页。
    pub fn close_other_tabs(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        let keep = self.tabs[idx].clone();
        self.tabs = vec![keep];
        self.active_tab = 0;
        self.sync_from_active_tab();
    }

    /// 关闭 `idx` 右侧的所有标签页。
    pub fn close_tabs_to_right(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        self.tabs.truncate(idx + 1);
        if self.active_tab > idx {
            self.active_tab = idx;
        }
        self.sync_from_active_tab();
    }
}

// =====================================================================
// 脚本解析器（手写，不依赖 regex crate）
//
// 支持形如：
//   db.users.find({})
//   db.getCollection('users').find({a:1}).sort({age:-1}).limit(10).skip(5)
// 解析出集合名、filter、sort、limit、skip。
// =====================================================================

/// `parse_script` 的解析结果。
struct ParsedQuery {
    coll: String,
    filter: Document,
    sort: Document,
    limit: u64,
    skip: u64,
}

/// 解析 Robo3T 风格查询脚本。失败时返回人类可读错误（供状态栏展示）。
fn parse_script(script: &str) -> Result<ParsedQuery, String> {
    let s = script.trim();
    let find_pos = s
        .find(".find(")
        .ok_or_else(|| "找不到 find() 调用".to_string())?;
    let coll_expr = s[..find_pos].trim();
    let coll = parse_collection_name(coll_expr)?;
    let open = s[find_pos..]
        .find('(')
        .map(|p| find_pos + p)
        .ok_or_else(|| "find() 括号不匹配".to_string())?;
    let close = find_matching_paren(s, open)
        .ok_or_else(|| "find() 括号不匹配".to_string())?;
    let args = s[open + 1..close].trim();
    // 顶层按逗号拆分，取第一段作为 filter（忽略可选的 projection 第二段）
    let filter_str = split_top_level(args, ',')
        .into_iter()
        .next()
        .unwrap_or_default()
        .trim()
        .to_string();
    let filter = parse_doc(&filter_str)?;

    let mut rest = &s[close + 1..];
    let mut sort = Document::new();
    let mut limit = 50u64;
    let mut skip = 0u64;

    loop {
        let t = rest.trim_start();
        if let Some(after) = t.strip_prefix(".sort(") {
            // after 形如 "{a:-1}).limit(5)"
            let close_brace = find_matching_brace(after)
                .ok_or_else(|| "sort() 括号不匹配".to_string())?;
            let sort_str = after[1..close_brace].trim();
            sort = parse_doc(sort_str)?;
            rest = &after[close_brace + 1..];
        } else if let Some(after) = t.strip_prefix(".limit(") {
            // after 形如 "10).skip(5)"
            let c = after
                .find(')')
                .ok_or_else(|| "limit() 缺少右括号".to_string())?;
            limit = parse_u64(&after[..c])?;
            rest = &after[c + 1..];
        } else if let Some(after) = t.strip_prefix(".skip(") {
            let c = after
                .find(')')
                .ok_or_else(|| "skip() 缺少右括号".to_string())?;
            skip = parse_u64(&after[..c])?;
            rest = &after[c + 1..];
        } else {
            break;
        }
    }

    Ok(ParsedQuery {
        coll,
        filter,
        sort,
        limit,
        skip,
    })
}

/// 从集合表达式中解析集合名：`db.getCollection('x')` 或 `db.x`。
fn parse_collection_name(expr: &str) -> Result<String, String> {
    let expr = expr.trim();
    if let Some(pos) = expr.find("getCollection(") {
        let start = pos + "getCollection(".len();
        let tail = &expr[start..];
        let quote_pos = tail
            .find(|c| c == '\'' || c == '"')
            .ok_or_else(|| "无法解析 getCollection 参数".to_string())?;
        let quote = tail[quote_pos..].chars().next().unwrap();
        let inner = &tail[quote_pos + 1..];
        let end = inner
            .find(quote)
            .ok_or_else(|| "getCollection 字符串未闭合".to_string())?;
        return Ok(inner[..end].to_string());
    }
    if let Some(stripped) = expr.strip_prefix("db.") {
        let name: String = stripped
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            return Err("无法解析集合名".to_string());
        }
        return Ok(name);
    }
    Err("无法解析集合表达式".to_string())
}

/// 解析一段文档文本（JSON 对象）为 BSON Document。
fn parse_doc(s: &str) -> Result<Document, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(Document::new());
    }
    bson_ext::parse_filter_text(s).map_err(|e| format!("解析文档失败: {}", e))
}

/// 解析无符号整数（limit/skip 用）。
fn parse_u64(s: &str) -> Result<u64, String> {
    let s = s.trim();
    s.parse::<u64>()
        .map_err(|e| format!("无法解析数字 '{}': {}", s, e))
}

/// 在顶层（忽略字符串/括号内部）按分隔符拆分。
fn split_top_level(s: &str, sep: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut quote = b' ';
    let mut cur = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_str {
            cur.push(c);
            if c == '\\' && i + 1 < bytes.len() {
                cur.push(bytes[i + 1] as char);
                i += 2;
                continue;
            }
            if (c as u8) == quote {
                in_str = false;
            }
        } else {
            match c {
                '\'' | '"' => {
                    in_str = true;
                    quote = c as u8;
                    cur.push(c);
                }
                '{' | '(' | '[' => {
                    depth += 1;
                    cur.push(c);
                }
                '}' | ')' | ']' => {
                    depth -= 1;
                    cur.push(c);
                }
                _ if c == sep && depth == 0 => {
                    parts.push(std::mem::take(&mut cur));
                }
                _ => cur.push(c),
            }
        }
        i += 1;
    }
    if !cur.trim().is_empty() {
        parts.push(cur);
    }
    parts
}

/// 从 `s[open]` 处的 '(' 开始，找到匹配的右括号位置（支持嵌套与字符串）。
fn find_matching_paren(s: &str, open: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.get(open) != Some(&b'(') {
        return None;
    }
    let mut depth = 0i32;
    let mut in_str = false;
    let mut quote = b' ';
    let mut i = open;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == quote {
                in_str = false;
            }
        } else if c == b'\'' || c == b'"' {
            in_str = true;
            quote = c;
        } else if c == b'(' {
            depth += 1;
        } else if c == b')' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}

/// 从以 '{' 开头的字符串中找到匹配的 '}' 位置（支持嵌套与字符串）。
fn find_matching_brace(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.first() != Some(&b'{') {
        return None;
    }
    let mut depth = 0i32;
    let mut in_str = false;
    let mut quote = b' ';
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            if c == b'\\' {
                i += 2;
                continue;
            }
            if c == quote {
                in_str = false;
            }
        } else if c == b'\'' || c == b'"' {
            in_str = true;
            quote = c;
        } else if c == b'{' {
            depth += 1;
        } else if c == b'}' {
            depth -= 1;
            if depth == 0 {
                return Some(i);
            }
        }
        i += 1;
    }
    None
}
