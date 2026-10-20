#!/bin/bash
set -euo pipefail

project_root="$(cd -- "$(dirname -- "$0")/.." && pwd)"
test_build_dir="$(mktemp -d "${TMPDIR:-/tmp}/astrabrew-state-checks.XXXXXX")"
trap 'rm -rf "$test_build_dir"' EXIT

xcrun --sdk macosx swiftc -parse-as-library \
  "$project_root/AstraBrew Launcher/Models/LauncherPage.swift" \
  "$project_root/AstraBrew Launcher/Models/TavernVersion.swift" \
  "$project_root/AstraBrew Launcher/ViewModels/AppState.swift" \
  "$project_root/Tests/LauncherStateChecks.swift" \
  -o "$test_build_dir/state-checks"
"$test_build_dir/state-checks"
