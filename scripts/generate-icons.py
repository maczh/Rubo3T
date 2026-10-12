#!/usr/bin/env python3
"""从 crates/app/resources/icons/logo.png 生成 OS 级图标格式。

产物：
- logo-256x256.png：非交错 RGBA，供 eframe 窗口图标使用。
- logo.ico：Windows 可执行文件图标（16/32/48/256 px）。
- logo.icns：macOS .app 图标（16/32/64/128/256/512/1024 px）。

运行要求：
- Pillow
- macOS 内置 iconutil（用于 .icns，其他平台可跳过）。
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path


def resource_dir() -> Path:
    return Path(__file__).resolve().parent.parent / "crates" / "app" / "resources" / "icons"


def resize_pil(src: Path, size: int) -> object:
    from PIL import Image

    im = Image.open(src).convert("RGBA")
    if im.size != (size, size):
        im = im.resize((size, size), Image.Resampling.LANCZOS)
    return im


def generate_clean_256(src: Path, dst: Path) -> None:
    im = resize_pil(src, 256)
    im.save(dst, "PNG", interlace=False)
    print(f"[ok] {dst.relative_to(Path(__file__).resolve().parent.parent)}")


def generate_ico(src: Path, dst: Path) -> None:
    from PIL import Image

    # Pillow 的 ICO writer 支持 `sizes` 参数：对单一源图按给定尺寸生成
    # 多分辨率图标（并自动处理 BMP/PNG 格式）。
    src_im = Image.open(src).convert("RGBA")
    src_im.save(
        dst,
        format="ICO",
        sizes=[(16, 16), (32, 32), (48, 48), (256, 256)],
    )
    print(f"[ok] {dst.relative_to(Path(__file__).resolve().parent.parent)}")


def generate_icns(src: Path, dst: Path) -> None:
    """用 macOS iconutil 生成 .icns；非 macOS 直接跳过。"""
    if sys.platform != "darwin":
        print("[skip] logo.icns 只能在 macOS 上用 iconutil 生成")
        return

    iconset = dst.with_suffix(".iconset")
    if iconset.exists():
        shutil.rmtree(iconset)
    iconset.mkdir(parents=True)

    sizes = [16, 32, 64, 128, 256, 512, 1024]
    for size in sizes:
        # 1x
        im = resize_pil(src, size)
        if size <= 512:
            im.save(iconset / f"icon_{size}x{size}.png", "PNG", interlace=False)
        # 2x（对 512 及以上不提供 2x，因为 1024@2x=2048 超出常见源图）
        if size <= 512:
            im2x = resize_pil(src, size * 2)
            im2x.save(iconset / f"icon_{size}x{size}@2x.png", "PNG", interlace=False)

    try:
        subprocess.run(["iconutil", "-c", "icns", str(iconset)], check=True)
        print(f"[ok] {dst.relative_to(Path(__file__).resolve().parent.parent)}")
    finally:
        shutil.rmtree(iconset, ignore_errors=True)


def generate_png_sizes(src: Path, dst_dir: Path) -> None:
    """生成 cargo-bundle / cargo-packager 元数据常用的图标尺寸 PNG。"""
    from PIL import Image

    for size in [16, 32, 128, 256]:
        im = Image.open(src).convert("RGBA")
        if im.size != (size, size):
            im = im.resize((size, size), Image.Resampling.LANCZOS)
        dst = dst_dir / f"logo-{size}x{size}.png"
        im.save(dst, "PNG", interlace=False)
        print(f"[ok] {dst.relative_to(Path(__file__).resolve().parent.parent)}")


def main() -> int:
    src = resource_dir() / "logo.png"
    if not src.exists():
        print(f"[err] source icon not found: {src}", file=sys.stderr)
        return 1

    dst_dir = resource_dir()
    generate_png_sizes(src, dst_dir)
    generate_clean_256(src, dst_dir / "logo-256x256.png")
    generate_ico(src, dst_dir / "logo.ico")
    generate_icns(src, dst_dir / "logo.icns")
    return 0


if __name__ == "__main__":
    sys.exit(main())
