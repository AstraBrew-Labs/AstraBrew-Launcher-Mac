import SwiftUI

struct TavernVersionsView: View {
    @EnvironmentObject private var appState: AppState
    @State private var errorMessage: String?

    var body: some View {
        VStack(alignment: .leading, spacing: 24) {
            HStack {
                PageHeading(title: "版本管理", subtitle: "本地酒馆 · \(appState.versions.count) 个版本")
                TavernVersionPicker {
                    Label("添加或切换", systemImage: "shippingbox")
                        .fixedSize()
                }
                .fixedSize()
            }

            if appState.versions.isEmpty {
                ContentUnavailableView {
                    Label("暂无本地版本", systemImage: "shippingbox")
                } description: {
                    Text("添加已有的酒馆目录后，即可在主页切换。")
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    LazyVStack(spacing: 12) {
                        ForEach(appState.versions) { version in
                            versionRow(version)
                        }
                    }
                }
            }
        }
        .padding(32)
        .alert("无法切换酒馆版本", isPresented: Binding(
            get: { errorMessage != nil },
            set: { if !$0 { errorMessage = nil } }
        )) {
            Button("好", role: .cancel) { errorMessage = nil }
        } message: {
            Text(errorMessage ?? "")
        }
    }

    private func versionRow(_ version: TavernVersion) -> some View {
        HStack(spacing: 16) {
            Image(systemName: "shippingbox")
                .font(.title2)
                .foregroundStyle(Color.accentColor)
                .frame(width: 40)
            VStack(alignment: .leading, spacing: 6) {
                Text(version.menuTitle).font(.headline).lineLimit(1)
                Text(version.directory)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .textSelection(.enabled)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if appState.configuration.selectedVersionID == version.id {
                Label("当前版本", systemImage: "checkmark.circle.fill")
                    .font(.callout)
                    .foregroundStyle(Color.accentColor)
                    .fixedSize()
            } else {
                Button("使用此版本", systemImage: "arrow.right") {
                    do { try appState.selectVersion(version.id) }
                    catch { errorMessage = error.localizedDescription }
                }
                .fixedSize()
            }
        }
        .padding(18)
        .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
        .help(version.directory)
    }
}
