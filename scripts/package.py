#!/usr/bin/env python3
"""Rubo3T 跨平台安装包打包脚本。

用法：
    python scripts/package.py
    python scripts/package.py --format dmg
    python scripts/package.py --format rpm
    python scripts/package.py --no-build

行为：
1. 默认先执行 `cargo build --release`。
2. 检查并安装 `cargo-packager`（一次性）。
3. 调用 `cargo packager --release` 生成当前平台对应安装包。
4. Linux 额外调用 `cargo-generate-rpm` 生成 .rpm。
5. 打印产物路径。

平台/产物对应关系：
- macOS: .app, .dmg
- Linux: .deb, .rpm, .AppImage
- Windows: .msi, -setup.exe (NSIS)

前置依赖（由 cargo-packager / cargo-generate-rpm 按需调用）：
- macOS: 内置 hdiutil / iconutil（无需额外安装）。
- Linux: dpkg-deb（.deb）、rpmbuild（.rpm）、appimagetool（.AppImage）。
- Windows: WiX Toolset（.msi）、NSIS（-setup.exe）。
"""

from __future__ import annotations

import argparse
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PACKAGER_CONFIG = ROOT / "crates" / "app" / "Cargo.toml"
BUNDLE_DIR = ROOT / "target" / "release" / "bundle"

# 与用户需求对应：
# - macOS: .app (cargo-packager) + .dmg (hdiutil wrapper)
# - Linux: .deb/.AppImage (cargo-packager) + .rpm (cargo-generate-rpm)
# - Windows: .msi (cargo-packager wix) + -setup.exe (cargo-packager nsis)
DEFAULT_ARTIFACTS: dict[str, list[str]] = {
    "Darwin": ["app", "dmg"],
    "Linux": ["deb", "appimage", "rpm"],
    "Windows": ["wix", "nsis"],
}
# cargo-packager 原生支持的格式
PACKAGER_FORMATS = {"app", "dmg", "deb", "appimage", "wix", "nsis"}


def run(cmd: list[str], check: bool = True) -> subprocess.CompletedProcess:
    print("\n$", " ".join(cmd))
    return subprocess.run(cmd, cwd=ROOT, check=check, text=True)


def shell(cmd: str, check: bool = True) -> subprocess.CompletedProcess:
    print("\n$", cmd)
    return subprocess.run(cmd, cwd=ROOT, shell=True, check=check, text=True)


def cargo_packager_available() -> bool:
    """检查 `cargo packager` 子命令是否可用（PATH 中或 cargo 已安装）。"""
    if shutil.which("cargo-packager"):
        return True
    result = subprocess.run(
        ["cargo", "packager", "--version"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    return result.returncode == 0


def install_cargo_packager() -> None:
    """通过 cargo install 安装 cargo-packager。"""
    print("cargo-packager 未安装，正在通过 `cargo install cargo-packager --locked` 安装...")
    print("（首次安装可能需要几分钟）")
    start = time.time()
    try:
        run(["cargo", "install", "cargo-packager", "--locked"])
    except subprocess.CalledProcessError as e:
        print(
            f"安装 cargo-packager 失败（耗时 {time.time()-start:.0f}s）。\n"
            "请确认网络可用，或手动执行：cargo install cargo-packager --locked",
            file=sys.stderr,
        )
        raise SystemExit(e.returncode) from e
    print(f"cargo-packager 安装完成（耗时 {time.time()-start:.0f}s）")


def build_release() -> None:
    print("==> 步骤 1: cargo build --release")
    run(["cargo", "build", "--release"])


def ensure_cargo_generate_rpm() -> None:
    """检查并安装 cargo-generate-rpm（仅在 Linux 生成 .rpm 时需要）。"""
    result = subprocess.run(
        ["cargo", "generate-rpm", "--version"],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        return
    print("cargo-generate-rpm 未安装，正在通过 `cargo install cargo-generate-rpm --locked` 安装...")
    start = time.time()
    try:
        run(["cargo", "install", "cargo-generate-rpm", "--locked"])
    except subprocess.CalledProcessError as e:
        print(
            f"安装 cargo-generate-rpm 失败（耗时 {time.time()-start:.0f}s）。\n"
            "请确认网络可用，或手动执行：cargo install cargo-generate-rpm --locked",
            file=sys.stderr,
        )
        raise SystemExit(e.returncode) from e
    print(f"cargo-generate-rpm 安装完成（耗时 {time.time()-start:.0f}s）")


def _split_comma(items: list[str] | None) -> list[str] | None:
    """支持 --format app,dmg 以及多次 --format app --format dmg 两种写法。"""
    if items is None:
        return None
    out: list[str] = []
    for item in items:
        out.extend(fmt.strip() for fmt in item.split(",") if fmt.strip())
    return out


def app_version() -> str:
    """从 workspace 根 Cargo.toml 的 [workspace.package] 读取版本号（单一数据源）。"""
    import tomllib

    with open(ROOT / "Cargo.toml", "rb") as f:
        data = tomllib.load(f)
    return data["workspace"]["package"]["version"]


def _dmg_filename() -> str:
    arch = platform.machine().lower().replace("amd64", "x64")
    return f"Rubo3T_{app_version()}_{arch}.dmg"


def create_macos_dmg() -> None:
    """用 hdiutil 从已生成的 .app 制作 .dmg（无需 AppleScript，可在 CI/沙箱运行）。"""
    import tempfile

    app_src = BUNDLE_DIR / "Rubo3T.app"
    if not app_src.exists():
        print("[err] 找不到 Rubo3T.app，无法制作 .dmg", file=sys.stderr)
        raise SystemExit(1)

    dmg_dst = BUNDLE_DIR / _dmg_filename()
    with tempfile.TemporaryDirectory(prefix="rubo3t-dmg-") as tmp:
        staging = Path(tmp) / "staging"
        staging.mkdir()
        # 复制 .app 并创建 Applications 软链
        shutil.copytree(app_src, staging / "Rubo3T.app", symlinks=True)
        applications_link = staging / "Applications"
        if not applications_link.exists():
            applications_link.symlink_to("/Applications")

        print(f"==> 步骤 dmg: 使用 hdiutil 创建 {dmg_dst.name}")
        # 创建压缩的 UDZO 格式 .dmg
        run(
            [
                "hdiutil",
                "create",
                "-volname",
                "Rubo3T",
                "-srcfolder",
                str(staging),
                "-ov",
                "-format",
                "UDZO",
                str(dmg_dst),
            ],
            check=True,
        )


def package_release(formats: list[str] | None) -> None:
    formats = _split_comma(formats)
    artifacts = formats or DEFAULT_ARTIFACTS.get(platform.system(), [])
    if not artifacts:
        print(f"[err] 不支持的平台：{platform.system()}", file=sys.stderr)
        raise SystemExit(1)

    print("==> 步骤 2: 检查 cargo-packager")
    if not cargo_packager_available():
        install_cargo_packager()

    # 需要 cargo-packager 处理的格式（macOS 的 dmg 走 hdiutil，不交给 cargo-packager，
    # 以避免 create-dmg 的 AppleScript 在 CI/沙箱环境下被权限拦截）
    packager_fmts = [f for f in artifacts if f in PACKAGER_FORMATS and f != "dmg"]
    # 若只选了 dmg，仍需要 cargo-packager 先构建 .app
    if "dmg" in artifacts and "app" not in packager_fmts:
        packager_fmts.append("app")

    if packager_fmts:
        print(f"==> 步骤 3: cargo packager --release --formats {','.join(packager_fmts)}")
        run(
            ["cargo", "packager", "--release", "--verbose"]
            + ["--formats", ",".join(packager_fmts)]
        )

    # macOS .dmg 独立生成（避开 create-dmg 的 AppleScript 权限问题）
    if platform.system() == "Darwin" and (formats is None or "dmg" in artifacts):
        create_macos_dmg()

    # Linux .rpm 独立生成
    if platform.system() == "Linux" and (formats is None or "rpm" in artifacts):
        print("==> 步骤 rpm: 生成 .rpm（cargo-generate-rpm）")
        ensure_cargo_generate_rpm()
        # cargo-generate-rpm 默认把 .rpm 放在 target/release/generate-rpm/
        # 注意：该工具的 `-p` 参数是「包目录路径」而非包名（内部做 Path::new(p).join("Cargo.toml")），
        # 本仓库 rubo3t 包位于 crates/app，故必须传 `crates/app`。
        # 另：不可传 `--target <triple>`——它会把资产路径解析成 target/<triple>/release/，
        # 与 Makefile `cargo build --release`（主机构建，产物在 target/release/）不一致；
        # 用 `--arch x86_64` 仅指定 rpm 架构标签，不影响二进制资产定位。
        run(["cargo", "generate-rpm", "-p", "crates/app", "--arch", "x86_64"])


def list_artifacts() -> list[Path]:
    found: list[Path] = []
    # cargo-packager 产物
    if BUNDLE_DIR.exists():
        found.extend(p for p in BUNDLE_DIR.rglob("*") if p.is_file())
    # cargo-generate-rpm 产物（.rpm 不进 bundle 目录）。
    # 实际输出目录为 target/generate-rpm/（未指定 --target；若指定 --target 则为
    # target/<triple>/generate-rpm/，见 generate-rpm 的 determine_output_dir），
    # 并非 target/release/generate-rpm/。
    rpm_dirs = [ROOT / "target" / "generate-rpm", *ROOT.glob("target/*/generate-rpm")]
    for rpm_dir in rpm_dirs:
        if rpm_dir.is_dir():
            found.extend(p for p in rpm_dir.rglob("*.rpm") if p.is_file())
    return sorted(found)


def print_artifacts() -> None:
    artifacts = list_artifacts()
    if not artifacts:
        print("\n未在 target/release/{bundle,generate-rpm} 下找到产物，请检查打包输出。")
        return

    print("\n==================== 安装包产物 ====================")
    for p in artifacts:
        size = p.stat().st_size
        print(f"  {p.relative_to(ROOT)} ({_human(size)})")
    print("=====================================================")


def _human(n: int) -> str:
    for unit in ("B", "KiB", "MiB", "GiB"):
        if n < 1024:
            return f"{n:.1f} {unit}" if isinstance(n, float) else f"{n} {unit}"
        n /= 1024
    return f"{n:.1f} TiB"


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Rubo3T 跨平台安装包打包脚本",
    )
    parser.add_argument(
        "--no-build",
        action="store_true",
        help="跳过 `cargo build --release`（假设已编译）",
    )
    parser.add_argument(
        "--format",
        action="append",
        dest="formats",
        help="只生成指定格式，可多次使用（如 app,dmg, deb, rpm, wix, nsis, appimage）",
    )
    args = parser.parse_args()

    if not PACKAGER_CONFIG.exists():
        print(f"[err] 找不到打包配置：{PACKAGER_CONFIG}", file=sys.stderr)
        return 1

    if not args.no_build:
        build_release()

    package_release(args.formats)
    print_artifacts()
    return 0


if __name__ == "__main__":
    sys.exit(main())
