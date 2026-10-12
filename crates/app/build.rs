//! 构建脚本：
//! - 监听资源图标文件变化，变更时触发重新构建。
//! - 在 Windows 目标下通过 `embed-resource` 将 `logo.ico` 嵌入到可执行文件资源段，
//!   使 Windows 资源管理器 / 任务栏 / 安装包能看到 Robo3T 图标。

use std::env;

fn main() {
    // 资源变更时重新运行构建脚本。
    println!("cargo:rerun-if-changed=resources/icons/logo.rc");
    println!("cargo:rerun-if-changed=resources/icons/logo.ico");

    // 仅在 Windows 目标下嵌入图标资源（cross-compile 时也按目标 OS 判断）。
    if env::var("CARGO_CFG_TARGET_OS").ok().as_deref() == Some("windows") {
        embed_resource::compile("resources/icons/logo.rc", embed_resource::NONE);
    }
}
