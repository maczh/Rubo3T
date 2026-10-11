//! 多语言支持（中 / 英）。
//!
//! 用懒加载的静态 HashMap 做翻译表，`Lang::tr(key)` 返回对应语言文本。
//! 未找到 key 时回退 key 本身，便于开发时定位漏掉的文案。

use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Lang {
    #[default]
    Zh,
    En,
}

impl Lang {
    pub fn label(self) -> &'static str {
        match self {
            Lang::Zh => "中文",
            Lang::En => "English",
        }
    }

    pub fn tr<'a>(self, key: &'a str) -> &'a str {
        let map = match self {
            Lang::Zh => &*ZH,
            Lang::En => &*EN,
        };
        map.get::<str>(key).copied().unwrap_or(key)
    }
}

/// 带参数的翻译：把模板里的 `{}` 按顺序替换为 `args`。
///
/// 因为 Rust 的 `format!` 要求格式串是字面量，而翻译串是运行期查表得到的，
/// 无法用 `format!` 直接格式化，故这里用一个轻量占位符替换实现。
pub fn trf(lang: Lang, key: &str, args: &[&str]) -> String {
    let tmpl = lang.tr(key);
    let mut out = String::with_capacity(tmpl.len());
    let mut it = args.iter();
    let mut chars = tmpl.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' && chars.peek() == Some(&'}') {
            chars.next();
            if let Some(a) = it.next() {
                out.push_str(a);
            }
        } else {
            out.push(c);
        }
    }
    out
}

static ZH: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("app.title", "Rubo3T");
    m.insert("app.subtitle", "用 Rust 重写的 MongoDB 管理工具 · 支持 MongoDB 3.x → 8.x / 集群 / RDS");
    m.insert("common.ok", "确定");
    m.insert("common.close", "关闭");
    m.insert("common.save", "保存");
    m.insert("common.cancel", "取消");
    m.insert("common.delete", "删除");
    m.insert("common.refresh", "刷新");
    m.insert("common.create", "创建");
    m.insert("common.run", "执行");
    m.insert("common.name", "名称");
    m.insert("common.type", "类型");
    m.insert("common.host", "主机");
    m.insert("common.port", "端口");
    m.insert("common.user", "用户");
    m.insert("common.password", "密码");

    m.insert("menu.connect", "连接管理");
    m.insert("menu.disconnect", "断开连接");
    m.insert("menu.version_caps", "版本能力");
    m.insert("caps.title", "版本能力");
    m.insert("caps.not_connected", "尚未连接到服务器，无法显示版本能力。");
    m.insert("caps.server_label", "服务器");
    m.insert("caps.version_label", "版本");
    m.insert("caps.edition_label", "版本类型");
    m.insert("caps.host_label", "主机");
    m.insert("caps.topology_label", "拓扑");
    m.insert("caps.standalone", "单机");
    m.insert("caps.cluster", "集群（副本集 / 分片）");
    m.insert("caps.default_auth_label", "默认认证机制");
    m.insert("caps.matrix_label", "能力矩阵");
    m.insert("caps.since_label", "最低版本");
    m.insert("caps.supported", "支持");
    m.insert("caps.unsupported", "不支持");
    m.insert("auth.warn.cr_removed", "注意：当前服务器 ≥ 4.0，已移除 MONGODB-CR，使用此机制可能连接失败。");
    m.insert("auth.warn.sha256_min", "注意：SCRAM-SHA-256 需要 MongoDB 4.0+，当前服务器版本较低。");
    m.insert("auth.warn.sha1_min", "注意：SCRAM-SHA-1 需要 MongoDB 3.0+，当前服务器版本较低。");
    m.insert("menu.lang", "语言");
    m.insert("status.connected", "已连接");
    m.insert("status.not_connected", "未连接");
    m.insert("status.cluster", "集群");

    m.insert("explorer.title", "连接");
    m.insert("explorer.not_connected", "尚未连接。点击顶部「连接管理」建立连接。");
    m.insert("explorer.no_collections", "(无集合)");

    m.insert("welcome.open_connect", "打开连接管理");

    m.insert("query.filter", "筛选:");
    m.insert("query.sort", "排序:");
    m.insert("query.limit", "限制:");
    m.insert("query.run", "执行查询");
    m.insert("query.new_doc", "新建文档");
    m.insert("query.doc_list", "文档列表");
    m.insert("query.select_collection_first", "请先在左侧选择一个集合");

    m.insert("doc.save", "保存修改");
    m.insert("doc.delete", "删除文档");
    m.insert("doc.insert", "插入该文档");
    m.insert("doc.saved", "文档已保存");
    m.insert("doc.deleted", "文档已删除");
    m.insert("doc.inserted", "已插入新文档");
    m.insert("doc.no_id", "文档缺少 _id，无法定位更新目标");

    m.insert("index.title", "索引管理");
    m.insert("index.existing", "已有索引:");
    m.insert("index.empty", "(无索引)");
    m.insert("index.field", "字段:");
    m.insert("index.unique", "唯一");
    m.insert("index.name", "名称(可选):");
    m.insert("index.field_hint", "支持多字段: a,b 或 a:1,b:-1（默认升序 1）");
    m.insert("index.created", "已创建索引");
    m.insert("index.create_failed", "创建索引失败");
    m.insert("index.dropped", "已删除索引");
    m.insert("index.drop_failed", "删除索引失败");
    m.insert("index.load_failed", "加载索引失败");

    m.insert("agg.title", "聚合管道");
    m.insert("agg.pipeline", "管道 (JSON 数组):");
    m.insert("agg.results", "结果");
    m.insert("agg.parse_failed", "管道解析错误");
    m.insert("agg.failed", "聚合失败");
    m.insert("agg.select_collection_first", "请先选择集合");

    m.insert("user.title", "用户管理");
    m.insert("user.empty", "(无用户)");
    m.insert("user.roles", "角色:");
    m.insert("user.dropped", "已删除用户");
    m.insert("user.drop_failed", "删除用户失败");
    m.insert("user.load_failed", "加载用户失败");

    m.insert("shell.label", "Shell/命令:");
    m.insert("shell.db", "(库)");
    m.insert("shell.run", "执行");
    m.insert("shell.tip", "提示: 输入扩展 JSON 命令，如 {\"find\":\"users\",\"filter\":{}}");
    m.insert("shell.history_up", "上一条");
    m.insert("shell.history_down", "下一条");

    m.insert("dialog.saved_connections", "已保存连接:");
    m.insert("dialog.new", "新建");
    m.insert("dialog.save_mod", "保存修改");
    m.insert("dialog.connect", "连接");
    m.insert("dialog.delete", "删除");
    m.insert("dialog.save_new", "保存为新连接");
    m.insert("dialog.direct", "直接连接");

    m.insert("form.mode.standalone", "单节点");
    m.insert("form.mode.replica", "副本集");
    m.insert("form.mode.shard", "分片集群");
    m.insert("form.use_srv", "使用 SRV (mongodb+srv, 如 Atlas)");
    m.insert("form.direct", "直连 (directConnection)");
    m.insert("form.hosts", "主机 (每行一个 host:port):");
    m.insert("form.replica_set", "副本集名称:");
    m.insert("form.auth", "认证");
    m.insert("form.auth.enable", "启用认证");
    m.insert("form.user", "用户名:");
    m.insert("form.password", "密码:");
    m.insert("form.auth_db", "认证库:");
    m.insert("form.mechanism", "机制:");
    m.insert("mech.auto", "自动(按版本协商)");
    m.insert("mech.mongodbcr", "MONGODB-CR");
    m.insert("mech.scramsha1", "SCRAM-SHA-1");
    m.insert("mech.scramsha256", "SCRAM-SHA-256");
    m.insert("mech.plain", "PLAIN (LDAP/AD)");
    m.insert("mech.x509", "MONGODB-X509");
    m.insert("mech.gssapi", "GSSAPI (Kerberos)");
    m.insert("mech.awsiam", "MONGODB-AWS (IAM)");
    m.insert("ssh.password", "密码");
    m.insert("ssh.key", "密钥");
    m.insert("form.mechanism.tip", "机制说明: 留「自动」时，驱动会按服务器版本协商（4.0+ 默认 SCRAM-SHA-256）。");
    m.insert("form.ssl", "TLS/SSL");
    m.insert("form.ssl.enable", "启用 TLS");
    m.insert("form.ssl.allow_invalid", "允许无效/自签名证书");
    m.insert("form.ssh", "SSH 隧道 (访问 RDS / DocumentDB / 内网)");
    m.insert("form.ssh.enable", "通过 SSH 跳板机转发");
    m.insert("form.ssh.bastion", "跳板机:");
    m.insert("form.ssh.auth", "认证方式");
    m.insert("form.ssh.key", "密钥");
    m.insert("form.ssh.password_label", "密码");
    m.insert("form.ssh.private_key", "私钥路径:");
    m.insert("form.ssh.local_port", "本地端口:");
    m.insert("form.ssh.remote_host", "远端主机:");
    m.insert("form.ssh.remote_port", "远端端口:");

    m.insert("toast.title", "提示");
    m.insert("msg.connected", "已连接到 {} (MongoDB {})");
    m.insert("msg.connect_failed", "连接失败: {}");
    m.insert("msg.disconnected", "已断开连接");
    m.insert("msg.save_ok", "已保存连接设置");
    m.insert("msg.save_failed", "连接设置校验失败");
    m.insert("msg.save_new_ok", "已保存新连接");
    m.insert("msg.save_new_failed", "连接设置校验失败");
    m.insert("msg.select_collection_first", "请先选择集合");
    m.insert("msg.select_doc_first", "请先选择要删除的文档");
    m.insert("msg.load_dbs_failed", "加载数据库失败: {}");
    m.insert("msg.load_colls_failed", "加载集合失败: {}");
    m.insert("msg.load_indexes_failed", "加载索引失败: {}");
    m.insert("msg.load_users_failed", "加载用户失败: {}");
    m.insert("msg.ssh_failed", "SSH 隧道失败: {}");
    m.insert("msg.doc_convert_failed", "文档转换错误: {}");
    m.insert("msg.save_doc_failed", "保存失败: {}");
    m.insert("msg.delete_doc_failed", "删除失败: {}");
    m.insert("msg.insert_doc_failed", "插入失败: {}");
    m.insert("msg.query_returned", "查询返回 {} 条文档");
    m.insert("msg.query_failed", "查询失败: {}");
    m.insert("msg.parse.filter", "筛选 JSON 解析错误: {}");
    m.insert("msg.parse.sort", "排序 JSON 解析错误: {}");
    m.insert("msg.parse.doc", "文档 JSON 解析错误: {}");
    m.insert("msg.parse.pipeline", "管道解析错误: {}");
    m.insert("msg.check_name_host", "请检查名称与主机");
    m.insert("msg.connected", "已连接到 {} (MongoDB {})");

    m.insert("view.table", "表格视图");
    m.insert("view.tree", "树形视图");
    m.insert("view.text", "文本视图");
    m.insert("view.custom", "自定义");

    m.insert("menu.file", "文件");
    m.insert("menu.view", "视图");
    m.insert("menu.options", "选项");
    m.insert("menu.window", "窗口");
    m.insert("menu.help", "帮助");
    m.insert("menu.exit", "退出");
    m.insert("menu.open", "打开...");
    m.insert("menu.save", "保存");
    m.insert("menu.save_as", "另存为...");
    m.insert("menu.refresh", "刷新");
    m.insert("menu.about", "关于 Rubo3T...");
    m.insert("menu.disconnect", "断开连接");
    m.insert("menu.view_mode", "默认视图模式");
    m.insert("menu.display_dates", "日期显示方式");
    m.insert("menu.utc", "UTC");
    m.insert("menu.local_time", "本地时间");
    m.insert("menu.uuid", "旧版 UUID 编码");
    m.insert("menu.uuid_default", "不解码(原样显示)");
    m.insert("menu.uuid_java", "使用 Java 编码");
    m.insert("menu.uuid_dotnet", "使用 .NET 编码");
    m.insert("menu.uuid_python", "使用 Python 编码");
    m.insert("menu.autocomplete", "自动补全模式");
    m.insert("menu.autocomplete_all", "全部");
    m.insert("menu.autocomplete_no_coll", "全部(不含集合名)");
    m.insert("menu.autocomplete_none", "无");
    m.insert("menu.load_mongorc", "加载 .mongorc.js");
    m.insert("menu.auto_expand", "自动展开首个文档");
    m.insert("menu.line_numbers", "默认显示行号");
    m.insert("menu.auto_exec", "新标签页自动执行代码");
    m.insert("menu.disable_conn_shortcuts", "禁用连接快捷键");
    m.insert("menu.full_screen", "全屏");
    m.insert("menu.next_tab", "下一个标签");
    m.insert("menu.prev_tab", "上一个标签");
    m.insert("menu.reexec", "重新执行当前标签查询");
    m.insert("menu.duplicate_tab", "复制查询到新标签");

    m.insert("toolbar.connect", "连接");
    m.insert("toolbar.refresh", "刷新");
    m.insert("toolbar.execute", "执行");
    m.insert("toolbar.stop", "停止");
    m.insert("toolbar.rotate", "旋转结果方向");

    m.insert("tab.close", "关闭标签");

    m.insert("explorer.title", "数据库资源管理器");
    m.insert("explorer.server", "服务器");
    m.insert("explorer.collections", "集合");
    m.insert("explorer.functions", "函数");
    m.insert("explorer.users", "用户");
    m.insert("explorer.indexes", "索引");
    m.insert("explorer.expand", "展开");
    m.insert("explorer.collapse", "折叠");

    m.insert("welcome.title", "欢迎");
    m.insert("welcome.heading", "欢迎使用 Rubo3T");
    m.insert("welcome.sub", "Rust 重写的 MongoDB 管理工具 · 支持 MongoDB 3.x → 8.x / 集群 / RDS");
    m.insert("welcome.open_connect", "打开连接管理");
    m.insert("welcome.tip", "从顶部「连接」按钮或菜单「文件 → 连接…」选择连接");

    m.insert("query.run", "执行");
    m.insert("query.new_doc", "新建文档");
    m.insert("query.doc_count", "文档数");
    m.insert("query.paging", "分页");
    m.insert("query.prev", "上一页");
    m.insert("query.next", "下一页");
    m.insert("query.time", "耗时");
    m.insert("query.tab_title", "查询");

    m.insert("doc.view", "查看文档");
    m.insert("doc.edit", "编辑文档");
    m.insert("doc.copy_value", "复制值");
    m.insert("doc.copy_name", "复制名称");
    m.insert("doc.copy_path", "复制路径");
    m.insert("doc.copy_json", "复制 JSON");
    m.insert("doc.insert_doc", "插入文档...");
    m.insert("doc.delete_doc", "删除文档...");

    m.insert("dialog.conn_title", "MongoDB 连接");
    m.insert("dialog.conn_name", "名称");
    m.insert("dialog.conn_address", "地址");
    m.insert("dialog.conn_attr", "属性");
    m.insert("dialog.conn_auth", "认证库 / 用户");
    m.insert("dialog.conn_create", "新建");
    m.insert("dialog.conn_edit", "编辑");
    m.insert("dialog.conn_remove", "移除");
    m.insert("dialog.conn_clone", "克隆");
    m.insert("dialog.conn_connect", "连接");
    m.insert("dialog.conn_cancel", "取消");
    m.insert("dialog.conn_reorder", "或拖拽排序连接");
    m.insert("dialog.conn_confirm_remove", "确定要删除 “{}” 连接吗?");

    m.insert("settings.title", "连接设置");
    m.insert("settings.tab_connection", "连接");
    m.insert("settings.tab_auth", "认证");
    m.insert("settings.tab_ssh", "SSH");
    m.insert("settings.tab_tls", "TLS");
    m.insert("settings.tab_advanced", "高级");
    m.insert("settings.test", "测试");
    m.insert("settings.default_db", "默认数据库");
    m.insert("settings.from_uri", "从 URI 导入");
    m.insert("settings.uri_placeholder", "从 MongoDB URI 连接字符串导入连接详情");

    m.insert("logs.title", "日志");
    m.insert("logs.toggle", "日志");
    m.insert("logs.empty", "(暂无日志)");

    m.insert("about.title", "关于 Rubo3T");
    m.insert("about.line1", "Rubo3T —— 用 Rust 重写的 MongoDB 管理工具");
    m.insert("about.line2", "UI 复刻 Robo 3T 1.4.4 (Studio 3T)");
    m.insert("about.line3", "驱动: 官方 mongodb-rust 3.8.x · 支持 MongoDB 3.x → 8.x");

    m.insert("options.title", "选项");

    // ---- Robo3T 1:1 改造新增文案 ----
    m.insert("toolbar.open", "打开");
    m.insert("toolbar.save", "保存");
    m.insert("tab.new_shell", "新建 Shell");
    m.insert("tab.reexecute", "重新执行查询");
    m.insert("tab.duplicate", "复制查询到新标签");
    m.insert("tab.close_shell", "关闭 Shell");
    m.insert("tab.close_others", "关闭其他 Shell");
    m.insert("tab.close_right", "关闭右侧 Shell");
    m.insert("paging.skip", "跳过");
    m.insert("paging.batch", "批量大小（一次显示的文档数）");
    m.insert("doc.edit_title", "编辑文档");
    m.insert("doc.insert_title", "插入文档");
    m.insert("indexes.caption", "索引");
    m.insert("users.caption", "用户");
    m.insert("view.custom.tip", "以自定义 UI 查看结果");
    m.insert("view.tree.tip", "以树形模式查看结果");
    m.insert("view.table.tip", "以表格模式查看结果");
    m.insert("view.text.tip", "以文本模式查看结果");
    m.insert("status.bar.tip", "连接状态");
    m.insert("query.script_empty", "（空脚本）");
    m.insert("output.time", "耗时");
    m.insert("dialog.indexes", "索引...");
    m.insert("dialog.users", "用户...");

    // ---- 第二部分对话框（§11–§15）新增文案 ----
    m.insert("dialog.conn_link_create", "新建");
    m.insert("dialog.conn_link_edit", "编辑");
    m.insert("dialog.conn_link_remove", "移除");
    m.insert("dialog.conn_link_clone", "克隆");
    m.insert("dialog.conn_link_tail", "或拖拽排序连接。");
    m.insert("dialog.conn_clone_prefix", "复制的 ");
    m.insert("dialog.confirm_remove_title", "删除连接");
    m.insert("dialog.add", "添加");
    m.insert("form.hint.name", "输入一个便于识别此连接的名称。");
    m.insert("form.hint.address", "指定 MongoDB 服务器主机与端口，可为 IPv4、IPv6 或域名。");
    m.insert("form.tls.ca_file", "CA 文件:");
    m.insert("form.tls.cert_file", "PEM 证书:");
    m.insert("form.tls.key_file", "PEM 密码:");
    m.insert("form.ssh.use_tunnel", "使用 SSH 隧道");
    m.insert("form.ssh.passphrase", "口令:");
    m.insert("form.auth.perform", "执行认证");
    m.insert("attr.replica", "副本集");
    m.insert("attr.tls", "TLS");
    m.insert("attr.ssh", "SSH");
    m.insert("doc.validate", "校验");
    m.insert("about.copyright", "Copyright © 2016-2024 Studio 3T / Robo 3T。UI 复刻 by Rubo3T。");
    m.insert("test.title", "连接诊断");
    m.insert("common.error", "错误");
    m.insert("doc.validate_ok", "JSON 合法");
    m.insert("common.browse", "浏览...");
    m.insert("common.path", "路径:");
    m.insert("form.address", "地址");
    m.insert("form.ssh.address", "地址 (host:port):");
    m.insert("form.tls.use", "使用 TLS/SSL 协议");
    m.insert("form.tls.validate", "校验服务器证书");
    m
});

static EN: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    m.insert("app.title", "Rubo3T");
    m.insert("app.subtitle", "MongoDB admin tool in Rust · supporting MongoDB 3.x → 8.x / cluster / RDS");
    m.insert("common.ok", "OK");
    m.insert("common.close", "Close");
    m.insert("common.save", "Save");
    m.insert("common.cancel", "Cancel");
    m.insert("common.delete", "Delete");
    m.insert("common.refresh", "Refresh");
    m.insert("common.create", "Create");
    m.insert("common.run", "Run");
    m.insert("common.name", "Name");
    m.insert("common.type", "Type");
    m.insert("common.host", "Host");
    m.insert("common.port", "Port");
    m.insert("common.user", "User");
    m.insert("common.password", "Password");

    m.insert("menu.connect", "Connection Manager");
    m.insert("menu.disconnect", "Disconnect");
    m.insert("menu.version_caps", "Version Capabilities");
    m.insert("caps.title", "Version Capabilities");
    m.insert("caps.not_connected", "Not connected to a server; version capabilities unavailable.");
    m.insert("caps.server_label", "Server");
    m.insert("caps.version_label", "Version");
    m.insert("caps.edition_label", "Edition");
    m.insert("caps.host_label", "Host");
    m.insert("caps.topology_label", "Topology");
    m.insert("caps.standalone", "Standalone");
    m.insert("caps.cluster", "Cluster (Replica Set / Sharded)");
    m.insert("caps.default_auth_label", "Default Auth Mechanism");
    m.insert("caps.matrix_label", "Capability Matrix");
    m.insert("caps.since_label", "Since");
    m.insert("caps.supported", "Supported");
    m.insert("caps.unsupported", "Not supported");
    m.insert("auth.warn.cr_removed", "Note: server ≥ 4.0 removed MONGODB-CR; this mechanism may fail.");
    m.insert("auth.warn.sha256_min", "Note: SCRAM-SHA-256 requires MongoDB 4.0+; current server is older.");
    m.insert("auth.warn.sha1_min", "Note: SCRAM-SHA-1 requires MongoDB 3.0+; current server is older.");
    m.insert("menu.lang", "Language");
    m.insert("status.connected", "Connected");
    m.insert("status.not_connected", "Not connected");
    m.insert("status.cluster", "cluster");

    m.insert("explorer.title", "Connection");
    m.insert("explorer.not_connected", "Not connected. Click Connection Manager above.");
    m.insert("explorer.no_collections", "(no collections)");

    m.insert("welcome.open_connect", "Open Connection Manager");

    m.insert("query.filter", "Filter:");
    m.insert("query.sort", "Sort:");
    m.insert("query.limit", "Limit:");
    m.insert("query.run", "Run Query");
    m.insert("query.new_doc", "New Document");
    m.insert("query.doc_list", "Documents");
    m.insert("query.select_collection_first", "Select a collection first");

    m.insert("doc.save", "Save Changes");
    m.insert("doc.delete", "Delete Document");
    m.insert("doc.insert", "Insert Document");
    m.insert("doc.saved", "Document saved");
    m.insert("doc.deleted", "Document deleted");
    m.insert("doc.inserted", "Document inserted");
    m.insert("doc.no_id", "Document missing _id, cannot locate update target");

    m.insert("index.title", "Index Management");
    m.insert("index.existing", "Existing indexes:");
    m.insert("index.empty", "(no indexes)");
    m.insert("index.field", "Field:");
    m.insert("index.unique", "Unique");
    m.insert("index.name", "Name (optional):");
    m.insert("index.field_hint", "Multi-field: a,b or a:1,b:-1 (default ascending 1)");
    m.insert("index.created", "Index created");
    m.insert("index.create_failed", "Create index failed");
    m.insert("index.dropped", "Index dropped");
    m.insert("index.drop_failed", "Drop index failed");
    m.insert("index.load_failed", "Load indexes failed");

    m.insert("agg.title", "Aggregation Pipeline");
    m.insert("agg.pipeline", "Pipeline (JSON array):");
    m.insert("agg.results", "Results");
    m.insert("agg.parse_failed", "Pipeline parse error");
    m.insert("agg.failed", "Aggregation failed");
    m.insert("agg.select_collection_first", "Select a collection first");

    m.insert("user.title", "User Management");
    m.insert("user.empty", "(no users)");
    m.insert("user.roles", "Roles:");
    m.insert("user.dropped", "User dropped");
    m.insert("user.drop_failed", "Drop user failed");
    m.insert("user.load_failed", "Load users failed");

    m.insert("shell.label", "Shell / command:");
    m.insert("shell.db", "(db)");
    m.insert("shell.run", "Run");
    m.insert("shell.tip", "Tip: enter extended JSON command, e.g. {\"find\":\"users\",\"filter\":{}}");
    m.insert("shell.history_up", "Previous");
    m.insert("shell.history_down", "Next");

    m.insert("dialog.saved_connections", "Saved connections:");
    m.insert("dialog.new", "New");
    m.insert("dialog.save_mod", "Save Changes");
    m.insert("dialog.connect", "Connect");
    m.insert("dialog.delete", "Delete");
    m.insert("dialog.save_new", "Save as New");
    m.insert("dialog.direct", "Direct Connect");

    m.insert("form.mode.standalone", "Standalone");
    m.insert("form.mode.replica", "Replica Set");
    m.insert("form.mode.shard", "Sharded Cluster");
    m.insert("form.use_srv", "Use SRV (mongodb+srv, e.g. Atlas)");
    m.insert("form.direct", "Direct connection");
    m.insert("form.hosts", "Hosts (one host:port per line):");
    m.insert("form.replica_set", "Replica set name:");
    m.insert("form.auth", "Authentication");
    m.insert("form.auth.enable", "Enable authentication");
    m.insert("form.user", "Username:");
    m.insert("form.password", "Password:");
    m.insert("form.auth_db", "Auth source:");
    m.insert("form.mechanism", "Mechanism:");
    m.insert("mech.auto", "Auto");
    m.insert("mech.mongodbcr", "MONGODB-CR");
    m.insert("mech.scramsha1", "SCRAM-SHA-1");
    m.insert("mech.scramsha256", "SCRAM-SHA-256");
    m.insert("mech.plain", "PLAIN (LDAP/AD)");
    m.insert("mech.x509", "MONGODB-X509");
    m.insert("mech.gssapi", "GSSAPI (Kerberos)");
    m.insert("mech.awsiam", "MONGODB-AWS (IAM)");
    m.insert("ssh.password", "Password");
    m.insert("ssh.key", "Key");
    m.insert("form.mechanism.tip", "Leave Auto to let driver negotiate by server version (4.0+ defaults to SCRAM-SHA-256).");
    m.insert("form.ssl", "TLS/SSL");
    m.insert("form.ssl.enable", "Enable TLS");
    m.insert("form.ssl.allow_invalid", "Allow invalid/self-signed certificates");
    m.insert("form.ssh", "SSH Tunnel (for RDS / DocumentDB / private network)");
    m.insert("form.ssh.enable", "Forward via SSH bastion");
    m.insert("form.ssh.bastion", "Bastion:");
    m.insert("form.ssh.auth", "Auth method");
    m.insert("form.ssh.key", "Key");
    m.insert("form.ssh.password_label", "Password");
    m.insert("form.ssh.private_key", "Private key path:");
    m.insert("form.ssh.local_port", "Local port:");
    m.insert("form.ssh.remote_host", "Remote host:");
    m.insert("form.ssh.remote_port", "Remote port:");

    m.insert("toast.title", "Notice");
    m.insert("msg.connected", "Connected to {} (MongoDB {})");
    m.insert("msg.connect_failed", "Connection failed: {}");
    m.insert("msg.disconnected", "Disconnected");
    m.insert("msg.save_ok", "Connection settings saved");
    m.insert("msg.save_failed", "Validation failed");
    m.insert("msg.save_new_ok", "New connection saved");
    m.insert("msg.save_new_failed", "Validation failed");
    m.insert("msg.select_collection_first", "Select a collection first");
    m.insert("msg.select_doc_first", "Select a document to delete");
    m.insert("msg.load_dbs_failed", "Load databases failed: {}");
    m.insert("msg.load_colls_failed", "Load collections failed: {}");
    m.insert("msg.load_indexes_failed", "Load indexes failed: {}");
    m.insert("msg.load_users_failed", "Load users failed: {}");
    m.insert("msg.ssh_failed", "SSH tunnel failed: {}");
    m.insert("msg.doc_convert_failed", "Document conversion error: {}");
    m.insert("msg.save_doc_failed", "Save failed: {}");
    m.insert("msg.delete_doc_failed", "Delete failed: {}");
    m.insert("msg.insert_doc_failed", "Insert failed: {}");
    m.insert("msg.query_returned", "Query returned {} documents");
    m.insert("msg.query_failed", "Query failed: {}");
    m.insert("msg.parse.filter", "Filter JSON parse error: {}");
    m.insert("msg.parse.sort", "Sort JSON parse error: {}");
    m.insert("msg.parse.doc", "Document JSON parse error: {}");
    m.insert("msg.parse.pipeline", "Pipeline parse error: {}");
    m.insert("msg.check_name_host", "please check name and host");
    m.insert("msg.connected", "Connected to {} (MongoDB {})");

    m.insert("view.table", "Table View");
    m.insert("view.tree", "Tree View");
    m.insert("view.text", "Text View");
    m.insert("view.custom", "Custom");

    m.insert("menu.file", "File");
    m.insert("menu.view", "View");
    m.insert("menu.options", "Options");
    m.insert("menu.window", "Window");
    m.insert("menu.help", "Help");
    m.insert("menu.exit", "Exit");
    m.insert("menu.open", "Open...");
    m.insert("menu.save", "Save");
    m.insert("menu.save_as", "Save As...");
    m.insert("menu.refresh", "Refresh");
    m.insert("menu.about", "About Rubo3T...");
    m.insert("menu.disconnect", "Disconnect");
    m.insert("menu.view_mode", "Default View Mode");
    m.insert("menu.display_dates", "Display Dates In...");
    m.insert("menu.utc", "UTC");
    m.insert("menu.local_time", "Local Time");
    m.insert("menu.uuid", "Legacy UUID Encoding");
    m.insert("menu.uuid_default", "Do not decode (show as is)");
    m.insert("menu.uuid_java", "Use Java Encoding");
    m.insert("menu.uuid_dotnet", "Use .NET Encoding");
    m.insert("menu.uuid_python", "Use Python Encoding");
    m.insert("menu.autocomplete", "Autocompletion Mode");
    m.insert("menu.autocomplete_all", "All");
    m.insert("menu.autocomplete_no_coll", "All (Except Collection Names)");
    m.insert("menu.autocomplete_none", "None");
    m.insert("menu.load_mongorc", "Load .mongorc.js");
    m.insert("menu.auto_expand", "Auto Expand First Document");
    m.insert("menu.line_numbers", "Show Line Numbers By Default");
    m.insert("menu.auto_exec", "Automatically execute code in new tab");
    m.insert("menu.disable_conn_shortcuts", "Disable Connection Shortcuts");
    m.insert("menu.full_screen", "Full Screen");
    m.insert("menu.next_tab", "Select Next Tab");
    m.insert("menu.prev_tab", "Select Previous Tab");
    m.insert("menu.reexec", "Re-execute Query in Current Tab");
    m.insert("menu.duplicate_tab", "Duplicate Query in New Tab");

    m.insert("toolbar.connect", "Connect");
    m.insert("toolbar.refresh", "Refresh");
    m.insert("toolbar.execute", "Execute");
    m.insert("toolbar.stop", "Stop");
    m.insert("toolbar.rotate", "Rotate");

    m.insert("tab.close", "Close Tab");

    m.insert("explorer.title", "Database Explorer");
    m.insert("explorer.server", "Server");
    m.insert("explorer.collections", "Collections");
    m.insert("explorer.functions", "Functions");
    m.insert("explorer.users", "Users");
    m.insert("explorer.indexes", "Indexes");
    m.insert("explorer.expand", "Expand");
    m.insert("explorer.collapse", "Collapse");

    m.insert("welcome.title", "Welcome");
    m.insert("welcome.heading", "Welcome to Rubo3T");
    m.insert("welcome.sub", "MongoDB admin tool in Rust · supporting MongoDB 3.x → 8.x / cluster / RDS");
    m.insert("welcome.open_connect", "Open Connection Manager");
    m.insert("welcome.tip", "Choose a connection from the Connect button or File → Connect...");

    m.insert("query.run", "Run");
    m.insert("query.new_doc", "New Document");
    m.insert("query.doc_count", "Documents");
    m.insert("query.paging", "Paging");
    m.insert("query.prev", "Previous");
    m.insert("query.next", "Next");
    m.insert("query.time", "Elapsed");
    m.insert("query.tab_title", "Query");

    m.insert("doc.view", "View Document");
    m.insert("doc.edit", "Edit Document");
    m.insert("doc.copy_value", "Copy Value");
    m.insert("doc.copy_name", "Copy Name");
    m.insert("doc.copy_path", "Copy Path");
    m.insert("doc.copy_json", "Copy JSON");
    m.insert("doc.insert_doc", "Insert Document...");
    m.insert("doc.delete_doc", "Delete Document...");

    m.insert("dialog.conn_title", "MongoDB Connections");
    m.insert("dialog.conn_name", "Name");
    m.insert("dialog.conn_address", "Address");
    m.insert("dialog.conn_attr", "Attributes");
    m.insert("dialog.conn_auth", "Auth. Database / User");
    m.insert("dialog.conn_create", "Create");
    m.insert("dialog.conn_edit", "Edit");
    m.insert("dialog.conn_remove", "Remove");
    m.insert("dialog.conn_clone", "Clone");
    m.insert("dialog.conn_connect", "Connect");
    m.insert("dialog.conn_cancel", "Cancel");
    m.insert("dialog.conn_reorder", "or reorder connections via drag'n'drop");
    m.insert("dialog.conn_confirm_remove", "Are you sure you want to delete the '{}' connection?");

    m.insert("settings.title", "Connection Settings");
    m.insert("settings.tab_connection", "Connection");
    m.insert("settings.tab_auth", "Authentication");
    m.insert("settings.tab_ssh", "SSH");
    m.insert("settings.tab_tls", "TLS");
    m.insert("settings.tab_advanced", "Advanced");
    m.insert("settings.test", "Test");
    m.insert("settings.default_db", "Default Database");
    m.insert("settings.from_uri", "From URI");
    m.insert("settings.uri_placeholder", "Import connection details from MongoDB URI connection string");

    m.insert("logs.title", "Logs");
    m.insert("logs.toggle", "Logs");
    m.insert("logs.empty", "(no logs)");

    m.insert("about.title", "About Rubo3T");
    m.insert("about.line1", "Rubo3T —— MongoDB admin tool rewritten in Rust");
    m.insert("about.line2", "UI clone of Robo 3T 1.4.4 (Studio 3T)");
    m.insert("about.line3", "Driver: official mongodb-rust 3.8.x · supports MongoDB 3.x → 8.x");

    m.insert("options.title", "Options");

    // ---- Robo3T 1:1 改造新增文案 ----
    m.insert("toolbar.open", "Open");
    m.insert("toolbar.save", "Save");
    m.insert("tab.new_shell", "New Shell");
    m.insert("tab.reexecute", "Re-execute Query");
    m.insert("tab.duplicate", "Duplicate Query In New Tab");
    m.insert("tab.close_shell", "Close Shell");
    m.insert("tab.close_others", "Close Other Shells");
    m.insert("tab.close_right", "Close Shells to the Right");
    m.insert("paging.skip", "Skip");
    m.insert("paging.batch", "Batch Size (number of documents shown at once)");
    m.insert("doc.edit_title", "Edit Document");
    m.insert("doc.insert_title", "Insert Document");
    m.insert("indexes.caption", "Indexes");
    m.insert("users.caption", "Users");
    m.insert("view.custom.tip", "View results in custom UI");
    m.insert("view.tree.tip", "View results in tree mode");
    m.insert("view.table.tip", "View results in table mode");
    m.insert("view.text.tip", "View results in text mode");
    m.insert("status.bar.tip", "Connection status");
    m.insert("query.script_empty", "(empty script)");
    m.insert("output.time", "Elapsed");
    m.insert("dialog.indexes", "Indexes...");
    m.insert("dialog.users", "Users...");

    // ---- 第二部分对话框（§11–§15）新增文案 ----
    m.insert("dialog.conn_link_create", "Create");
    m.insert("dialog.conn_link_edit", "edit");
    m.insert("dialog.conn_link_remove", "remove");
    m.insert("dialog.conn_link_clone", "clone");
    m.insert("dialog.conn_link_tail", "or reorder connections via drag'n'drop.");
    m.insert("dialog.conn_clone_prefix", "Copy of ");
    m.insert("dialog.confirm_remove_title", "Delete Connection");
    m.insert("dialog.add", "Add");
    m.insert("form.hint.name", "Choose any connection name that will help you to identify this connection.");
    m.insert("form.hint.address", "Specify host and port of MongoDB server. Host can be either IPv4, IPv6 or domain name.");
    m.insert("form.tls.ca_file", "CA File:");
    m.insert("form.tls.cert_file", "PEM Cert:");
    m.insert("form.tls.key_file", "PEM Passphrase:");
    m.insert("form.ssh.use_tunnel", "Use SSH tunnel");
    m.insert("form.ssh.passphrase", "Passphrase:");
    m.insert("form.auth.perform", "Perform authentication");
    m.insert("attr.replica", "Replica Set");
    m.insert("attr.tls", "TLS");
    m.insert("attr.ssh", "SSH");
    m.insert("doc.validate", "Validate");
    m.insert("about.copyright", "Copyright © 2016-2024 Studio 3T / Robo 3T. UI clone by Rubo3T.");
    m.insert("test.title", "Connection Diagnostic");
    m.insert("common.error", "Error");
    m.insert("doc.validate_ok", "JSON is valid");
    m.insert("common.browse", "Browse...");
    m.insert("common.path", "Path:");
    m.insert("form.address", "Address");
    m.insert("form.ssh.address", "Address (host:port):");
    m.insert("form.tls.use", "Use TLS/SSL Protocol");
    m.insert("form.tls.validate", "Validate Server Certificate");
    m
});
