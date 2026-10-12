//! `rubo3t-ssh` — 基于纯 Rust SSH 客户端 [`russh`] 的本地端口转发隧道。
//!
//! 用于经跳板机访问 AWS DocumentDB / RDS / 内网 MongoDB 等场景。
//! 完全不依赖系统 `ssh` / `sshpass` 二进制，因此可干净地打进 macOS/Linux/Windows
//! 安装包，跨平台行为一致。
//!
//! 实现方式：与 OpenSSH 的 `-N -L local:remote_host:remote_port` 等价——在本地
//! `127.0.0.1:local_port` 监听，每接受一个本地连接就通过 SSH 会话开一条
//! `direct-tcpip` 通道到远端 MongoDB，并双向转发字节。
//!
//! - 公钥认证：加载 `private_key_path` 指定的私钥（支持口令 `passphrase`）。
//! - 密码认证：直接使用用户名/密码。
//! - 主机密钥校验：等价 `StrictHostKeyChecking=no`（开发/内网场景常见，先在 `connect`
//!   阶段接受，后续可改为基于 known_hosts 的校验）。
//!
//! 调试日志：通过 `start_with_logger` 传入回调，隧道的完整建立/转发过程都会回调出去，
//! 由调用方（如 GUI 的日志框）展示。

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use rubo3t_core::error::{RuboError, RuboResult};
use rubo3t_core::settings::{SshAuthMethod, SshSettings};

use russh::client::{connect, Config, Handler, Handle};
use russh::keys::key::PublicKey;
use russh::keys::load_secret_key;
use tokio::io::copy_bidirectional;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

/// 调试日志回调：隧道把每一步过程通过它传出（`&str` 为单行日志）。
///
/// 必须是 `'static + Send + Sync`，因为隧道会在独立后台任务里并发调用它
/// （每条本地连接都会产生日志）。
pub type SshLogger = Arc<dyn Fn(&str) + Send + Sync + 'static>;

/// 本地端口转发隧道（纯 Rust 实现）。
///
/// 当 `settings.enabled == false` 时，`start` 返回一个「空隧道」（不实际建立 SSH），
/// 调用方据此把 Mongo 连接目标改写为本地转发端口。
pub struct Tunnel {
    local_port: u16,
    /// 隧道任务所在的私有 tokio 运行时；为空表示空隧道（未启用）。
    runtime: Option<tokio::runtime::Runtime>,
    /// 关闭信号发送端；发送即让接受循环退出。
    shutdown: Option<broadcast::Sender<()>>,
    /// 接受循环任务句柄（持有 SSH Handle，维持会话存活）。
    task: Option<JoinHandle<()>>,
}

impl Tunnel {
    /// 根据连接设置启动隧道（无日志回调版本）。
    ///
    /// 若 `settings.enabled` 为 false，返回空隧道（不建立连接）。
    /// 若启用，则**同步**完成 SSH 握手与本地监听绑定后才返回；返回即隧道就绪，
    /// 后续的 MongoDB 连接可直接指向 `local_port`。
    pub fn start(settings: &SshSettings) -> RuboResult<Tunnel> {
        Self::start_with_logger(
            settings,
            settings.remote_host.clone(),
            settings.remote_port,
            None,
        )
    }

    /// 根据连接设置启动隧道，并把完整过程通过 `logger` 回调出来。
    ///
    /// `remote_host` / `remote_port` 为隧道的转发目标（即 MongoDB 真实服务器地址，
    /// 从跳板机可达），通常由调用方从 `ConnectionSettings.hosts[0]` 传入；若直接调用
    /// [`Tunnel::start`]，则回退使用 `SshSettings.remote_host` / `remote_port`。
    pub fn start_with_logger(
        settings: &SshSettings,
        remote_host: String,
        remote_port: u16,
        logger: Option<SshLogger>,
    ) -> RuboResult<Tunnel> {
        let log = |m: &str| {
            if let Some(l) = &logger {
                l(m);
            }
        };

        if !settings.enabled {
            log("SSH 隧道未启用，跳过（直接使用原始主机地址）。");
            return Ok(Tunnel {
                local_port: settings.local_port,
                runtime: None,
                shutdown: None,
                task: None,
            });
        }

        let local_port_hint = if settings.local_port == 0 {
            "0 (系统随机分配)".to_string()
        } else {
            settings.local_port.to_string()
        };
        log(&format!(
            "SSH 隧道启动: {}@{}:{} → 127.0.0.1:{} → {}:{}",
            settings.user,
            settings.host,
            settings.port,
            local_port_hint,
            remote_host,
            remote_port,
        ));

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| RuboError::Ssh(format!("无法创建 SSH 隧道运行时: {e}")))?;

        // 在私有运行时内完成连接、认证与监听绑定；随后接受循环作为任务继续运行。
        let (shutdown, task, actual_port) = rt
            .block_on(setup_tunnel(settings, remote_host, remote_port, logger.clone()))
            .map_err(|e| RuboError::Ssh(format!("SSH 隧道启动失败: {e}")))?;

        Ok(Tunnel {
            local_port: actual_port,
            runtime: Some(rt),
            shutdown: Some(shutdown),
            task: Some(task),
        })
    }

    /// 本地监听端口（Mongo 连接应指向此端口）。
    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    /// 主动停止隧道（Drop 时也会自动停止）。
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
        if let Some(rt) = self.runtime.take() {
            rt.shutdown_timeout(Duration::from_secs(2));
        }
    }

    /// 隧道是否实际在运行（而非空隧道）。
    pub fn is_active(&self) -> bool {
        self.runtime.is_some()
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        self.stop();
    }
}

/// russh 客户端 Handler：接受任意主机密钥（等同 StrictHostKeyChecking=no）。
struct ClientHandler;

#[async_trait]
impl Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(&mut self, _server_public_key: &PublicKey) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

/// 建立 SSH 会话、认证、绑定本地监听，并启动接受循环。
///
/// 返回 `(shutdown 发送端, 接受循环任务)`。该函数会在调用方提供的运行时内运行，
/// 连接/认证/绑定任一环节失败都会提前返回错误；成功时本地端口已可接收连接。
async fn setup_tunnel(
    settings: &SshSettings,
    remote_host: String,
    remote_port: u16,
    logger: Option<SshLogger>,
) -> RuboResult<(broadcast::Sender<()>, JoinHandle<()>, u16)> {
    // 让日志闭包持有 logger 的克隆（Arc 克隆廉价），避免与下方把 logger move 进
    // accept_loop 产生借用冲突。
    let logger_for_log = logger.clone();
    let log = move |m: &str| {
        if let Some(l) = &logger_for_log {
            l(m);
        }
    };

    let config = Arc::new(Config::default());
    let addr = (settings.host.clone(), settings.port);

    log(&format!(
        "正在建立 SSH TCP 连接: {}:{} ...",
        settings.host, settings.port
    ));
    let mut handle = connect(config, addr, ClientHandler)
        .await
        .map_err(|e| RuboError::Ssh(format!("SSH 连接失败（{}:{}）: {}", settings.host, settings.port, e)))?;
    log(&format!(
        "SSH TCP 连接已建立: {}:{}",
        settings.host, settings.port
    ));

    // 认证：优先按显式选择的认证方式；若选了「公钥」却没给私钥路径，
    // 且填了密码，则自动回退到密码认证（兼容历史/误配置的连接）。
    match settings.auth_method {
        SshAuthMethod::Password => {
            log("SSH 认证方式: 密码");
            authenticate_password(&mut handle, settings, &log).await?;
        }
        SshAuthMethod::PublicKey => {
            if settings.private_key_path.is_none() {
                if !settings.password.is_empty() {
                    log("提示: 认证方式选为「公钥」但未配置私钥路径，已自动回退到密码认证。");
                    authenticate_password(&mut handle, settings, &log).await?;
                } else {
                    return Err(RuboError::Ssh(
                        "公钥认证需要私钥路径（private_key_path）；如需密码认证请在认证方式中选择「密码」。".into(),
                    ));
                }
            } else {
                log("SSH 认证方式: 公钥");
                authenticate_publickey(&mut handle, settings, &log).await?;
            }
        }
    }
    log("SSH 认证成功。");

    // 绑定本地监听端口：端口 0 由操作系统随机分配一个空闲端口（每次连接都不同）。
    log("正在绑定本地监听端口 127.0.0.1:0（随机端口）...");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(|e| RuboError::Ssh(format!("无法在本地绑定随机端口监听: {e}")))?;
    let local_port = listener
        .local_addr()
        .map(|a| a.port())
        .unwrap_or(0);
    log(&format!(
        "本地监听已绑定: 127.0.0.1:{}（转发至 {}:{}）",
        local_port, remote_host, remote_port
    ));

    let (shutdown_tx, shutdown_rx) = broadcast::channel::<()>(4);
    let remote_port = remote_port as u32;

    // Handle 非 Clone，用 Arc 在 accept 循环与每个连接任务间共享会话。
    let session = Arc::new(handle);

    let task = tokio::spawn(accept_loop(
        listener,
        session,
        remote_host,
        remote_port,
        shutdown_rx,
        logger,
    ));

    log("SSH 隧道已就绪，可连接本地端口。");
    Ok((shutdown_tx, task, local_port))
}

/// 密码认证。
async fn authenticate_password<F>(
    handle: &mut Handle<ClientHandler>,
    settings: &SshSettings,
    log: &F,
) -> RuboResult<()>
where
    F: Fn(&str),
{
    log(&format!("正在用密码认证用户 {} ...", settings.user));
    let ok = handle
        .authenticate_password(&settings.user, &settings.password)
        .await
        .map_err(|e| RuboError::Ssh(format!("SSH 密码认证出错: {e}")))?;
    if !ok {
        return Err(RuboError::Ssh("SSH 密码认证被拒绝，请检查用户名/密码。".into()));
    }
    Ok(())
}

/// 公钥认证。
async fn authenticate_publickey<F>(
    handle: &mut Handle<ClientHandler>,
    settings: &SshSettings,
    log: &F,
) -> RuboResult<()>
where
    F: Fn(&str),
{
    let key_path = settings
        .private_key_path
        .as_ref()
        .ok_or_else(|| RuboError::Ssh("公钥认证需要私钥路径（private_key_path）。".into()))?;
    log(&format!("正在加载私钥: {key_path}"));
    let key = load_secret_key(key_path, settings.passphrase.as_deref())
        .map_err(|e| RuboError::Ssh(format!("加载私钥失败（{key_path}）: {e}")))?;
    log(&format!("正在用公钥认证用户 {} ...", settings.user));
    let ok = handle
        .authenticate_publickey(&settings.user, Arc::new(key))
        .await
        .map_err(|e| RuboError::Ssh(format!("SSH 公钥认证出错: {e}")))?;
    if !ok {
        return Err(RuboError::Ssh("SSH 公钥认证被拒绝，请检查私钥/证书与用户名。".into()));
    }
    Ok(())
}

/// 接受本地连接并为每条连接开 `direct-tcpip` 通道做双向转发。
async fn accept_loop(
    listener: TcpListener,
    session: Arc<Handle<ClientHandler>>,
    remote_host: String,
    remote_port: u32,
    mut shutdown: broadcast::Receiver<()>,
    logger: Option<SshLogger>,
) {
    loop {
        tokio::select! {
            // 收到关闭信号，退出循环（随后 session Handle 释放，SSH 会话结束）。
            _ = shutdown.recv() => break,
            accepted = listener.accept() => {
                match accepted {
                    Ok((mut local_stream, _peer)) => {
                        let session = Arc::clone(&session);
                        let remote_host = remote_host.clone();
                        let logger = logger.clone();
                        let src = local_stream
                            .local_addr()
                            .unwrap_or_else(|_| SocketAddr::from((Ipv4Addr::LOCALHOST, 0)));
                        tokio::spawn(async move {
                            let log = |m: &str| {
                                if let Some(l) = &logger {
                                    l(m);
                                }
                            };
                            log(&format!(
                                "新建本地连接 {src} → 正在打开 direct-tcpip 通道到 {remote_host}:{remote_port}"
                            ));
                            match session
                                .channel_open_direct_tcpip(
                                    remote_host.clone(),
                                    remote_port,
                                    src.ip().to_string(),
                                    src.port() as u32,
                                )
                                .await
                            {
                                Ok(channel) => {
                                    let mut channel_stream = channel.into_stream();
                                    log("direct-tcpip 通道已建立，开始双向转发。");
                                    let _ =
                                        copy_bidirectional(&mut local_stream, &mut channel_stream)
                                            .await;
                                    log("连接转发结束。");
                                }
                                Err(e) => {
                                    log(&format!(
                                        "打开 direct-tcpip 通道失败: {e}（远端 {remote_host}:{remote_port} 不可达，或 SSH 会话/通道已关闭）"
                                    ));
                                }
                            }
                        });
                    }
                    Err(_) => {
                        // 监听套接字出错，停止接受。
                        break;
                    }
                }
            }
        }
    }
}
