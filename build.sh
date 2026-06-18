#!/bin/bash

# 设置遇到错误即退出
set -e

# 定义项目名称和输出目录
APP_NAME="ai-term"
DIST_DIR="dist"

echo "🚀 开始打包 $APP_NAME (无 Docker 模式) ..."

# 1. 检查 Rust 环境
if ! command -v cargo &> /dev/null; then
    echo "❌ 错误: 未安装 cargo。请先安装 Rust 环境 (https://rustup.rs/)"
    exit 1
fi

# 2. 检查 Homebrew (用于安装交叉编译工具链)
if ! command -v brew &> /dev/null; then
    echo "❌ 错误: 未安装 Homebrew。macOS 本地交叉编译需要依赖 Homebrew。"
    exit 1
fi

# 3. 安装并配置 Linux musl 交叉编译工具链
if ! command -v x86_64-linux-musl-gcc &> /dev/null; then
    echo "⚠️ 未找到 x86_64-linux-musl-gcc 工具链，正在使用 Homebrew 安装..."
    brew install filosottile/musl-cross/musl-cross
fi

# 4. 添加 Rust target
echo "🔧 确保安装了 Linux musl target..."
rustup target add x86_64-unknown-linux-musl

# 5. 自动配置 Cargo Linker
mkdir -p .cargo
if [ ! -f .cargo/config.toml ]; then
    echo "🔧 配置 Cargo 交叉编译链接器..."
    cat <<EOF > .cargo/config.toml
[target.x86_64-unknown-linux-musl]
linker = "x86_64-linux-musl-gcc"
EOF
fi

# 清理并创建发布目录
echo "🧹 清理旧的发布目录..."
rm -rf "$DIST_DIR"
mkdir -p "$DIST_DIR"

# 6. 编译 macOS 版本 (原生)
echo "🍏 正在编译 macOS 版本..."
cargo build --release
cp target/release/$APP_NAME "$DIST_DIR/${APP_NAME}-macos"
echo "✅ macOS 版本编译完成！"

# 7. 编译 Linux 版本 (通过本地工具链交叉编译)
echo "🐧 正在编译 Linux (x86_64-musl) 静态链接版本..."
# 注意：由于依赖 ssh2 和 openssl，需要强制启用静态链接和使用 musl-gcc
CC_x86_64_unknown_linux_musl=x86_64-linux-musl-gcc \
CXX_x86_64_unknown_linux_musl=x86_64-linux-musl-g++ \
cargo build --target x86_64-unknown-linux-musl --release

cp target/x86_64-unknown-linux-musl/release/$APP_NAME "$DIST_DIR/${APP_NAME}-linux-x86_64"
echo "✅ Linux 版本编译完成！"

# 8. 完成并展示结果
echo "🎉 所有打包任务已完成！"
echo "📁 产物已保存至 $DIST_DIR 目录:"
ls -lh "$DIST_DIR"
