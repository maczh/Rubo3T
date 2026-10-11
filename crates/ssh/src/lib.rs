//! `rubo3t-ssh` — 通过系统 `ssh` 建立本地端口转发隧道。
//!
//! 用于经跳板机访问 AWS DocumentDB / RDS / 内网 MongoDB 等场景。
//! 采用系统 `ssh` 二进制（Windows 自带 OpenSSH，或 Git Bash 中的 ssh），
//! 因此本 crate 零额外依赖、编译极快、跨平台可靠。
//!
//! - 密钥认证：完全支持（推荐）。
//! - 密码认证：若系统存在 `sshpass` 则自动使用；否则给出明确指引。
//!
//! 隧道进程在前台以 `-N -L` 方式常驻，由 `Tunnel` 持有子进程句柄，
//! `Drop` 时自动结束，避免残留 ssh 进程。

use std::process::{Command, Stdio};

use rubo3t_core::error::{RuboError, RuboResult};
use rubo3t_core::settings::{SshAuthMethod, SshSettings};

/// 本地端口转发隧道。
pub struct Tunnel {
    child: Option<std::process::Child>,
    local_port: u16,
}

impl Tunnel {
    /// 根据连接设置启动隧道。
    ///
    /// 若 `settings.enabled` 为 false，则返回一个「空隧道」（不实际启动 ssh），
    /// 调用方据此把 Mongo 连接目标改写为本地转发端口。
    pub fn start(settings: &SshSettings) -> RuboResult<Tunnel> {
        if !settings.enabled {
            return Ok(Tunnel {
                child: None,
                local_port: settings.local_port,
            });
        }

        let local_port = settings.local_port;
        let forward = format!(
            "{}:{}:{}",
            settings.local_port, settings.remote_host, settings.remote_port
        );
        let target = format!("{}@{}", settings.user, settings.host);

        // 公共 ssh 参数
        let mut ssh_args = vec![
            "-N".to_string(),
            "-o".to_string(),
            "StrictHostKeyChecking=no".to_string(),
            "-o".to_string(),
            "UserKnownHostsFile=/dev/null".to_string(),
            "-o".to_string(),
            "LogLevel=ERROR".to_string(),
            "-L".to_string(),
            forward,
            "-p".to_string(),
            settings.port.to_string(),
            target,
        ];

        let command = match settings.auth_method {
            SshAuthMethod::PublicKey => {
                let mut c = Command::new("ssh");
                if let Some(key) = &settings.private_key_path {
                    ssh_args.insert(0, key.clone());
                    ssh_args.insert(0, "-i".to_string());
                }
                c.args(&ssh_args);
                c
            }
            SshAuthMethod::Password => {
                if !sshpass_available() {
                    return Err(RuboError::Ssh(
                        "密码认证需要 sshpass 且未在 PATH 中发现。请改用密钥认证，或在系统安装 sshpass 后重试。".into(),
                    ));
                }
                let mut cmd = Command::new("sshpass");
                cmd.arg("-p").arg(&settings.password).arg("ssh");
                cmd.args(&ssh_args);
                cmd
            }
        };

        spawn_tunnel(command, local_port)
    }

    /// 本地监听端口（Mongo 连接应指向此端口）。
    pub fn local_port(&self) -> u16 {
        self.local_port
    }

    /// 主动停止隧道（Drop 时也会自动停止）。
    pub fn stop(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }

    /// 隧道是否实际在运行（而非空隧道）。
    pub fn is_active(&self) -> bool {
        self.child.is_some()
    }
}

impl Drop for Tunnel {
    fn drop(&mut self) {
        self.stop();
    }
}

fn spawn_tunnel(mut command: Command, local_port: u16) -> RuboResult<Tunnel> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let child = command.spawn().map_err(|e| {
        RuboError::Ssh(format!(
            "无法启动 ssh 隧道: {e}。请确认系统已安装 ssh 且位于 PATH（Windows 可启用 OpenSSH 或安装 Git Bash）。"
        ))
    })?;
    Ok(Tunnel {
        child: Some(child),
        local_port,
    })
}

/// 检测系统是否存在 `sshpass`。
fn sshpass_available() -> bool {
    Command::new("sshpass")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}
