import SwiftUI

struct ContentView: View {
    @EnvironmentObject private var appState: AppState

    var body: some View {
        NavigationSplitView {
            VStack(spacing: 0) {
                HStack(spacing: 0) {
                    BrandLogo()
                        .frame(width: 166, height: 56)
                    Spacer(minLength: 0)
                }
                .padding(.horizontal, 20)
                .padding(.vertical, 22)

                List(selection: $appState.selection) {
                    Section("工作空间") {
                        ForEach(LauncherPage.workspacePages) { page in
                            Label(page.title, systemImage: page.symbol)
                                .tag(page)
                                .padding(.vertical, 3)
                        }
                    }
                }
                .listStyle(.sidebar)

                Divider().padding(.horizontal, 16)
                VStack(spacing: 4) {
                    ForEach(LauncherPage.utilityPages) { page in
                        Button {
                            appState.selection = page
                        } label: {
                            Label(page.title, systemImage: page.symbol)
                                .font(.system(size: 13))
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .padding(.horizontal, 12)
                                .padding(.vertical, 9)
                                .background(
                                    appState.selection == page ? Color.accentColor.opacity(0.14) : .clear,
                                    in: RoundedRectangle(cornerRadius: 6)
                                )
                                .contentShape(Rectangle())
                        }
                        .buttonStyle(.plain)
                        .foregroundStyle(appState.selection == page ? Color.accentColor : .primary)
                        .accessibilityAddTraits(appState.selection == page ? .isSelected : [])
                    }
                }
                .padding(12)

                HStack(spacing: 7) {
                    Circle().fill(.secondary).frame(width: 6, height: 6)
                    Text("酒馆未运行").font(.caption).foregroundStyle(.secondary)
                    Spacer()
                }
                .padding(.horizontal, 24)
                .padding(.bottom, 18)
            }
            .navigationSplitViewColumnWidth(min: 210, ideal: 230, max: 270)
        } detail: {
            VStack(spacing: 0) {
                selectedPage
                    .frame(maxWidth: .infinity, maxHeight: .infinity)
                Divider()
                HStack(spacing: 8) {
                    Image(systemName: "externaldrive").foregroundStyle(.secondary)
                    Text(appState.configuration.name).lineLimit(1)
                    Spacer()
                    Text("本地工作空间").foregroundStyle(.secondary)
                }
                .font(.caption)
                .padding(.horizontal, 24)
                .frame(height: 32)
                .background(.bar)
            }
            .background(Color(nsColor: .windowBackgroundColor))
            .navigationTitle((appState.selection ?? .dashboard).title)
            .toolbar {
                ToolbarItem(placement: .primaryAction) {
                    Button {
                        appState.selection = .console
                    } label: {
                        Label("控制台", systemImage: "terminal")
                    }
                    .help("打开控制台")
                }
            }
        }
        .navigationSplitViewStyle(.balanced)
        .tint(.accentColor)
    }

    @ViewBuilder
    private var selectedPage: some View {
        switch appState.selection ?? .dashboard {
        case .dashboard: DashboardView()
        case .configuration: TavernConfigurationView()
        case .versions: TavernVersionsView()
        case .extensions: LibraryView(kind: .extensions)
        case .resources: LibraryView(kind: .resources)
        case .console: ConsoleView()
        case .settings: LauncherSettingsView()
        case .about: AboutView()
        }
    }
}

#Preview {
    ContentView()
        .environmentObject(AppState())
        .frame(width: 1180, height: 780)
}
