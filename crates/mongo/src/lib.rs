//! `rubo3t-mongo` — 基于官方 `mongodb` Rust 驱动的数据访问层。
//!
//! 这是 Rubo3T 重写中「支持 MongoDB 3.x → 8.x、集群、RDS」的关键：
//! 原版 Robo3T 1.4.4 使用的旧 C++ 驱动只支持到 4.2，且其认证/协议写死。
//! 这里改用官方驱动（原生支持 3.6–8.x、SCRAM-SHA-256、TLS、SRV、副本集与
//! 分片集群、Atlas），并把所有「版本差异」交给 `rubo3t-core` 的 `FeatureGate`
//! 处理。本层只负责与驱动对话、把结果转成领域模型。
//!
//! 注意：官方 `mongodb` 3.x 采用「action（构建器）」风格的异步 API——
//! `collection.find(filter)` 返回 `Find` 构建器（链式 `.limit()/.sort()`），
//! `insert_one(doc)` / `update_one(q, u)` / `delete_one(q)` 等也都是先返回 action
//! 再 `.await`。下方所有调用均按此风格编写。

use mongodb::bson::doc;
use mongodb::bson::Document;
use mongodb::options::IndexOptions;
use mongodb::IndexModel;
// mongodb 的 Cursor 通过 `TryStreamExt::try_next` 逐个取出（返回 Result），
// 该 trait 来自 futures-util；本 crate 需显式引入才能调用。
use futures_util::TryStreamExt;
use mongodb::Client;

use rubo3t_core::domain::{
    CollectionInfo, CollectionKind, DatabaseInfo, IndexInfo as DomainIndex, ServerInfo, UserInfo,
};
use rubo3t_core::error::{RuboError, RuboResult};
use rubo3t_core::settings::ConnectionSettings;
use rubo3t_core::version::{FeatureGate, ServerVersion, VersionCapability};

/// MongoDB 服务封装。
#[derive(Clone)]
pub struct MongoService {
    client: Client,
    settings: ConnectionSettings,
    server_info: Option<ServerInfo>,
}

impl MongoService {
    /// 建立连接并探测服务器版本 / 拓扑。
    pub async fn connect(settings: &ConnectionSettings) -> RuboResult<MongoService> {
        rubo3t_core::settings::validate(settings)?;
        let uri = settings.build_uri();
        Self::connect_with_uri(&uri, settings).await
    }

    /// 用「已构造好的连接 URI 字符串」建立连接（例如用户直接粘贴的 DSN）。
    ///
    /// `settings` 仅用于版本/拓扑探测时的上下文（主机展示、集群启发式），
    /// 真正的连接目标完全由 `uri` 决定——这样可在 URI 上附加
    /// `serverSelectionTimeoutMS` 等调试参数，而不污染持久化设置。
    pub async fn connect_with_uri(
        uri: &str,
        settings: &ConnectionSettings,
    ) -> RuboResult<MongoService> {
        let client = Client::with_uri_str(uri)
            .await
            .map_err(|e| RuboError::Connection(format!("无法连接到 {}: {}", settings.name, e)))?;

        // 探测版本与拓扑
        let server_info = detect_server_info(&client, settings).await?;

        Ok(MongoService {
            client,
            settings: settings.clone(),
            server_info: Some(server_info),
        })
    }

    /// 已探测到的服务器信息。连接失败时返回 None。
    pub fn server_info(&self) -> Option<&ServerInfo> {
        self.server_info.as_ref()
    }

    /// 能力门控（供 UI 判断版本差异）。
    pub fn feature_gate(&self) -> Option<FeatureGate> {
        self.server_info
            .as_ref()
            .map(|s| FeatureGate::new(s.version))
    }

    pub fn settings(&self) -> &ConnectionSettings {
        &self.settings
    }

    fn database(&self, name: &str) -> mongodb::Database {
        self.client.database(name)
    }

    /// 列出数据库（含大小）。
    pub async fn list_databases(&self) -> RuboResult<Vec<DatabaseInfo>> {
        let admin = self.client.database("admin");
        let res = admin
            .run_command(doc! {"listDatabases": 1})
            .await
            .map_err(map_err)?;
        let mut out = Vec::new();
        if let Some(bson::Bson::Array(dbs)) = res.get("databases") {
            for d in dbs {
                if let bson::Bson::Document(d) = d {
                    let name = d.get_str("name").unwrap_or_default().to_string();
                    let size = d.get("sizeOnDisk").and_then(|v| v.as_i64()).unwrap_or(0) as u64;
                    out.push(DatabaseInfo {
                        name,
                        size_on_disk: size,
                    });
                }
            }
        }
        if out.is_empty() {
            // 退化方案：仅列名称
            let names = self.client.list_database_names().await.map_err(map_err)?;
            out = names
                .into_iter()
                .map(|n| DatabaseInfo {
                    name: n,
                    size_on_disk: 0,
                })
                .collect();
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// 列出集合 / 视图。
    pub async fn list_collections(&self, db: &str) -> RuboResult<Vec<CollectionInfo>> {
        let database = self.database(db);
        let mut cursor = database
            .run_cursor_command(doc! {
                "listCollections": 1,
                "nameOnly": false,
            })
            .await
            .map_err(map_err)?;
        let mut out = Vec::new();
        while let Some(spec) = cursor.try_next().await.map_err(map_err)? {
            out.push(collection_info_from_spec(spec)?);
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    /// 查询文档。
    pub async fn find(
        &self,
        db: &str,
        coll: &str,
        filter: &Document,
        sort: &Document,
        limit: u64,
        skip: u64,
    ) -> RuboResult<Vec<Document>> {
        let collection: mongodb::Collection<Document> = self.database(db).collection(coll);
        // action 风格：find 返回 Find 构建器，链式设置 limit/skip/sort
        let mut action = collection.find(filter.clone());
        action = action.limit(limit as i64).skip(skip);
        if !sort.is_empty() {
            action = action.sort(sort.clone());
        }
        let mut cursor = action.await.map_err(map_err)?;
        let mut out = Vec::new();
        while let Some(doc) = cursor.try_next().await.map_err(map_err)? {
            out.push(doc);
        }
        Ok(out)
    }

    /// 执行聚合管道，返回结果文档列表。
    pub async fn aggregate(
        &self,
        db: &str,
        coll: &str,
        pipeline: &[Document],
    ) -> RuboResult<Vec<Document>> {
        let collection: mongodb::Collection<Document> = self.database(db).collection(coll);
        let mut cursor = collection
            .aggregate(pipeline.to_vec())
            .await
            .map_err(map_err)?;
        let mut out = Vec::new();
        while let Some(doc) = cursor.try_next().await.map_err(map_err)? {
            out.push(doc);
        }
        Ok(out)
    }

    /// 统计匹配文档数。
    pub async fn count(&self, db: &str, coll: &str, filter: &Document) -> RuboResult<u64> {
        let collection: mongodb::Collection<Document> = self.database(db).collection(coll);
        let n = collection
            .count_documents(filter.clone())
            .await
            .map_err(map_err)?;
        Ok(n)
    }

    /// 插入一条文档。
    pub async fn insert_one(&self, db: &str, coll: &str, doc: &Document) -> RuboResult<()> {
        let collection: mongodb::Collection<Document> = self.database(db).collection(coll);
        collection.insert_one(doc.clone()).await.map_err(map_err)?;
        Ok(())
    }

    /// 更新一条文档（可按 filter 选择是否 upsert）。
    pub async fn update_one(
        &self,
        db: &str,
        coll: &str,
        filter: &Document,
        update: &Document,
        upsert: bool,
    ) -> RuboResult<()> {
        let collection: mongodb::Collection<Document> = self.database(db).collection(coll);
        collection
            .update_one(filter.clone(), update.clone())
            .upsert(upsert)
            .await
            .map_err(map_err)?;
        Ok(())
    }

    /// 删除一条文档。
    pub async fn delete_one(&self, db: &str, coll: &str, filter: &Document) -> RuboResult<()> {
        let collection: mongodb::Collection<Document> = self.database(db).collection(coll);
        collection
            .delete_one(filter.clone())
            .await
            .map_err(map_err)?;
        Ok(())
    }

    /// 列出索引。
    pub async fn list_indexes(&self, db: &str, coll: &str) -> RuboResult<Vec<DomainIndex>> {
        let collection: mongodb::Collection<Document> = self.database(db).collection(coll);
        let mut cursor = collection.list_indexes().await.map_err(map_err)?;
        let mut out = Vec::new();
        while let Some(idx) = cursor.try_next().await.map_err(map_err)? {
            let keys = idx.keys.clone();
            let opts = idx.options.clone().unwrap_or_default();
            let keys_json = rubo3t_core::bson_ext::document_to_json(&keys).unwrap_or_default();
            out.push(DomainIndex {
                name: opts.name.clone().unwrap_or_else(|| "<unnamed>".into()),
                keys: keys_json,
                unique: opts.unique.unwrap_or(false),
                // `background` 选项在现代 MongoDB 已废弃且无效，统一置 false。
                background: false,
                sparse: opts.sparse.unwrap_or(false),
            });
        }
        Ok(out)
    }

    /// 创建索引。
    pub async fn create_index(
        &self,
        db: &str,
        coll: &str,
        keys: &Document,
        name: Option<String>,
        unique: bool,
    ) -> RuboResult<()> {
        let collection: mongodb::Collection<Document> = self.database(db).collection(coll);
        // IndexOptions 是 #[non_exhaustive] + TypedBuilder，不能用字面量构造，
        // 但字段均为公开且 derive 了 Default，故用 Default 后直接赋值最稳妥。
        let mut opts = IndexOptions::default();
        opts.unique = Some(unique);
        opts.name = name;
        let model = IndexModel::builder()
            .keys(keys.clone())
            .options(Some(opts))
            .build();
        collection.create_index(model).await.map_err(map_err)?;
        Ok(())
    }

    /// 删除索引。
    pub async fn drop_index(&self, db: &str, coll: &str, name: &str) -> RuboResult<()> {
        let collection: mongodb::Collection<Document> = self.database(db).collection(coll);
        collection.drop_index(name).await.map_err(map_err)?;
        Ok(())
    }

    /// 列出用户（在指定库上执行 usersInfo）。
    pub async fn list_users(&self, db: &str) -> RuboResult<Vec<UserInfo>> {
        let database = self.database(db);
        let res = database
            .run_command(doc! {"usersInfo": 1})
            .await
            .map_err(map_err)?;
        let mut out = Vec::new();
        if let Some(bson::Bson::Array(users)) = res.get("users") {
            for u in users {
                if let bson::Bson::Document(d) = u {
                    let name = d.get_str("user").unwrap_or_default().to_string();
                    let roles: Vec<String> = d
                        .get("roles")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|r| {
                                    r.as_document().and_then(|doc| {
                                        doc.get_str("role").ok().map(|s| s.to_string())
                                    })
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    out.push(UserInfo {
                        name,
                        db: db.to_string(),
                        roles,
                    });
                }
            }
        }
        Ok(out)
    }

    /// 执行任意命令（Shell / 诊断用）。返回原始文档。
    pub async fn run_command(&self, db: &str, cmd: &Document) -> RuboResult<Document> {
        let database = self.database(db);
        let res = database.run_command(cmd.clone()).await.map_err(map_err)?;
        Ok(res)
    }

    /// 服务器运行状态统计（connections / memory / opcounters / asserts 等）。
    pub async fn server_status(&self, db: &str) -> RuboResult<Document> {
        let d = self.database(db);
        d.run_command(doc! { "serverStatus": 1 })
            .await
            .map_err(map_err)
    }

    /// 主机信息（CPU/内存/内核）。
    pub async fn host_info(&self, db: &str) -> RuboResult<Document> {
        let d = self.database(db);
        d.run_command(doc! { "hostInfo": 1 }).await.map_err(map_err)
    }

    /// 副本集状态（含成员拓扑）；单机实例会报错，调用方需容错。
    pub async fn repl_set_status(&self, db: &str) -> RuboResult<Document> {
        let d = self.database(db);
        d.run_command(doc! { "replSetGetStatus": 1 })
            .await
            .map_err(map_err)
    }

    /// 启动命令行参数。
    pub async fn cmd_line_opts(&self, db: &str) -> RuboResult<Document> {
        let d = self.database(db);
        d.run_command(doc! { "getCmdLineOpts": 1 })
            .await
            .map_err(map_err)
    }

    /// 删除集合。
    pub async fn drop_collection(&self, db: &str, coll: &str) -> RuboResult<()> {
        let collection: mongodb::Collection<Document> = self.database(db).collection(coll);
        collection.drop().await.map_err(map_err)?;
        Ok(())
    }

    /// 删除数据库。
    pub async fn drop_database(&self, db: &str) -> RuboResult<()> {
        self.client.database(db).drop().await.map_err(map_err)?;
        Ok(())
    }
}

/// 探测服务器版本与拓扑。
async fn detect_server_info(
    client: &Client,
    settings: &ConnectionSettings,
) -> RuboResult<ServerInfo> {
    let admin = client.database("admin");
    let build = admin
        .run_command(doc! {"buildInfo": 1})
        .await
        .map_err(map_err)?;
    let version_str = build
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RuboError::Connection("无法解析服务器版本".into()))?;
    let version = ServerVersion::parse(version_str)
        .ok_or_else(|| RuboError::Connection(format!("无法解析版本: {version_str}")))?;

    // 拓扑探测：isMaster（hello 在 4.4+ 才引入，isMaster 在 3.x–8.x 均受支持）
    let hello = admin
        .run_command(doc! {"isMaster": 1})
        .await
        .map_err(map_err)?;
    let is_replicaset = hello.get("setName").is_some();
    let is_sharded = hello
        .get("msg")
        .and_then(|v| v.as_str())
        .map(|s| s == "isdbgrid")
        .unwrap_or(false);
    let is_cluster = is_replicaset
        || is_sharded
        || settings.mode != rubo3t_core::settings::ConnectionMode::Standalone
        || settings.hosts.len() > 1;

    let edition = build
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let gate = FeatureGate::new(version);
    let feature_summary: Vec<VersionCapability> = gate.summary();

    Ok(ServerInfo {
        version,
        edition,
        host: settings
            .hosts
            .first()
            .map(|h| h.to_string())
            .unwrap_or_default(),
        is_cluster,
        feature_summary,
    })
}

fn map_err(e: mongodb::error::Error) -> RuboError {
    RuboError::Mongo(e.to_string())
}

fn collection_info_from_spec(spec: Document) -> RuboResult<CollectionInfo> {
    let name = spec
        .get_str("name")
        .map_err(|e| RuboError::Mongo(format!("无效的集合列表响应: {e}")))?
        .to_string();
    let kind = match spec.get_str("type").unwrap_or_default() {
        "view" => CollectionKind::View,
        "timeseries" => CollectionKind::Timeseries,
        "collection" => CollectionKind::Collection,
        _ => CollectionKind::Unknown,
    };

    Ok(CollectionInfo {
        name,
        kind,
        size_bytes: 0,
        count: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::collection_info_from_spec;
    use mongodb::bson::doc;
    use rubo3t_core::settings::{ConnectionSettings, Host};

    #[test]
    fn collection_spec_accepts_floating_point_size_options() {
        let spec = doc! {
            "name": "large_collection",
            "type": "collection",
            "options": { "size": 2_621_440_000.0 },
        };

        let info = collection_info_from_spec(spec).expect("collection specification should parse");

        assert_eq!(info.name, "large_collection");
        assert_eq!(info.kind, rubo3t_core::domain::CollectionKind::Collection);
    }

    #[tokio::test]
    async fn generated_uri_with_options_parses() {
        let settings = ConnectionSettings {
            hosts: vec![Host::new("localhost", 27017)],
            replica_set_name: Some("rs0".into()),
            ..Default::default()
        };

        let uri = settings.build_uri();
        mongodb::options::ClientOptions::parse(&uri)
            .await
            .expect("generated MongoDB URI should parse");
    }
}
