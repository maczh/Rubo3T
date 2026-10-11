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
    m
});
