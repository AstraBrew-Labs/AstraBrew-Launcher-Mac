#!/bin/bash
#
# AstraBrew Launcher（macOS）构建脚本 —— 从旧版仓库迁移并适配 iced 版。
#
# 用法：
#   ./build.sh
#
# 构建渠道由 Cargo.toml 里的 `[package.metadata.astrabrew] beta` 决定：
#   beta = true  → 测试版（左上角显示「测试版」标记，tag 用 `beta-v{版本}`）
#   beta = false 或省略 → 正式版（无标记，tag 用 `v{版本}`）
#
# 渠道决定三件事：
#   1. 二进制里注入的 `ASTRA_BUILD_CHANNEL`（决定界面左上角是否显示「测试版」标记）；
#   2. 生成的 `latest.json` 里下载地址使用的 tag（`beta-v{版本}` 或 `v{版本}`）；
#   3. `latest.json` 的说明文案。
#
# 产物（dist-packages/）：
#   AstraBrew-Launcher_aarch64.app.tar.gz      ARM64 更新包
#   AstraBrew-Launcher_universal.app.tar.gz    Universal 更新包（自动更新的安装目标）
#   AstraBrew-Launcher_aarch64.dmg             ARM64 安装镜像
#   AstraBrew-Launcher_universal.dmg           Universal 安装镜像
#   latest.json                                更新清单（需要签名私钥 keys/update_key.pem）
#   AstraBrew-Launcher_universal.app.tar.gz.sig  更新包签名（同上）
#
# 依赖：cargo-packager、以及 aarch64/x86_64 两个交叉编译目标。
#   安装：cargo install cargo-packager --locked
#   目标：rustup target add aarch64-apple-darwin x86_64-apple-darwin

set -euo pipefail

APP_NAME="AstraBrew Launcher"
BIN_NAME="astrabrew-launcher-mac"
DIST_DIR="dist"
OUT_DIR="dist-packages"
# 与 GitHub / GitCode 上的仓库路径保持一致，用于拼下载地址。
REPO="AstraBrew-Labs/AstraBrew-Launcher-Mac"

cd "$(dirname "$0")"

# ── 构建渠道（读 Cargo.toml，不用命令行参数）────────────────────────────────
# 取 [package.metadata.astrabrew] 块内的 `beta = ...`，缺省按正式版。
# 注意：不能放进 [package.metadata.packager]，cargo-packager 会拒绝未知字段。
BETA_RAW=$(sed -n '/^\[package\.metadata\.astrabrew\]/,/^\[/p' Cargo.toml \
           | grep -m1 '^[[:space:]]*beta[[:space:]]*=' \
           | sed 's/.*=[[:space:]]*//; s/[[:space:]]*#.*//')
case "$BETA_RAW" in
  true|True|TRUE)
    CHANNEL="beta"; TAG_PREFIX="beta-v"; MANIFEST_NOTES="测试版本发布" ;;
  *)
    CHANNEL="release"; TAG_PREFIX="v"; MANIFEST_NOTES="新版本发布" ;;
esac

VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
TAG="${TAG_PREFIX}${VERSION}"

echo "渠道：${CHANNEL}（tag=${TAG}）"
echo "===== 清理 ====="
rm -rf "$DIST_DIR" "$OUT_DIR"
mkdir -p "$OUT_DIR" target/current/release target/universal/release

echo "===== 编译 ARM64 ====="
ASTRA_BUILD_CHANNEL="$CHANNEL" cargo build --release --target aarch64-apple-darwin

echo "===== 编译 x86_64 ====="
ASTRA_BUILD_CHANNEL="$CHANNEL" cargo build --release --target x86_64-apple-darwin

echo "===== 合并 Universal Binary ====="
lipo \
  target/aarch64-apple-darwin/release/$BIN_NAME \
  target/x86_64-apple-darwin/release/$BIN_NAME \
  -create \
  -output target/universal/release/$BIN_NAME
file target/universal/release/$BIN_NAME

# cargo-packager 只打包、不编译：二进制必须预先放到 binaries_dir。
# 打包两次（先 ARM64、再 Universal），各自产物放进 dist-packages。
package_one() {
  local arch_label="$1"
  local binary="$2"

  cp "$binary" "target/current/release/$BIN_NAME"
  cargo packager --release

  local app
  app=$(find "$DIST_DIR" -name "*.app" | head -n1)
  local dmg
  dmg=$(find "$DIST_DIR" -name "*.dmg" | head -n1)

  if [[ -z "$app" ]]; then
    echo "未找到 .app 产物" >&2; find "$DIST_DIR" 2>/dev/null; exit 1
  fi

  # 去掉 cargo-packager 产生的 ad-hoc 签名，保持与旧版一致的裸包分发方式。
  codesign --remove-signature "$app" 2>/dev/null || true
  xattr -cr "$app"

  tar czf "$OUT_DIR/AstraBrew-Launcher_${arch_label}.app.tar.gz" \
    -C "$(dirname "$app")" "$(basename "$app")"

  if [[ -n "$dmg" ]]; then
    mv "$dmg" "$OUT_DIR/AstraBrew-Launcher_${arch_label}.dmg"
  fi

  rm -rf "$DIST_DIR"/*
}

echo "===== 打包 ARM64 ====="
package_one "aarch64" "target/aarch64-apple-darwin/release/$BIN_NAME"

echo "===== 打包 Universal ====="
package_one "universal" "target/universal/release/$BIN_NAME"

echo ""
echo "===== 打包完成 ====="
ls -lh "$OUT_DIR"

# ── 签名更新包并生成 latest.json（需要 keys/update_key.pem）──────────────────
if [[ -f keys/update_key.pem ]]; then
  echo "===== 签名更新包 ====="
  cargo packager signer sign \
    --private-key keys/update_key.pem \
    "$OUT_DIR/AstraBrew-Launcher_universal.app.tar.gz"

  echo "===== 生成 latest.json（tag=${TAG}）====="
  SIGNATURE=$(tr -d '\n' < "$OUT_DIR/AstraBrew-Launcher_universal.app.tar.gz.sig")
  cat > "$OUT_DIR/latest.json" <<EOF
{
  "version": "$VERSION",
  "notes": "$MANIFEST_NOTES",
  "pub_date": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "platforms": {
    "darwin-universal": {
      "signature": "$SIGNATURE",
      "url": "https://github.com/$REPO/releases/download/$TAG/AstraBrew-Launcher_universal.app.tar.gz",
      "format": "app"
    }
  }
}
EOF
  echo "已生成 $OUT_DIR/latest.json（含签名）"
else
  echo ""
  echo "⚠ 未找到 keys/update_key.pem，跳过签名与 latest.json 生成。"
  echo "  要发布可自动更新的版本，请把签名私钥放到 keys/update_key.pem 后重新运行。"
fi

echo ""
echo "===== 发布提示 ====="
echo "版本：$VERSION   渠道：$CHANNEL   发布 tag：$TAG"
echo "发布命令："
echo "  gh release create \"$TAG\" --title \"$TAG\" \\"
echo "    \"$OUT_DIR/AstraBrew-Launcher_universal.app.tar.gz\" \\"
echo "    \"$OUT_DIR/AstraBrew-Launcher_universal.app.tar.gz.sig\" \\"
echo "    \"$OUT_DIR/AstraBrew-Launcher_aarch64.dmg\" \\"
echo "    \"$OUT_DIR/AstraBrew-Launcher_universal.dmg\" \\"
echo "    \"$OUT_DIR/latest.json\""
