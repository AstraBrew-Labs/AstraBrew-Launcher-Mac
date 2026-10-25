import SwiftUI

/// UI fixtures only. These states never detect or change the host environment.
enum RuntimeEnvironmentPreviewScenario: String, CaseIterable, Identifiable {
    case missingHomebrew, homebrewOnly, ready, withPM2

    var id: Self { self }

    var title: String {
        switch self {
        case .missingHomebrew: "尚未安装 Homebrew"
        case .homebrewOnly: "已安装 Homebrew，缺少 Node.js"
        case .ready: "基础环境已就绪"
        case .withPM2: "环境已就绪，含 PM2"
        }
    }

    var hasHomebrew: Bool { self != .missingHomebrew }
    var hasNode: Bool { self == .ready || self == .withPM2 }
    var hasPM2: Bool { self == .withPM2 }
    var hasGit: Bool { hasNode }
}

private enum NPMRegistryPreview: String, CaseIterable, Identifiable {
    case npmmirror, huawei, pku, official

    var id: Self { self }

    var title: String {
        switch self {
        case .npmmirror: "npmmirror"
        case .huawei: "华为"
        case .pku: "北大"
        case .official: "官方"
        }
    }
}

struct RuntimeEnvironmentSettingsView: View {
    @State private var scenario: RuntimeEnvironmentPreviewScenario
    @State private var registry = NPMRegistryPreview.npmmirror

    init(previewScenario: RuntimeEnvironmentPreviewScenario = .ready) {
        _scenario = State(initialValue: previewScenario)
    }

    var body: some View {
        Form {
            Section {
                HStack(spacing: 14) {
                    RuntimeEnvironmentIcon(symbol: "terminal", color: .accentColor)
                    VStack(alignment: .leading, spacing: 5) {
                        Text("运行酒馆的环境")
                            .font(.headline)
                        Text("准备运行工具、选择软件源与后台保活方式。")
                            .font(.callout)
                            .foregroundStyle(.secondary)
                    }
                    Spacer(minLength: 12)
                    Text("界面原型")
                        .font(.caption.weight(.medium))
                        .foregroundStyle(Color.accentColor)
                        .padding(.horizontal, 10)
                        .padding(.vertical, 5)
                        .background(Color.accentColor.opacity(0.1), in: Capsule())
                }
                .padding(.vertical, 4)
            }

            Section {
                RuntimeEnvironmentToolRow(
                    title: "Homebrew",
                    detail: "安装其他运行环境的前置工具。",
                    symbol: "shippingbox",
                    color: .orange,
                    isInstalled: scenario.hasHomebrew,
                    requirement: "前置必需"
                )

                if scenario.hasHomebrew {
                    RuntimeEnvironmentToolRow(
                        title: "Node.js",
                        detail: "运行酒馆的 JavaScript 环境，PM2 的前置依赖。",
                        symbol: "hexagon",
                        color: .green,
                        isInstalled: scenario.hasNode,
                        requirement: "必需"
                    )

                    registryRow

                    if scenario.hasNode {
                        RuntimeEnvironmentToolRow(
                            title: "PM2",
                            detail: "用于酒馆后台保活与进程管理。",
                            symbol: "arrow.triangle.2.circlepath",
                            color: .purple,
                            isInstalled: scenario.hasPM2,
                            requirement: "可选"
                        )
                    }

                    RuntimeEnvironmentToolRow(
                        title: "Git / MinGit",
                        detail: "用于获取酒馆源码与管理版本更新。",
                        symbol: "arrow.triangle.branch",
                        color: .blue,
                        isInstalled: scenario.hasGit,
                        requirement: "版本工具"
                    )
                }
            } header: {
                Text("环境与依赖")
            } footer: {
                if !scenario.hasHomebrew {
                    Label("请先准备 Homebrew，其余运行环境将在此后显示。",
                          systemImage: "info.circle")
                } else if !scenario.hasNode {
                    Label("Node.js 就绪后，将显示可选的 PM2 后台保活工具。",
                          systemImage: "info.circle")
                } else {
                    Text("PM2 为可选项，可按需用于酒馆后台运行。")
                }
            }

            Section {
                Picker("演示状态", selection: $scenario) {
                    ForEach(RuntimeEnvironmentPreviewScenario.allCases) { value in
                        Text(value.title).tag(value)
                    }
                }
                .pickerStyle(.menu)
            } header: {
                Text("原型预览")
            } footer: {
                Text("当前展示演示状态，尚未检测本机环境。安装按钮暂不可用，软件源选择仅用于预览。")
            }
        }
        .formStyle(.grouped)
    }

    private var registryRow: some View {
        HStack(spacing: 12) {
            RuntimeEnvironmentIcon(symbol: "network", color: .teal)
            VStack(alignment: .leading, spacing: 5) {
                Text("NPM 软件源")
                    .font(.system(size: 13, weight: .semibold))
                Text("安装依赖时使用的软件源，默认 npmmirror。")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer(minLength: 16)
            Picker("NPM 软件源", selection: $registry) {
                ForEach(NPMRegistryPreview.allCases) { value in
                    Text(value.title).tag(value)
                }
            }
            .pickerStyle(.menu)
            .labelsHidden()
            .frame(width: 160)
            .help("选择 NPM 软件源，仅作原型预览")
        }
        .padding(.vertical, 6)
    }
}

private struct RuntimeEnvironmentToolRow: View {
    let title: String
    let detail: String
    let symbol: String
    let color: Color
    let isInstalled: Bool
    let requirement: String

    var body: some View {
        HStack(spacing: 12) {
            RuntimeEnvironmentIcon(symbol: symbol, color: color)
            VStack(alignment: .leading, spacing: 5) {
                HStack(spacing: 8) {
                    Text(title)
                        .font(.system(size: 13, weight: .semibold))
                    Text(requirement)
                        .font(.caption2)
                        .foregroundStyle(.secondary)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(.quaternary, in: RoundedRectangle(cornerRadius: 4))
                }
                Text(detail)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            Spacer(minLength: 16)
            Label(isInstalled ? "已安装" : "未安装",
                  systemImage: isInstalled ? "checkmark.circle.fill" : "circle.dashed")
                .font(.caption)
                .foregroundStyle(isInstalled ? Color.green : .secondary)
                .frame(width: 78, alignment: .leading)

            Button("安装") {}
                .disabled(true)
                .opacity(isInstalled ? 0 : 1)
                .accessibilityHidden(isInstalled)
                .help("安装功能将在后续版本提供")
        }
        .padding(.vertical, 6)
    }
}

private struct RuntimeEnvironmentIcon: View {
    let symbol: String
    let color: Color

    var body: some View {
        Image(systemName: symbol)
            .font(.system(size: 18, weight: .medium))
            .foregroundStyle(color)
            .frame(width: 36, height: 36)
            .background(color.opacity(0.1), in: RoundedRectangle(cornerRadius: 8))
            .accessibilityHidden(true)
    }
}

#Preview("运行环境 · 基础环境就绪") {
    RuntimeEnvironmentSettingsView()
        .frame(width: 800, height: 680)
}

#Preview("运行环境 · 缺少 Homebrew") {
    RuntimeEnvironmentSettingsView(previewScenario: .missingHomebrew)
        .frame(width: 800, height: 680)
        .preferredColorScheme(.dark)
}

#Preview("运行环境 · 缺少 Node.js") {
    RuntimeEnvironmentSettingsView(previewScenario: .homebrewOnly)
        .frame(width: 700, height: 600)
}
