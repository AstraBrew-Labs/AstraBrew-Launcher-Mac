import SwiftUI
import UniformTypeIdentifiers

struct TavernVersionPicker<Label: View>: View {
    @EnvironmentObject private var appState: AppState
    @State private var isImporting = false
    @State private var errorMessage: String?
    private let label: Label

    init(@ViewBuilder label: () -> Label) {
        self.label = label()
    }

    var body: some View {
        Menu {
            if appState.versions.isEmpty {
                Text("暂无已添加的本地版本")
            } else {
                Picker("酒馆版本", selection: selection) {
                    Text("使用自定义目录").tag(nil as UUID?)
                    ForEach(appState.versions) { version in
                        Text(version.menuTitle).tag(Optional(version.id))
                    }
                }
                .pickerStyle(.inline)
            }
            Divider()
            Button("添加本地酒馆…", systemImage: "folder.badge.plus") {
                isImporting = true
            }
            Button("版本管理", systemImage: "shippingbox") {
                appState.selection = .versions
            }
        } label: {
            label
        }
        .menuStyle(.borderlessButton)
        .menuIndicator(.hidden)
        .fileImporter(isPresented: $isImporting, allowedContentTypes: [.folder]) { result in
            do {
                let url = try result.get()
                let hasAccess = url.startAccessingSecurityScopedResource()
                defer { if hasAccess { url.stopAccessingSecurityScopedResource() } }
                try appState.importVersion(from: url)
            } catch {
                errorMessage = error.localizedDescription
            }
        }
        .alert("无法选择酒馆版本", isPresented: Binding(
            get: { errorMessage != nil },
            set: { if !$0 { errorMessage = nil } }
        )) {
            Button("好", role: .cancel) { errorMessage = nil }
        } message: {
            Text(errorMessage ?? "")
        }
        .help("切换已添加的本地酒馆版本")
        .accessibilityLabel("酒馆版本")
        .accessibilityValue(appState.versionTitle)
    }

    private var selection: Binding<UUID?> {
        Binding(
            get: { appState.configuration.selectedVersionID },
            set: {
                do { try appState.selectVersion($0) }
                catch { errorMessage = error.localizedDescription }
            }
        )
    }
}

