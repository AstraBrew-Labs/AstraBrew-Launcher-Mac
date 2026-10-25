import SwiftUI

struct LauncherSettingsView: View {
    @AppStorage("appearance") private var appearance = AppAppearance.system.rawValue
    @EnvironmentObject private var appState: AppState
    @State private var category = SettingsCategory.general

    var body: some View {
        VStack(spacing: 0) {
            PageHeading(title: "设置", subtitle: "AstraBrew Launcher")
                .padding(32)

            Picker("设置分类", selection: $category) {
                Label("常规", systemImage: "gearshape").tag(SettingsCategory.general)
                Label("运行环境", systemImage: "shippingbox").tag(SettingsCategory.runtime)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            .frame(width: 260)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(.horizontal, 32)
            .padding(.bottom, 16)

            switch category {
            case .general:
                generalSettings
            case .runtime:
                RuntimeEnvironmentSettingsView()
            }
        }
    }

    private var generalSettings: some View {
        Form {
            Section("外观") {
                Picker("主题", selection: $appearance) {
                    ForEach(AppAppearance.allCases) { value in
                        Text(value.title).tag(value.rawValue)
                    }
                }
                .pickerStyle(.segmented)
            }
            Section("控制台") {
                Toggle("显示时间戳", isOn: $appState.showsTimestamps)
                LabeledContent("日志保留", value: "当前会话，最多 1,000 条")
            }
            Section("应用") {
                LabeledContent("版本", value: AppMetadata.version)
                LabeledContent("平台", value: "macOS")
            }
        }
        .formStyle(.grouped)
    }

    private enum SettingsCategory {
        case general, runtime
    }
}

enum AppMetadata {
    static var version: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "1.0"
    }

    static var build: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleVersion") as? String ?? "1"
    }
}

struct AboutView: View {
    var body: some View {
        ScrollView {
            VStack(spacing: 18) {
                BrandMark(size: 80)
                VStack(spacing: 7) {
                    Text("AstraBrew Launcher")
                        .font(.system(size: 26, weight: .bold))
                    Text("星酿 · 酒馆启动器")
                        .font(.title3)
                        .foregroundStyle(.secondary)
                }
                Text("版本 \(AppMetadata.version) (\(AppMetadata.build))")
                    .font(.callout.monospacedDigit())
                    .foregroundStyle(.secondary)
                Divider().padding(.vertical, 14)
                DetailRow(title: "开发团队", value: "AstraBrew Labs")
                DetailRow(title: "应用平台", value: "macOS")
            }
            .padding(48)
            .frame(maxWidth: 560)
            .frame(maxWidth: .infinity)
        }
    }
}
