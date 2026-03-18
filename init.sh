#!/bin/bash
# tiny 项目环境初始化脚本
# 用于快速恢复开发环境

set -e

echo "=== tiny 项目环境初始化 ==="

# 检查 Rust 工具链
echo "检查 Rust 工具链..."
rustc --version
cargo --version

# 读取 rust-toolchain
if [ -f rust-toolchain ]; then
    echo "使用工具链：$(cat rust-toolchain)"
fi

# 构建项目（debug 模式快速检查）
echo "构建项目..."
cargo check --workspace

# 运行测试（如有）
echo "运行测试..."
cargo test --workspace --lib 2>/dev/null || echo "无库测试或测试失败"

echo "=== 环境初始化完成 ==="
