#!/bin/bash

set -e  # 遇到错误立即退出

echo "🚀 开始构建打包流程..."

# 1. 构建 Release 版本
echo "📦 正在编译 Release 版本 (cargo build --release)..."
cargo build --release

# 2. 创建发布目录
DIST_DIR="gitx_dist"
echo "📂 创建发布目录: $DIST_DIR"
rm -rf $DIST_DIR
mkdir -p $DIST_DIR

# 3. 复制文件
echo "cp 正在复制文件..."

# 复制可执行文件
if [ -f "target/release/gitx" ]; then
    cp target/release/gitx $DIST_DIR/
    echo "  ✅ gitx 可执行文件已复制"
else
    echo "  ❌ 错误: 未找到 target/release/gitx"
    exit 1
fi

# 复制配置文件
if [ -f "config.toml" ]; then
    cp config.toml $DIST_DIR/
    echo "  ✅ config.toml 已复制"
else
    echo "  ⚠️ 警告: 未找到 config.toml，将不包含在包中"
fi

# 复制静态资源目录
if [ -d "statics" ]; then
    cp -r statics $DIST_DIR/
    echo "  ✅ statics/ 目录已复制"
else
    echo "  ❌ 错误: 未找到 statics 目录"
    exit 1
fi

# 4. 打包压缩
ARCHIVE_NAME="gitx_release_$(date +%Y%m%d).tar.gz"
echo "🗜️  正在压缩打包为 $ARCHIVE_NAME..."
tar -czf $ARCHIVE_NAME $DIST_DIR

echo "✨ 打包完成!"
echo "----------------------------------------"
echo "生成的压缩包: $ARCHIVE_NAME"
echo "解压后的目录: $DIST_DIR"
echo ""
echo "部署步骤:"
echo "1. 将 $ARCHIVE_NAME 上传到目标机器"
echo "2. 解压: tar -xzf $ARCHIVE_NAME"
echo "3. 进入目录: cd $DIST_DIR"
echo "4. 运行: ./gitx"
echo "----------------------------------------"
