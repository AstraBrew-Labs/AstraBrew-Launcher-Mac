import SwiftUI

enum LibraryKind {
    case versions, extensions, resources

    var page: LauncherPage {
        switch self {
        case .versions: .versions
        case .extensions: .extensions
        case .resources: .resources
        }
    }

    var categories: [String] {
        switch self {
        case .versions: ["全部", "稳定版", "开发版"]
        case .extensions: ["全部", "已启用", "未启用"]
        case .resources: ["全部", "角色卡", "世界书", "主题"]
        }
    }

    var emptyTitle: String {
        switch self {
        case .versions: "暂无本地版本"
        case .extensions: "暂无扩展"
        case .resources: "暂无资源"
        }
    }

    var emptyDescription: String {
        switch self {
        case .versions: "版本安装与切换服务尚未接入。"
        case .extensions: "扩展扫描与管理服务尚未接入。"
        case .resources: "资源扫描与导入服务尚未接入。"
        }
    }
}

struct LibraryView: View {
    let kind: LibraryKind
    @State private var category = "全部"
    @State private var query = ""
    @EnvironmentObject private var appState: AppState

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            PageHeading(title: kind.page.title, subtitle: "本地资料库 · 0 项")

            HStack(spacing: 20) {
                Picker("分类", selection: $category) {
                    ForEach(kind.categories, id: \.self) { value in
                        Text(value).tag(value)
                    }
                }
                .pickerStyle(.segmented)
                .frame(maxWidth: 360)
                Spacer(minLength: 0)
                HStack(spacing: 7) {
                    Image(systemName: "magnifyingglass").foregroundStyle(.secondary)
                    TextField("搜索", text: $query).textFieldStyle(.plain)
                    if !query.isEmpty {
                        Button {
                            query = ""
                        } label: {
                            Image(systemName: "xmark.circle.fill")
                        }
                        .buttonStyle(.plain)
                        .help("清除搜索")
                    }
                }
                .padding(7)
                .background(.quaternary.opacity(0.4), in: RoundedRectangle(cornerRadius: 6))
                .frame(width: 200)
                .accessibilityLabel("搜索\(kind.page.title)")
            }
            Divider()
            ContentUnavailableView {
                Label(query.isEmpty ? kind.emptyTitle : "没有匹配的项目",
                      systemImage: query.isEmpty ? kind.page.symbol : "magnifyingglass")
            } description: {
                Text(query.isEmpty ? kind.emptyDescription : "未找到“\(query)”")
            } actions: {
                if query.isEmpty {
                    Button("酒馆配置", systemImage: "slider.horizontal.3") {
                        appState.selection = .configuration
                    }
                } else {
                    Button("清除搜索") { query = "" }
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
        .padding(32)
        .id(kind.page)
    }
}
