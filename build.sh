#!/bin/bash

set -e

APP_NAME="AstraBrew Launcher"
BIN_NAME="astrabrew-launcher-mac"

echo "===== 清理 ====="
rm -rf dist
rm -rf dist-packages
mkdir -p dist-packages
mkdir -p target/current/release
mkdir -p target/universal/release

echo "===== 编译 ARM64 ====="
cargo build --release --target aarch64-apple-darwin

echo "===== 编译 x86_64 ====="
cargo build --release --target x86_64-apple-darwin

echo "===== 合并 Universal Binary ====="
lipo \
target/aarch64-apple-darwin/release/$BIN_NAME \
target/x86_64-apple-darwin/release/$BIN_NAME \
-create \
-output target/universal/release/$BIN_NAME

file target/universal/release/$BIN_NAME

############################################
# ARM64
############################################

echo ""
echo "===== 打包 ARM64 ====="

cp \
target/aarch64-apple-darwin/release/$BIN_NAME \
target/current/release/$BIN_NAME

cargo packager --release

APP=$(find dist -name "*.app" | head -n1)
DMG=$(find dist -name "*.dmg" | head -n1)

codesign --remove-signature "$APP" 2>/dev/null || true
xattr -cr "$APP"

tar czf \
dist-packages/AstraBrew-Launcher_aarch64.app.tar.gz \
-C "$(dirname "$APP")" \
"$(basename "$APP")"

mv "$DMG" dist-packages/AstraBrew-Launcher_aarch64.dmg

rm -rf dist/*

############################################
# UNIVERSAL
############################################

echo ""
echo "===== 打包 Universal ====="

cp \
target/universal/release/$BIN_NAME \
target/current/release/$BIN_NAME

cargo packager --release

APP=$(find dist -name "*.app" | head -n1)
DMG=$(find dist -name "*.dmg" | head -n1)

codesign --remove-signature "$APP" 2>/dev/null || true
xattr -cr "$APP"

tar czf \
dist-packages/AstraBrew-Launcher_universal.app.tar.gz \
-C "$(dirname "$APP")" \
"$(basename "$APP")"

mv "$DMG" dist-packages/AstraBrew-Launcher_universal.dmg

rm -rf dist/*

echo ""
echo "======================================"
echo "打包完成"
echo ""

ls -lh dist-packages

echo ""
echo "生成文件："
echo "✓ AstraBrew-Launcher_aarch64.dmg"
echo "✓ AstraBrew-Launcher_aarch64.app.tar.gz"
echo "✓ AstraBrew-Launcher_universal.dmg"
echo "✓ AstraBrew-Launcher_universal.app.tar.gz"
echo "======================================"