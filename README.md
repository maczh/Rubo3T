# Rubo3T

> A Rust rewrite of [Robo3T](https://github.com/Studio3T/robomongo) — a lightweight, cross-platform MongoDB GUI client.

[中文文档](#中文文档) · [English](#english)

---

## English

### What is Rubo3T?

Rubo3T is a from-scratch reimplementation of Robo3T (the classic 1.4.4 desktop
client) in Rust. It keeps the familiar Robo3T workflow and look, while replacing
the C++/Qt stack with [egui](https://github.com/emilk/egui) + [eframe](https://github.com/emilk/egui)
and the official [`mongodb`](https://crates.io/crates/mongodb) Rust driver.

The goal: a fast, native, memory-safe MongoDB client that covers MongoDB **3.x → 8.x**,
replica sets, sharded clusters, Atlas SRV, Amazon DocumentDB, every mainstream
authentication mechanism, TLS, and SSH tunnels — without a browser or Electron.

> Status: **MVP**. Core connection, query, CRUD, index/user/server-status features
> are implemented. See `docs/robo3t-ui-1to1-spec.md` for the UI fidelity spec.

### Features

- **Connections** — Standalone, Replica Set, Sharded Cluster; `mongodb+srv://` for
  Atlas / DocumentDB; optional direct connection (skip seed discovery).
- **Authentication** — version-aware default mechanism + explicit selection:
  `SCRAM-SHA-256`, `SCRAM-SHA-1`, legacy `MONGODB-CR`, `X.509`, and OIDC.
- **TLS / SSL** and **SSH tunnel** (local port forwarding).
- **Explorer** — list databases, collections (incl. capped/views/timeseries),
  indexes, and users.
- **Query** — `find` / `aggregate` / `count` / arbitrary `runCommand`, with pagination.
- **Document CRUD** — insert, edit, save, delete; JSON import / export.
- **Server diagnostics** — `serverStatus`, `hostInfo`, `replSetGetStatus`,
  `getCmdLineOpts`.
- **i18n** — 中文 / English UI.
- **Version-aware feature gating** — capabilities adapt to the connected server
  version (see `FeatureGate` in `crates/core`).

### Build

Requires the Rust toolchain (edition 2021) and a Python 3 interpreter (only for
packaging).

```bash
# Debug run
cargo run

# Release build
cargo build --release
# Binary: target/release/rubo3t
```

### Package / Release

Cross-platform installers are produced with [`cargo-packager`](https://github.com/crabnebula-dev/cargo-packager)
(+ `cargo-generate-rpm` on Linux for `.rpm`):

| Platform | Artifacts                         |
|----------|-----------------------------------|
| macOS    | `.app` (cargo-packager) + `.dmg` (hdiutil) |
| Linux    | `.deb` + `.AppImage` + `.rpm`    |
| Windows  | `.msi` (WiX) + `-setup.exe` (NSIS) |

```bash
# All-in-one (build + package for current OS)
python3 scripts/package.py
# or via the Justfile / Makefile
just package      # or: make package

# Skip the build step if already compiled
python3 scripts/package.py --no-build
# Build only a specific format
python3 scripts/package.py --format dmg
```

On push of a `v*` tag, `.github/workflows/release.yml` builds and packages all
three platforms and uploads the artifacts to a draft GitHub Release.

### Project layout

```
crates/
  core/   domain models, errors, version + FeatureGate, bson<->text, settings
  mongo/  MongoService (async wrapper over the mongodb driver)
  ssh/    SSH tunnel (local port forwarding)
  app/    egui GUI (AppState, forms, i18n, icons, UI)
scripts/  package.py (packaging), generate-icons.py (icon assets)
docs/     robo3t-ui-1to1-spec.md (1:1 UI fidelity spec)
```

### License

[GPL-3.0](./LICENSE)

---

## 中文文档

### 项目简介

Rubo3T 是用 Rust 对经典 MongoDB 桌面客户端 **Robo3T 1.4.4** 的彻底重写。它在保留
Robo3T 原有操作习惯与界面风格的前提下，将 C++/Qt 技术栈替换为
[egui](https://github.com/emilk/egui) + [eframe](https://github.com/emilk/egui)
GUI 框架，并使用官方 [`mongodb`](https://crates.io/crates/mongodb) Rust 驱动。

目标：提供一个**原生、轻量、内存安全**的 MongoDB 客户端，覆盖 MongoDB **3.x → 8.x**、
副本集、分片集群、Atlas(SRV)、Amazon DocumentDB、主流认证机制、TLS 与 SSH 隧道，
不依赖浏览器或 Electron。

> 当前状态：**MVP（最小可用版本）**。连接、查询、文档 CRUD、索引/用户/服务器状态等
> 核心功能已实现，界面按 `docs/robo3t-ui-1to1-spec.md` 规格 1:1 复刻 Robo3T。

### 功能特性

- **连接管理** — 单节点（Standalone）、副本集（Replica Set）、分片集群（Sharded
  Cluster）；`mongodb+srv://` 直连 Atlas / DocumentDB；支持直连模式（跳过种子发现）。
- **身份认证** — 按服务器版本自动选择默认机制，也可手动指定：
  `SCRAM-SHA-256`、`SCRAM-SHA-1`、旧版 `MONGODB-CR`、`X.509`、OIDC。
- **TLS / SSL** 加密连接与 **SSH 隧道**（本地端口转发）。
- **对象浏览器** — 列出数据库、集合（含上限集合 / 视图 / 时序集合）、索引与用户。
- **查询执行** — `find` / `aggregate` / `count` / 任意 `runCommand`，支持分页。
- **文档 CRUD** — 新增、编辑、保存、删除；JSON 导入 / 导出。
- **服务器诊断** — `serverStatus`、`hostInfo`、`replSetGetStatus`、`getCmdLineOpts`。
- **国际化** — 中文 / English 界面可切换。
- **版本感知能力门控** — 功能随连接的服务器版本自适应（见 `crates/core` 的 `FeatureGate`）。

### 支持的 MongoDB 版本与能力

通过 `FeatureGate` 根据服务器版本动态开启能力，典型包括：

| 能力 | 说明 |
|------|------|
| `supports_scram_sha256` | SCRAM-SHA-256 认证（4.0+ 默认） |
| `supports_mongodb_cr` | 旧版 MONGODB-CR 认证 |
| `supports_sessions` | 因果一致性会话 |
| `supports_retryable_writes` | 可重试写入 |
| `supports_change_streams` | 变更流 |
| `supports_zstd` | Zstd 压缩 |
| `supports_merge` / `supports_union_with` | 聚合 `$merge` / `$unionWith` |
| `supports_lookup_subquery` / `supports_graph_lookup_non_mongos` | 聚合子查询 / 非 mongos `$graphLookup` |
| `supports_queryable_encryption` | 可查询加密 |
| `supports_oidc` | OIDC 认证 |
| `supports_timeseries` | 时序集合 |
| `supports_replica_set` | 副本集 |

### 构建

需要 Rust 工具链（edition 2021）与 Python 3（仅打包时使用）。

```bash
# 调试运行
cargo run

# 发布构建
cargo build --release
# 产物：target/release/rubo3t
```

### 打包与发布

跨平台安装包由 [`cargo-packager`](https://github.com/crabnebula-dev/cargo-packager)
（Linux 上 `.rpm` 另由 `cargo-generate-rpm` 生成）产出：

| 平台   | 产物                              |
|--------|-----------------------------------|
| macOS  | `.app`（cargo-packager）+ `.dmg`（hdiutil） |
| Linux  | `.deb` + `.AppImage` + `.rpm`     |
| Windows| `.msi`（WiX）+ `-setup.exe`（NSIS）|

```bash
# 一站式：构建 + 按当前平台打包
python3 scripts/package.py
# 或通过 Justfile / Makefile
just package      # 或：make package

# 已编译时跳过构建
python3 scripts/package.py --no-build
# 只生成指定格式
python3 scripts/package.py --format dmg
```

推送 `v*` 标签时，`.github/workflows/release.yml` 会在三平台构建并打包，将产物上传至
GitHub Release 草稿。

### 项目结构

```
crates/
  core/   领域模型、错误类型、版本与 FeatureGate、bson<->文本、连接设置
  mongo/  MongoService（对 mongodb 驱动的异步封装）
  ssh/    SSH 隧道（本地端口转发）
  app/    egui GUI（AppState、表单、i18n、图标、界面）
scripts/  package.py（打包）、generate-icons.py（图标资源）
docs/     robo3t-ui-1to1-spec.md（1:1 界面复刻规格）
```

### 许可证

[GPL-3.0](./LICENSE)
