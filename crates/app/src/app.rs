//! 应用状态与业务逻辑。
//!
//! 将状态从 UI 渲染层分离，避免 egui 闭包借用冲突。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use chrono::Timelike;
use rubo3t_core::bson::{doc, Bson};
use rubo3t_core::bson_ext;
use rubo3t_core::domain::{CollectionInfo, DatabaseInfo, IndexInfo, ServerInfo, UserInfo};
use rubo3t_core::settings::ConnectionSettings;
use rubo3t_core::Document;
use rubo3t_mongo::MongoService;
use rubo3t_ssh::{SshLogger, Tunnel};

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

/// 点击左侧集合名打开标签时，默认查询的排序方式。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DefaultSort {
    /// 不附加排序（MongoDB 自然顺序）。
    None,
    /// 按 `_id` 升序。
    IdAsc,
    /// 按 `_id` 降序（Robo3T 习惯：最新文档在前）。
    #[default]
    IdDesc,
}

impl DefaultSort {
    /// 对应查询脚本中追加的 `.sort(...)` 片段（空串表示不加）。
    pub fn sort_script_suffix(self) -> &'static str {
        match self {
            DefaultSort::None => "",
            DefaultSort::IdAsc => ".sort({\"_id\": 1})",
            DefaultSort::IdDesc => ".sort({\"_id\": -1})",
        }
    }

    /// 对应标签页 `sort` 文本（空对象表示无）。
    pub fn sort_text(self) -> &'static str {
        match self {
            DefaultSort::None => "{}",
            DefaultSort::IdAsc => "{\"_id\": 1}",
            DefaultSort::IdDesc => "{\"_id\": -1}",
        }
    }

    pub fn label(self, lang: Lang) -> &'static str {
        match self {
            DefaultSort::None => lang.tr("sort.none"),
            DefaultSort::IdAsc => lang.tr("sort.id_asc"),
            DefaultSort::IdDesc => lang.tr("sort.id_desc"),
        }
    }
}

impl serde::Serialize for DefaultSort {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(match self {
            DefaultSort::None => "none",
            DefaultSort::IdAsc => "id_asc",
            DefaultSort::IdDesc => "id_desc",
        })
    }
}

impl<'de> serde::Deserialize<'de> for DefaultSort {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.as_str() {
            "id_asc" => DefaultSort::IdAsc,
            "id_desc" => DefaultSort::IdDesc,
            _ => DefaultSort::None,
        })
    }
}

/// 查询标签页的脚本执行模式：Find（JS 风格脚本）/ Aggregate（JSON 管道数组）。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum QueryMode {
    #[default]
    Find,
    Aggregate,
}

/// 资源管理器右键菜单待确认动作（删除集合 / 删除数据库）。
#[derive(Clone)]
pub enum ExplorerAction {
    DropCollection(String, String), // (db, coll)
    DropDatabase(String),           // (db)
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
    /// 查询执行模式：Find / Aggregate。
    pub query_mode: QueryMode,
    /// 聚合管道文本（JSON 数组），仅在 Aggregate 模式下使用。
    pub agg_text: String,
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
            query_mode: QueryMode::default(),
            agg_text: String::new(),
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
    /// 查询执行模式：Find / Aggregate。
    pub query_mode: QueryMode,
    /// 聚合管道文本（JSON 数组），仅在 Aggregate 模式下使用。
    pub agg_text: String,
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

    // 文档导入/导出弹窗
    pub export_open: bool,
    pub export_path: String,
    pub export_doc_index: Option<usize>,
    pub import_open: bool,
    pub import_path: String,

    // 文档编辑/插入弹窗
    pub edit_doc_open: bool,
    pub edit_doc_title: String,
    pub insert_doc_open: bool,

    // 索引/用户只读弹窗
    pub indexes_dlg_open: bool,
    pub indexes_dlg_db: String,
    pub indexes_dlg_coll: String,
    pub index_create_open: bool,
    pub index_create_field: String,
    pub index_create_name: String,
    pub index_create_unique: bool,
    pub user_create_open: bool,
    pub user_create_name: String,
    pub user_create_pwd: String,
    pub user_create_roles: String,
    pub user_edit_open: bool,
    pub user_edit_name: String,
    pub user_edit_pwd: String,
    pub user_edit_roles: String,
    pub index_edit_open: bool,
    pub index_edit_old_name: String,
    pub index_edit_field: String,
    pub index_edit_name: String,
    pub index_edit_unique: bool,
    pub users_dlg_open: bool,
    pub users_dlg_db: String,

    // 资源管理器右键菜单：新建集合 / 删除确认 / 重命名集合
    pub create_coll_db: Option<String>,
    pub create_coll_name: String,
    pub confirm_action: Option<ExplorerAction>,
    pub rename_coll: Option<(String, String)>, // (db, old_name)
    pub rename_coll_new: String,

    // 关于 / 测试 / 文件选择弹窗
    pub about_open: bool,
    // 版本能力弹窗
    pub version_caps_open: bool,

    // 服务器状态弹窗
    pub server_status_open: bool,
    pub server_status_doc: Option<Document>,
    pub host_info_doc: Option<Document>,
    pub repl_status_doc: Option<Document>,
    pub cmd_opts_doc: Option<Document>,
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
    /// SSH 隧道调试日志的共享缓冲（由隧道回调写入，每帧抽到 logs 中展示）。
    pub ssh_log_sink: Arc<Mutex<Vec<String>>>,

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
    /// 点击集合名打开标签时的默认排序方式。
    #[serde(default)]
    pub default_sort: DefaultSort,
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
            query_mode: QueryMode::default(),
            agg_text: String::new(),
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
            export_open: false,
            export_path: String::new(),
            export_doc_index: None,
            import_open: false,
            import_path: String::new(),
            edit_doc_open: false,
            edit_doc_title: String::new(),
            insert_doc_open: false,
            indexes_dlg_open: false,
            indexes_dlg_db: String::new(),
            indexes_dlg_coll: String::new(),
            index_create_open: false,
            index_create_field: String::new(),
            index_create_name: String::new(),
            index_create_unique: false,
            user_create_open: false,
            user_create_name: String::new(),
            user_create_pwd: String::new(),
            user_create_roles: String::new(),
            user_edit_open: false,
            user_edit_name: String::new(),
            user_edit_pwd: String::new(),
            user_edit_roles: String::new(),
            index_edit_open: false,
            index_edit_old_name: String::new(),
            index_edit_field: String::new(),
            index_edit_name: String::new(),
            index_edit_unique: false,
            users_dlg_open: false,
            users_dlg_db: String::new(),
            create_coll_db: None,
            create_coll_name: String::new(),
            confirm_action: None,
            rename_coll: None,
            rename_coll_new: String::new(),
            about_open: false,
            version_caps_open: false,
            server_status_open: false,
            server_status_doc: None,
            host_info_doc: None,
            repl_status_doc: None,
            cmd_opts_doc: None,
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
            ssh_log_sink: Arc::new(Mutex::new(Vec::new())),
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
        self.push_log("Info", msg.into());
    }

    fn log_error(&mut self, msg: impl Into<String>) {
        self.push_log("Error", msg.into());
    }

    fn push_log(&mut self, level: &str, msg: String) {
        let now = chrono::Local::now();
        let period = if now.hour() < 12 { "上午" } else { "下午" };
        let timestamp = format!("{} {}", now.format("%I:%M:%S"), period);
        self.logs.push(format!("{timestamp}\t{level}: {msg}"));
        if self.logs.len() > 200 {
            self.logs.remove(0);
        }
    }

    fn log_query_script(&mut self, script: &str) {
        let script = script.replace('\r', "\\r").replace('\n', "\\n");
        self.log(script);
    }

    /// 构造一个 SSH 隧道调试日志回调：隧道把每行日志写入共享缓冲 `ssh_log_sink`，
    /// 再由 `drain_ssh_logs` 每帧抽到 logs 展示。
    fn ssh_logger(&self) -> Option<SshLogger> {
        let sink = self.ssh_log_sink.clone();
        Some(Arc::new(move |msg: &str| {
            if let Ok(mut v) = sink.lock() {
                v.push(msg.to_string());
            }
        }))
    }

    /// 把 SSH 隧道共享缓冲里的调试日志抽到主日志框（每帧调用一次即可）。
    pub(crate) fn drain_ssh_logs(&mut self) {
        let mut drained: Vec<String> = Vec::new();
        if let Ok(mut v) = self.ssh_log_sink.lock() {
            if !v.is_empty() {
                std::mem::swap(&mut drained, &mut *v);
            }
        }
        // 先释放锁，再写日志（self.log 需要可变借用 self）。
        for m in drained {
            self.log(format!("[SSH] {m}"));
        }
    }

    // ----- 连接管理 -----

    pub fn connect(&mut self, mut settings: ConnectionSettings) {
        self.tunnel = None;
        if settings.ssh.enabled {
            // 隧道转发目标 = MongoDB 真实服务器地址（从跳板机可达），即连接设置中的 hosts[0]。
            // 本地 127.0.0.1:local_port 仅作入口，由 build_uri 改写为转发端口。
            let (fw_host, fw_port) = match settings.hosts.first() {
                Some(h) => (h.host.clone(), if h.port == 0 { 27017 } else { h.port }),
                None => (settings.ssh.remote_host.clone(), settings.ssh.remote_port),
            };
            match Tunnel::start_with_logger(&settings.ssh, fw_host, fw_port, self.ssh_logger()) {
                Ok(t) => self.tunnel = Some(t),
                Err(e) => {
                    self.drain_ssh_logs();
                    self.toast(&trf(self.lang, "msg.ssh_failed", &[&e.to_string()]), true);
                    return;
                }
            }
        }
        if let Some(t) = &self.tunnel {
            settings.ssh.local_port = t.local_port();
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
    pub fn test_connection(&mut self, mut settings: ConnectionSettings) {
        // 测试同样需要先建立 SSH 隧道（若启用），否则 URI 指向的本地转发端口无人监听。
        // 隧道为函数内临时变量（下划线前缀避免未使用告警），结束时随 Drop 自动停止。
        let _test_tunnel = if settings.ssh.enabled {
            // 同 connect：转发目标取 MongoDB 真实服务器地址 hosts[0]。
            let (fw_host, fw_port) = match settings.hosts.first() {
                Some(h) => (h.host.clone(), if h.port == 0 { 27017 } else { h.port }),
                None => (settings.ssh.remote_host.clone(), settings.ssh.remote_port),
            };
            match Tunnel::start_with_logger(&settings.ssh, fw_host, fw_port, self.ssh_logger()) {
                Ok(t) => {
                    settings.ssh.local_port = t.local_port();
                    Some(t)
                }
                Err(e) => {
                    self.drain_ssh_logs();
                    let msg = trf(self.lang, "msg.ssh_failed", &[&e.to_string()]);
                    self.test_result = msg.clone();
                    self.toast(&msg, true);
                    self.test_open = true;
                    return;
                }
            }
        } else {
            None
        };
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
        self.drain_ssh_logs();
        // test_tunnel Drop 在此触发，自动关闭临时隧道。
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

    /// 拉取服务器状态/主机/副本集/启动参数，缓存到 state 供对话框渲染。
    pub fn load_server_status(&mut self) {
        // serverStatus 在任意库都可执行；优先用当前集合所在库，否则 admin。
        let db = self
            .current_collection()
            .map(|(d, _)| d)
            .unwrap_or_else(|| "admin".to_string());
        if let Some(svc) = &self.service {
            self.server_status_doc = self.rt.block_on(svc.server_status(&db)).ok();
            self.host_info_doc = self.rt.block_on(svc.host_info(&db)).ok();
            self.repl_status_doc = self.rt.block_on(svc.repl_set_status(&db)).ok();
            self.cmd_opts_doc = self.rt.block_on(svc.cmd_line_opts(&db)).ok();
            if self.server_status_doc.is_none() {
                self.toast(self.lang.tr("server_status.failed"), true);
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
            // 按「选项 → 默认排序」生成默认查询脚本
            let sort = self.app_settings.default_sort;
            tab.script = format!("db.{}.find({{}}){}", coll, sort.sort_script_suffix());
            tab.sort = sort.sort_text().to_string();
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
            tab.query_mode = self.query_mode;
            tab.agg_text = self.agg_text.clone();
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
            self.query_mode = tab.query_mode;
            self.agg_text = tab.agg_text.clone();
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
                self.report_execution_error(format!("Failed to execute script. Error: {}", e));
                return;
            }
        };
        let db = match self.tabs.get(self.active_tab).map(|t| t.db.clone()) {
            Some(d) if !d.is_empty() => d,
            _ => {
                self.report_execution_error(self.lang.tr("query.select_collection_first"));
                return;
            }
        };
        let coll = if parsed.coll.is_empty() {
            match self.current_collection() {
                Some((_, c)) => c,
                None => {
                    self.report_execution_error(self.lang.tr("query.select_collection_first"));
                    return;
                }
            }
        } else {
            parsed.coll.clone()
        };
        self.limit = parsed.limit;
        self.skip = parsed.skip;
        let query_script = self.script_text.trim().to_string();
        self.log_query_script(&query_script);
        self.execute_query(&db, &coll, &parsed.filter, &parsed.sort, self.limit, self.skip);
        if let Some(tab) = self.tabs.get_mut(self.active_tab) {
            tab.dirty = false;
            tab.limit = self.limit;
            tab.skip = self.skip;
            tab.coll = coll;
        }
    }

    /// 执行当前编辑器对应的查询模式。
    pub fn run_current_query(&mut self) {
        if self.query_mode == QueryMode::Aggregate {
            self.run_aggregation();
        } else {
            self.run_query();
        }
    }

    /// 分页刷新：重新解析脚本的 filter/sort，但使用当前分页控件（skip/limit）而非脚本里的 .skip()/.limit()。
    pub fn run_query_paged(&mut self) {
        let parsed = match parse_script(&self.script_text) {
            Ok(p) => p,
            Err(e) => {
                self.report_execution_error(format!("Failed to execute script. Error: {}", e));
                return;
            }
        };
        let db = match self.tabs.get(self.active_tab).map(|t| t.db.clone()) {
            Some(d) if !d.is_empty() => d,
            _ => {
                self.report_execution_error(self.lang.tr("query.select_collection_first"));
                return;
            }
        };
        let coll = if parsed.coll.is_empty() {
            match self.current_collection() {
                Some((_, c)) => c,
                None => {
                    self.report_execution_error(self.lang.tr("query.select_collection_first"));
                    return;
                }
            }
        } else {
            parsed.coll.clone()
        };
        let query_script = self.script_text.trim().to_string();
        self.log_query_script(&query_script);
        self.execute_query(&db, &coll, &parsed.filter, &parsed.sort, self.limit, self.skip);
    }

    /// 以聚合管道模式执行：解析 agg_text 为 JSON 数组管道并运行，结果回填 documents。
    pub fn run_aggregation(&mut self) {
        let db = match self.tabs.get(self.active_tab).map(|t| t.db.clone()) {
            Some(d) if !d.is_empty() => d,
            _ => {
                self.report_execution_error(self.lang.tr("agg.select_collection_first"));
                return;
            }
        };
        let coll = match self.current_collection() {
            Some((_, c)) => c,
            None => {
                self.report_execution_error(self.lang.tr("agg.select_collection_first"));
                return;
            }
        };
        let text = self.agg_text.trim();
        let pipeline = match bson_ext::parse_pipeline_text(text) {
            Ok(p) => p,
            Err(e) => {
                self.report_execution_error(format!(
                    "{}: {}",
                    self.lang.tr("agg.parse_failed"),
                    e
                ));
                return;
            }
        };
        self.log_query_script(&format!(
            "db.getCollection('{}').aggregate({})",
            coll.replace('\\', "\\\\").replace('\'', "\\'"),
            text
        ));
        if let Some(svc) = &self.service {
            let start = std::time::Instant::now();
            match self.rt.block_on(svc.aggregate(&db, &coll, &pipeline)) {
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
                        self.lang.tr("agg.title"),
                        self.documents.len(),
                        elapsed
                    ));
                    if let Some(tab) = self.tabs.get_mut(self.active_tab) {
                        tab.documents = self.documents.clone();
                        tab.query_time_ms = elapsed;
                    }
                }
                Err(e) => self.report_execution_error(format!(
                    "{}: {}",
                    self.lang.tr("agg.failed"),
                    e
                )),
            }
        } else {
            self.report_execution_error(self.lang.tr("msg.disconnected"));
        }
        self.sync_to_active_tab();
    }

    fn report_execution_error(&mut self, message: impl Into<String>) {
        let message = message.into();
        self.status(&message);
        self.log_error(message);
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
                Err(e) => self.report_execution_error(trf(
                    self.lang,
                    "msg.query_failed",
                    &[&e.to_string()],
                )),
            }
        } else {
            self.report_execution_error(self.lang.tr("msg.disconnected"));
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

    /// 导出文档到 JSON 文件（单个或全部结果）。使用扩展 JSON 以便 ObjectId/日期等原样往返。
    pub fn export_documents(&mut self, path: &str, docs: &[Document]) {
        let parts: Vec<String> = docs
            .iter()
            .filter_map(|d| rubo3t_core::bson_ext::document_to_text(d).ok())
            .collect();
        let body = parts.join(",\n");
        let json = format!("[\n{body}\n]");
        match std::fs::write(path, json) {
            Ok(_) => self.toast(
                &trf(self.lang, "doc.export_ok", &[&docs.len().to_string(), path]),
                false,
            ),
            Err(e) => self.toast(&trf(self.lang, "doc.export_failed", &[&e.to_string()]), true),
        }
    }

    /// 从 JSON 文件导入文档到当前集合（支持文档数组或单个文档对象）。
    pub fn import_documents(&mut self, path: &str) {
        let (db, coll) = match self.current_collection() {
            Some(x) => x,
            None => {
                self.toast(self.lang.tr("msg.select_collection_first"), true);
                return;
            }
        };
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                self.toast(&trf(self.lang, "doc.import_failed", &[&e.to_string()]), true);
                return;
            }
        };
        // 优先按数组解析，失败再按单个文档对象解析。
        let docs = match rubo3t_core::bson_ext::parse_pipeline_text(text.trim()) {
            Ok(ds) => ds,
            Err(_) => match rubo3t_core::bson_ext::parse_document_text(text.trim()) {
                Ok(d) => vec![d],
                Err(e) => {
                    self.toast(&trf(self.lang, "doc.import_failed", &[&e.to_string()]), true);
                    return;
                }
            },
        };
        if docs.is_empty() {
            self.toast(self.lang.tr("doc.import_empty"), true);
            return;
        }
        if let Some(svc) = &self.service {
            let mut ok_count = 0usize;
            for d in &docs {
                if self.rt.block_on(svc.insert_one(&db, &coll, d)).is_ok() {
                    ok_count += 1;
                }
            }
            self.toast(&trf(self.lang, "doc.import_ok", &[&ok_count.to_string()]), false);
            self.run_query();
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

    fn parse_index_keys(s: &str) -> Option<Document> {
        let mut doc = Document::new();
        for part in s.split(',').map(|p| p.trim()).filter(|p| !p.is_empty()) {
            let (field, dir) = match part.split_once(':') {
                Some((f, d)) => (f.trim(), d.trim().parse::<i32>().unwrap_or(1)),
                None => (part, 1),
            };
            doc.insert(field.to_string(), dir);
        }
        if doc.is_empty() { None } else { Some(doc) }
    }

    pub fn reload_indexes(&mut self) {
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.list_indexes(&self.indexes_dlg_db, &self.indexes_dlg_coll)) {
                Ok(i) => self.indexes = i,
                Err(e) => self.toast(&trf(self.lang, "index.load_failed", &[&e.to_string()]), true),
            }
        }
    }

    pub fn create_index(&mut self, db: &str, coll: &str, keys_str: &str, name: &str, unique: bool) {
        match Self::parse_index_keys(keys_str) {
            Some(keys) => {
                if let Some(svc) = &self.service {
                    let nm = if name.trim().is_empty() { None } else { Some(name.trim().to_string()) };
                    match self.rt.block_on(svc.create_index(db, coll, &keys, nm, unique)) {
                        Ok(_) => {
                            self.reload_indexes();
                            self.toast(&trf(self.lang, "index.created", &[]), false);
                        }
                        Err(e) => self.toast(&trf(self.lang, "index.create_failed", &[&e.to_string()]), true),
                    }
                }
            }
            None => self.toast(&trf(self.lang, "index.parse_failed", &[keys_str]), true),
        }
    }

    pub fn drop_index(&mut self, db: &str, coll: &str, name: &str) {
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.drop_index(db, coll, name)) {
                Ok(_) => {
                    self.reload_indexes();
                    self.toast(&trf(self.lang, "index.dropped", &[]), false);
                }
                Err(e) => self.toast(&trf(self.lang, "index.drop_failed", &[&e.to_string()]), true),
            }
        }
    }

    pub fn reload_users(&mut self) {
        if let Some(svc) = &self.service {
            match self.rt.block_on(svc.list_users(&self.users_dlg_db)) {
                Ok(u) => self.users = u,
                Err(e) => self.toast(&trf(self.lang, "user.load_failed", &[&e.to_string()]), true),
            }
        }
    }

    pub fn create_user(&mut self, db: &str, name: &str, pwd: &str, roles_str: &str) {
        let roles: Vec<String> = roles_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if let Some(svc) = &self.service {
            let cmd = doc! {
                "createUser": name.to_string(),
                "pwd": pwd.to_string(),
                "roles": roles.clone(),
            };
            match self.rt.block_on(svc.run_command(db, &cmd)) {
                Ok(_) => {
                    self.reload_users();
                    self.toast(&trf(self.lang, "user.created", &[name]), false);
                }
                Err(e) => self.toast(&trf(self.lang, "user.create_failed", &[&e.to_string()]), true),
            }
        }
    }

    pub fn drop_user(&mut self, db: &str, name: &str) {
        if let Some(svc) = &self.service {
            let cmd = doc! { "dropUser": name.to_string() };
            match self.rt.block_on(svc.run_command(db, &cmd)) {
                Ok(_) => {
                    self.reload_users();
                    self.toast(&trf(self.lang, "user.dropped", &[]), false);
                }
                Err(e) => self.toast(&trf(self.lang, "user.drop_failed", &[&e.to_string()]), true),
            }
        }
    }

    /// 编辑用户：updateUser 命令；密码留空则不修改。
    pub fn edit_user(&mut self, db: &str, name: &str, pwd: &str, roles_str: &str) {
        if let Some(svc) = &self.service {
            let mut cmd = rubo3t_core::bson::doc! { "updateUser": name.to_string() };
            if !pwd.is_empty() {
                cmd.insert("pwd", pwd.to_string());
            }
            let roles: Vec<rubo3t_core::bson::Bson> = roles_str
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| rubo3t_core::bson::Bson::String(s.to_string()))
                .collect();
            cmd.insert("roles", roles);
            match self.rt.block_on(svc.run_command(db, &cmd)) {
                Ok(_) => {
                    self.reload_users();
                    self.toast(&trf(self.lang, "user.updated", &[name]), false);
                }
                Err(e) => self.toast(&trf(self.lang, "user.update_failed", &[&e.to_string()]), true),
            }
        }
    }

    /// 编辑索引：MongoDB 不支持就地修改索引，采用「先删旧、后建新」。
    pub fn edit_index(&mut self, db: &str, coll: &str, old_name: &str, keys_str: &str, name: &str, unique: bool) {
        let keys = match rubo3t_core::bson_ext::parse_filter_text(keys_str) {
            Ok(k) => k,
            Err(e) => {
                self.toast(&trf(self.lang, "index.parse_failed", &[&e.to_string()]), true);
                return;
            }
        };
        let nm = if name.is_empty() { None } else { Some(name.to_string()) };
        if let Some(svc) = &self.service {
            if let Err(e) = self.rt.block_on(svc.drop_index(db, coll, old_name)) {
                self.toast(&trf(self.lang, "index.update_failed", &[&e.to_string()]), true);
                return;
            }
            match self.rt.block_on(svc.create_index(db, coll, &keys, nm, unique)) {
                Ok(_) => {
                    self.reload_indexes();
                    self.toast(&trf(self.lang, "index.updated", &[name]), false);
                }
                Err(e) => self.toast(&trf(self.lang, "index.update_failed", &[&e.to_string()]), true),
            }
        }
    }

    // ----- 资源管理器右键菜单动作 -----

    /// 记录「新建集合」意图，由 dialogs 渲染输入框。
    pub fn request_create_collection(&mut self, db: String) {
        self.create_coll_db = Some(db);
    }

    /// 请求删除集合（弹确认框）。
    pub fn request_drop_collection(&mut self, db: String, coll: String) {
        self.confirm_action = Some(ExplorerAction::DropCollection(db, coll));
    }

    /// 请求删除数据库（弹确认框）。
    pub fn request_drop_database(&mut self, db: String) {
        self.confirm_action = Some(ExplorerAction::DropDatabase(db));
    }

    /// 打开插入文档弹窗（先选中集合）。
    pub fn start_insert_document(&mut self, db: String, coll: String) {
        self.select_collection(&db, &coll);
        self.insert_doc_open = true;
        self.selected_doc_text = "{\n  \n}".to_string();
    }

    /// 执行 `create` 命令新建集合。
    pub fn create_collection(&mut self, db: &str, name: &str) {
        if let Some(svc) = &self.service {
            let cmd = rubo3t_core::bson::doc! { "create": name.to_string() };
            match self.rt.block_on(svc.run_command(db, &cmd)) {
                Ok(_) => {
                    self.load_collections(db);
                    self.toast(&trf(self.lang, "msg.coll_created", &[name]), false);
                }
                Err(e) => self.toast(&trf(self.lang, "msg.coll_create_failed", &[&e.to_string()]), true),
            }
        }
    }

    /// 执行资源管理器右键菜单的待确认动作。
    pub fn perform_explorer_action(&mut self, action: ExplorerAction) {
        if let Some(svc) = &self.service {
            match action {
                ExplorerAction::DropCollection(db, coll) => {
                    match self.rt.block_on(svc.drop_collection(&db, &coll)) {
                        Ok(_) => {
                            self.load_collections(&db);
                            self.toast(
                                &trf(self.lang, "msg.coll_dropped", &[&format!("{}.{}", db, coll)]),
                                false,
                            );
                        }
                        Err(e) => {
                            self.toast(&trf(self.lang, "msg.coll_drop_failed", &[&e.to_string()]), true)
                        }
                    }
                }
                ExplorerAction::DropDatabase(db) => {
                    match self.rt.block_on(svc.drop_database(&db)) {
                        Ok(_) => {
                            self.load_databases();
                            self.toast(&trf(self.lang, "msg.db_dropped", &[&db]), false);
                        }
                        Err(e) => {
                            self.toast(&trf(self.lang, "msg.db_drop_failed", &[&e.to_string()]), true)
                        }
                    }
                }
            }
        }
    }

    pub fn request_rename_collection(&mut self, db: String, coll: String) {
        self.rename_coll = Some((db, coll.clone()));
        self.rename_coll_new = coll;
    }

    pub fn rename_collection(&mut self, db: &str, old: &str, new: &str) {
        if let Some(svc) = &self.service {
            let cmd = rubo3t_core::bson::doc! {
                "renameCollection": format!("{}.{}", db, old),
                "to": format!("{}.{}", db, new),
            };
            match self.rt.block_on(svc.run_command(db, &cmd)) {
                Ok(_) => {
                    self.load_collections(db);
                    self.toast(&trf(self.lang, "msg.coll_renamed", &[old, new]), false);
                }
                Err(e) => {
                    self.toast(&trf(self.lang, "msg.coll_rename_failed", &[&e.to_string()]), true)
                }
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
    let mut filter = parse_doc(&filter_str)?;
    if let Some(Bson::String(id)) = filter.get("_id") {
        if let Ok(object_id) = rubo3t_core::bson::oid::ObjectId::parse_str(id) {
            filter.insert("_id", Bson::ObjectId(object_id));
        }
    }

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
            let sort_str = after[..=close_brace].trim();
            sort = parse_doc(sort_str)?;
            rest = after[close_brace + 1..]
                .strip_prefix(')')
                .ok_or_else(|| "sort() 缺少右括号".to_string())?;
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
    let normalized = normalize_doc_syntax(s);
    bson_ext::parse_filter_text(&normalized).map_err(|e| format!("解析文档失败: {}", e))
}

fn normalize_doc_syntax(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                out.push(c);
                let mut escaped = false;
                for inner in chars.by_ref() {
                    out.push(inner);
                    if escaped {
                        escaped = false;
                    } else if inner == '\\' {
                        escaped = true;
                    } else if inner == '"' {
                        break;
                    }
                }
            }
            '\'' => {
                out.push('"');
                let mut escaped = false;
                for inner in chars.by_ref() {
                    if escaped {
                        match inner {
                            '\'' => out.push('\''),
                            '"' => out.push_str("\\\""),
                            '\\' => out.push_str("\\\\"),
                            'n' | 'r' | 't' | 'b' | 'f' | '/' => {
                                out.push('\\');
                                out.push(inner);
                            }
                            _ => {
                                out.push('\\');
                                out.push(inner);
                            }
                        }
                        escaped = false;
                    } else {
                        match inner {
                            '\'' => {
                                out.push('"');
                                break;
                            }
                            '\\' => escaped = true,
                            '"' => out.push_str("\\\""),
                            _ => out.push(inner),
                        }
                    }
                }
            }
            c if c == '_' || c == '$' || c.is_alphabetic() => {
                let mut identifier = String::from(c);
                while let Some(&next) = chars.peek() {
                    if next == '_' || next == '$' || next.is_alphanumeric() {
                        identifier.push(next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let is_key = chars.clone().find(|next| !next.is_whitespace()) == Some(':');
                if is_key {
                    out.push('"');
                    out.push_str(&identifier);
                    out.push('"');
                } else {
                    out.push_str(&identifier);
                }
            }
            _ => out.push(c),
        }
    }

    out
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
    let mut quote = ' ';
    let mut escaped = false;
    let mut cur = String::new();
    for c in s.chars() {
        if in_str {
            cur.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == quote {
                in_str = false;
            }
        } else {
            match c {
                '\'' | '"' => {
                    in_str = true;
                    quote = c;
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

#[cfg(test)]
mod query_script_tests {
    use super::parse_script;
    use rubo3t_core::bson::{oid::ObjectId, Bson};

    #[test]
    fn parses_empty_filter_with_sort() {
        let query = parse_script(
            r#"db.ChinaumsApiLog.find({}).sort({"requestTime": -1}).limit(20).skip(5)"#,
        )
        .expect("query script should parse");

        assert_eq!(query.coll, "ChinaumsApiLog");
        assert!(query.filter.is_empty());
        assert!(matches!(
            query.sort.get("requestTime"),
            Some(Bson::Int32(-1) | Bson::Int64(-1))
        ));
        assert_eq!(query.limit, 20);
        assert_eq!(query.skip, 5);
    }

    #[test]
    fn preserves_chinese_filter_values() {
        let query = parse_script(
            r#"db.ChinaumsApiLog.find({"apiName": "二维码支付"})"#,
        )
        .expect("query script should parse");

        assert_eq!(
            query.filter.get("apiName"),
            Some(&Bson::String("二维码支付".to_string()))
        );
    }

    #[test]
    fn parses_string_id_as_object_id() {
        let query = parse_script(r#"db.items.find({_id: "675019e7af159c4e18299c4c"})"#)
            .expect("string _id query should parse");

        assert_eq!(
            query.filter.get("_id"),
            Some(&Bson::ObjectId(
                ObjectId::parse_str("675019e7af159c4e18299c4c").unwrap()
            ))
        );
    }

    #[test]
    fn keeps_non_object_id_strings_unchanged() {
        let query = parse_script(r#"db.items.find({_id: "not-an-object-id", name: "675019e7af159c4e18299c4c"})"#)
            .expect("string _id query should parse");

        assert_eq!(
            query.filter.get("_id"),
            Some(&Bson::String("not-an-object-id".to_string()))
        );
        assert_eq!(
            query.filter.get("name"),
            Some(&Bson::String(
                "675019e7af159c4e18299c4c".to_string()
            ))
        );
    }

    #[test]
    fn accepts_unquoted_keys_and_single_quoted_values() {
        let query = parse_script(
            "db.ChinaumsApiLog.find({apiName: '二维码支付'}).sort({requestTime: -1})",
        )
        .expect("unquoted keys and single-quoted values should parse");

        assert_eq!(
            query.filter.get("apiName"),
            Some(&Bson::String("二维码支付".to_string()))
        );
        assert!(matches!(
            query.sort.get("requestTime"),
            Some(Bson::Int32(-1) | Bson::Int64(-1))
        ));
    }

    #[test]
    fn preserves_quotes_inside_single_quoted_values() {
        let query = parse_script(r#"db.items.find({name: 'Bob "the builder"'})"#)
            .expect("double quotes inside single-quoted values should parse");

        assert_eq!(
            query.filter.get("name"),
            Some(&Bson::String("Bob \"the builder\"".to_string()))
        );
    }
}
