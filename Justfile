# Rubo3T 构建与打包
# 要求：Rust、Python 3，以及各平台打包前置依赖（见 scripts/package.py 顶部注释）。

release:
    cargo build --release
    python3 scripts/package.py --no-build

package:
    python3 scripts/package.py

clean:
    cargo clean
