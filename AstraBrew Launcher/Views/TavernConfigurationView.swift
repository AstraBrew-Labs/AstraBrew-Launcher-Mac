import AppKit
import SwiftUI

struct TavernConfigurationView: View {
    @EnvironmentObject private var appState: AppState
    @State private var draft = TavernConfiguration()
    @State private var port = "8000"
    @State private var feedback: String?
    @State private var saveError: String?
    @State private var isChoosingDirectory = false

    private var validPort: Int? {
        guard let value = Int(port), (1...65535).contains(value) else { return nil }
        return value
    }

    private var hasChanges: Bool {
        draft != appState.configuration || port != String(appState.configuration.port)
    }

    var body: some View {
        VStack(spacing: 0) {
            PageHeading(title: "酒馆配置", subtitle: appState.configuration.name)
                .padding(32)

            Form {
                Section("基本信息") {
                    TextField("配置名称", text: $draft.name)
                    LabeledContent("酒馆目录") {
                        HStack {
                            Text(draft.directory.isEmpty ? "未选择" : draft.directory)
                                .foregroundStyle(.secondary)
                                .lineLimit(2)
                                .textSelection(.enabled)
                                .help(draft.directory)
                            Button("选择…", systemImage: "folder") {
                                chooseDirectory()
                            }
                            .disabled(isChoosingDirectory)
                        }
                    }
                }

                Section("本地服务") {
                    TextField("端口", text: $port)
                    if validPort == nil {
                        Label("端口须为 1–65535 之间的整数", systemImage: "exclamationmark.circle")
                            .foregroundStyle(.red)
                    }
                    LabeledContent("监听地址", value: "127.0.0.1")
                }

                Section("运行环境") {
                    Picker("启动模式", selection: $draft.launchMode) {
                        ForEach(TavernLaunchMode.allCases) { mode in
                            Label(mode.title, systemImage: mode.symbol).tag(mode)
                        }
                    }
                    LabeledContent("进程服务", value: "尚未接入")
                    LabeledContent("运行版本", value: draftVersionTitle)
                }
            }
            .formStyle(.grouped)

            Divider()
            HStack {
                if let feedback, !hasChanges {
                    Label(feedback, systemImage: "checkmark.circle.fill")
                        .foregroundStyle(.green)
                        .font(.callout)
                }
                Spacer()
                Button("还原", systemImage: "arrow.counterclockwise") {
                    loadConfiguration()
                }
                .disabled(!hasChanges)
                Button("保存配置", systemImage: "checkmark") {
                    save()
                }
                .buttonStyle(.borderedProminent)
                .disabled(!hasChanges || validPort == nil || draft.name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
            .padding(20)
        }
        .onAppear(perform: loadConfiguration)
        .alert("保存失败", isPresented: Binding(
            get: { saveError != nil },
            set: { if !$0 { saveError = nil } }
        )) {
            Button("好", role: .cancel) { saveError = nil }
        } message: {
            Text(saveError ?? "")
        }
    }

    private func loadConfiguration() {
        draft = appState.configuration
        port = String(appState.configuration.port)
        feedback = nil
    }

    private var draftVersionTitle: String {
        guard let version = appState.versions.first(where: {
            $0.id == draft.selectedVersionID && $0.directory == draft.directory
        }) else {
            return draft.directory.isEmpty ? "未选择版本" : "自定义目录"
        }
        return version.version
    }

    private func save() {
        guard let validPort else { return }
        draft.name = draft.name.trimmingCharacters(in: .whitespacesAndNewlines)
        draft.port = validPort
        port = String(validPort)
        do {
            try appState.saveConfiguration(draft)
            feedback = "配置已保存"
        } catch {
            saveError = error.localizedDescription
        }
    }

    private func chooseDirectory() {
        let panel = NSOpenPanel()
        panel.title = "选择酒馆目录"
        panel.canChooseFiles = false
        panel.canChooseDirectories = true
        panel.allowsMultipleSelection = false
        panel.prompt = "选择目录"
        isChoosingDirectory = true
        panel.begin { response in
            isChoosingDirectory = false
            if response == .OK, let url = panel.url {
                draft.directory = url.path
            }
        }
    }
}
