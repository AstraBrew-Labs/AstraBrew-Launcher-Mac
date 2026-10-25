import SwiftUI

private enum DashboardLayout {
    static let pagePadding: CGFloat = 28
    static let maximumWidth: CGFloat = 1180
    static let launchPanelWidth: CGFloat = 236
    static let columnSpacing: CGFloat = 20
    static let cardSpacing: CGFloat = 16

    static func controlHeight(compact: Bool) -> CGFloat { compact ? 40 : 44 }
    static func widgetHeight(compact: Bool) -> CGFloat { compact ? 114 : 140 }
    static func directoryHeight(compact: Bool) -> CGFloat { compact ? 86 : 104 }
    static func panelHeight(compact: Bool) -> CGFloat {
        widgetHeight(compact: compact) * 2 + directoryHeight(compact: compact) + cardSpacing * 2
    }
}

struct DashboardView: View {
    @EnvironmentObject private var appState: AppState

    private var needsConfiguration: Bool {
        appState.configuration.directory.isEmpty
    }

    var body: some View {
        GeometryReader { geometry in
            let compact = geometry.size.height < 640
            let contentWidth = min(geometry.size.width, DashboardLayout.maximumWidth)
                - DashboardLayout.pagePadding * 2
            let optionWidth = (contentWidth - DashboardLayout.launchPanelWidth
                - DashboardLayout.columnSpacing - DashboardLayout.cardSpacing) / 2

            ScrollView {
                VStack(alignment: .leading, spacing: 20) {
                    HStack(alignment: .center) {
                        PageHeading(title: "仪表盘", subtitle: "星酿酒馆 · 本地工作空间")
                        Label("未运行", systemImage: "circle.dotted")
                            .font(.callout)
                            .foregroundStyle(.secondary)
                            .fixedSize()
                    }

                    launchToolbar(compact: compact, optionWidth: optionWidth)
                        .zIndex(1)

                    VStack(spacing: 12) {
                        sectionHeadings

                        HStack(alignment: .top, spacing: DashboardLayout.columnSpacing) {
                            launchPanel(compact: compact)
                                .frame(width: DashboardLayout.launchPanelWidth)
                            workspace(compact: compact)
                                .frame(maxWidth: .infinity)
                        }
                    }

                    serviceNotice
                }
                .padding(DashboardLayout.pagePadding)
                .frame(maxWidth: DashboardLayout.maximumWidth, alignment: .leading)
                .frame(maxWidth: .infinity)
            }
        }
    }

    private var sectionHeadings: some View {
        HStack(spacing: DashboardLayout.columnSpacing) {
            HStack {
                Label("当前酒馆", systemImage: "desktopcomputer")
                Spacer()
                Button {
                    appState.selection = .configuration
                } label: {
                    Image(systemName: "gearshape")
                }
                .buttonStyle(.borderless)
                .help("编辑酒馆配置")
                .accessibilityLabel("编辑酒馆配置")
            }
            .frame(width: DashboardLayout.launchPanelWidth)

            HStack {
                Text("工作空间")
                Spacer()
                Text("本地").font(.caption).foregroundStyle(.tertiary)
            }
        }
        .font(.system(size: 13, weight: .semibold))
        .frame(height: 22)
    }

    private func launchPanel(compact: Bool) -> some View {
        VStack(spacing: 0) {
            Spacer(minLength: compact ? 8 : 18)

            VStack(spacing: 8) {
                BrandMark(size: compact ? 56 : 84)
                Text(appState.configuration.name)
                    .font(.system(size: 21, weight: .semibold))
                    .multilineTextAlignment(.center)
                    .lineLimit(2)
                    .frame(maxWidth: .infinity)
                Label(needsConfiguration ? "尚未配置" : "本地酒馆", systemImage: "externaldrive")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }

            Spacer(minLength: compact ? 8 : 18)

            VStack(spacing: 12) {
                Button {
                    if needsConfiguration {
                        appState.selection = .configuration
                    } else {
                        appState.launchTavern()
                    }
                } label: {
                    VStack(spacing: 5) {
                        Label(needsConfiguration ? "配置酒馆" : "一键启动",
                              systemImage: needsConfiguration ? "slider.horizontal.3" : "play.fill")
                            .font(.system(size: 16, weight: .semibold))
                        Text(needsConfiguration ? "选择目录与端口" : "按当前设置启动")
                            .font(.caption)
                    }
                    .frame(maxWidth: .infinity)
                    .frame(height: 54)
                }
                .buttonStyle(.borderedProminent)
                .buttonBorderShape(.roundedRectangle(radius: 8))
                .controlSize(.large)
                .help(needsConfiguration ? "配置酒馆目录和端口" : "按当前版本、模式和浏览器启动酒馆")

                HStack(spacing: 0) {
                    Button {
                        appState.selection = .configuration
                    } label: {
                        Label("酒馆设置", systemImage: "slider.horizontal.3")
                            .frame(maxWidth: .infinity)
                    }
                    Divider().frame(height: 14)
                    Button {
                        appState.selection = .console
                    } label: {
                        Label("控制台", systemImage: "terminal")
                            .frame(maxWidth: .infinity)
                    }
                }
                .font(.caption)
                .buttonStyle(.borderless)
                .padding(.top, 4)
            }
        }
        .padding(20)
        .frame(height: DashboardLayout.panelHeight(compact: compact))
        .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
        .overlay {
            RoundedRectangle(cornerRadius: 8)
                .strokeBorder(.primary.opacity(0.07), lineWidth: 1)
        }
    }

    private func workspace(compact: Bool) -> some View {
        VStack(alignment: .leading, spacing: DashboardLayout.cardSpacing) {
            LazyVGrid(columns: [
                .init(.flexible(), spacing: DashboardLayout.cardSpacing),
                .init(.flexible(), spacing: DashboardLayout.cardSpacing)
            ], spacing: DashboardLayout.cardSpacing) {
                DashboardWidget(
                    title: "运行版本", value: appState.versionTitle,
                    detail: appState.selectedVersion?.directoryName ?? "选择本地版本",
                    symbol: "shippingbox", color: .blue, compact: compact
                ) {
                    appState.selection = .versions
                }
                DashboardWidget(
                    title: "服务端口", value: String(appState.configuration.port), detail: "127.0.0.1",
                    symbol: "network", color: .teal, compact: compact, monospacedValue: true
                ) {
                    appState.selection = .configuration
                }
                DashboardWidget(
                    title: "扩展管理", value: "尚未扫描", detail: "酒馆扩展",
                    symbol: "puzzlepiece.extension", color: .purple, compact: compact
                ) {
                    appState.selection = .extensions
                }
                DashboardWidget(
                    title: "资源管理", value: "角色卡与世界书", detail: "主题与预设",
                    symbol: "folder", color: .orange, compact: compact
                ) {
                    appState.selection = .resources
                }
            }

            directoryWidget(compact: compact)
        }
    }

    private var serviceNotice: some View {
        HStack(alignment: .center, spacing: 8) {
            Image(systemName: "info.circle")
                .foregroundStyle(.secondary)
            Text("启动服务尚未接入")
                .foregroundStyle(.secondary)
            Spacer(minLength: 0)
            Button {
                appState.selection = .console
            } label: {
                Image(systemName: "arrow.up.right")
            }
            .buttonStyle(.borderless)
            .help("查看控制台")
            .accessibilityLabel("查看控制台")
        }
        .font(.caption)
        .padding(.horizontal, 2)
    }

    private func launchToolbar(compact: Bool, optionWidth: CGFloat) -> some View {
        HStack(alignment: .top, spacing: DashboardLayout.columnSpacing) {
            DashboardToolbarField(title: "酒馆版本") {
                TavernVersionPicker {
                    DashboardToolbarValueLabel(
                        value: appState.versionTitle,
                        symbol: "shippingbox",
                        showsChevron: true,
                        iconOnly: false
                    )
                }
                .padding(.horizontal, 12)
                .frame(maxWidth: .infinity)
                .frame(height: DashboardLayout.controlHeight(compact: compact))
                .background(Color(nsColor: .controlBackgroundColor),
                            in: RoundedRectangle(cornerRadius: 9))
                .overlay {
                    RoundedRectangle(cornerRadius: 9)
                        .strokeBorder(Color.accentColor.opacity(0.5), lineWidth: 1)
                }
            }
            .frame(width: DashboardLayout.launchPanelWidth)

            HStack(alignment: .top, spacing: DashboardLayout.cardSpacing) {
                DashboardToolbarField(title: "启动模式") {
                    DashboardModePicker(compact: compact, iconOnly: optionWidth < 300)
                }

                switch appState.configuration.launchMode {
                case .normal:
                    DashboardToolbarField(title: "浏览器") {
                        DashboardBrowserPicker(
                            compact: compact,
                            iconOnly: optionWidth < CGFloat(TavernBrowser.allCases.count) * 84
                        )
                    }
                case .desktop:
                    Color.clear
                        .frame(maxWidth: .infinity)
                        .frame(height: DashboardLayout.controlHeight(compact: compact))
                        .accessibilityHidden(true)
                case .server:
                    DashboardToolbarField(title: "服务模式") {
                        DashboardServerModePicker(compact: compact, iconOnly: optionWidth < 190)
                    }
                }
            }
            .frame(maxWidth: .infinity)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }

    private func directoryWidget(compact: Bool) -> some View {
        Button {
            appState.selection = .configuration
        } label: {
            HStack(spacing: 14) {
                Image(systemName: "externaldrive")
                    .font(.system(size: 24))
                    .foregroundStyle(Color.accentColor)
                    .frame(width: 36)
                VStack(alignment: .leading, spacing: 6) {
                    Text("酒馆目录")
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Text(needsConfiguration ? "选择一个本地目录" : directoryName)
                        .font(.system(size: 14, weight: .medium))
                        .lineLimit(1)
                        .truncationMode(.middle)
                    Text(needsConfiguration ? "尚未设置" : appState.configuration.directory)
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                Image(systemName: "chevron.right")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.tertiary)
            }
            .padding(compact ? 14 : 18)
            .frame(maxWidth: .infinity, alignment: .leading)
            .frame(height: DashboardLayout.directoryHeight(compact: compact))
            .contentShape(Rectangle())
        }
        .buttonStyle(DashboardCardStyle())
        .help(needsConfiguration ? "选择酒馆目录" : appState.configuration.directory)
    }

    private var directoryName: String {
        URL(fileURLWithPath: appState.configuration.directory).lastPathComponent
    }
}

private struct DashboardToolbarField<Content: View>: View {
    let title: String
    let content: Content

    init(title: String, @ViewBuilder content: () -> Content) {
        self.title = title
        self.content = content()
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(title)
                .font(.caption)
                .foregroundStyle(.secondary)
                .frame(height: 16, alignment: .leading)
            content
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct DashboardToolbarValueLabel: View {
    let value: String
    let symbol: String
    let showsChevron: Bool
    let iconOnly: Bool

    var body: some View {
        HStack(spacing: 10) {
            Image(systemName: symbol)
                .font(.system(size: 15, weight: .medium))
                .foregroundStyle(Color.accentColor)
                .frame(width: 20)
            if !iconOnly {
                Text(value)
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(.primary)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            Spacer(minLength: 0)
            if showsChevron {
                Image(systemName: "chevron.down")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(.secondary)
            }
        }
        .contentShape(Rectangle())
    }
}

private struct DashboardSegmentedControl<Option: Identifiable>: View {
    let options: [Option]
    let selection: Option
    let title: (Option) -> String
    let symbol: (Option) -> String
    let action: (Option) -> Void
    let compact: Bool
    let iconOnly: Bool

    var body: some View {
        HStack(spacing: 2) {
            ForEach(options) { option in
                Button {
                    action(option)
                } label: {
                    Group {
                        if iconOnly {
                            Image(systemName: symbol(option))
                                .accessibilityLabel(title(option))
                        } else {
                            Label(title(option), systemImage: symbol(option))
                                .lineLimit(1)
                                .minimumScaleFactor(0.8)
                        }
                    }
                    .font(.system(size: compact ? 11 : 12, weight: .medium))
                    .frame(maxWidth: .infinity)
                    .frame(height: DashboardLayout.controlHeight(compact: compact) - 6)
                    .padding(.horizontal, iconOnly ? 8 : 5)
                    .foregroundStyle(isSelected(option) ? .white : .primary)
                    .background {
                        if isSelected(option) {
                            RoundedRectangle(cornerRadius: 8)
                                .fill(Color.accentColor)
                        }
                    }
                    .contentShape(RoundedRectangle(cornerRadius: 8))
                }
                .buttonStyle(.plain)
                .frame(maxWidth: .infinity)
                .contentShape(RoundedRectangle(cornerRadius: 8))
                .onHover { isHovering in
                    guard iconOnly else { return }
                    hoveredOptionID = isHovering ? option.id : nil
                }
                .overlay(alignment: .bottom) {
                    if iconOnly && hoveredOptionID == option.id {
                        DashboardTooltip(text: title(option))
                            .offset(y: 34)
                            .zIndex(20)
                            .allowsHitTesting(false)
                    }
                }
                .help(title(option))
            }
        }
        .padding(3)
        .frame(height: DashboardLayout.controlHeight(compact: compact))
        .background(Color.primary.opacity(0.08), in: RoundedRectangle(cornerRadius: 10))
        .overlay {
            RoundedRectangle(cornerRadius: 10)
                .strokeBorder(.primary.opacity(0.06), lineWidth: 1)
        }
    }

    @State private var hoveredOptionID: Option.ID?

    private func isSelected(_ option: Option) -> Bool {
        option.id == selection.id
    }
}

private struct DashboardTooltip: View {
    let text: String

    var body: some View {
        Text(text)
            .font(.caption2.weight(.medium))
            .foregroundStyle(.primary)
            .padding(.horizontal, 8)
            .padding(.vertical, 5)
            .fixedSize()
            .background(.regularMaterial, in: RoundedRectangle(cornerRadius: 6))
            .overlay {
                RoundedRectangle(cornerRadius: 6)
                    .strokeBorder(.primary.opacity(0.12), lineWidth: 1)
            }
            .shadow(color: .black.opacity(0.18), radius: 5, y: 2)
    }
}

private struct DashboardBrowserPicker: View {
    @EnvironmentObject private var appState: AppState
    let compact: Bool
    let iconOnly: Bool
    @State private var errorMessage: String?

    var body: some View {
        DashboardSegmentedControl(
            options: TavernBrowser.allCases,
            selection: appState.configuration.browser,
            title: \.title,
            symbol: \.symbol,
            action: select,
            compact: compact,
            iconOnly: iconOnly
        )
        .accessibilityElement(children: .contain)
        .accessibilityLabel("浏览器")
        .accessibilityValue(appState.configuration.browser.title)
        .alert("无法保存浏览器设置", isPresented: Binding(
            get: { errorMessage != nil },
            set: { if !$0 { errorMessage = nil } }
        )) {
            Button("好", role: .cancel) { errorMessage = nil }
        } message: {
            Text(errorMessage ?? "")
        }
    }

    private func select(_ browser: TavernBrowser) {
        do {
            try appState.setBrowser(browser)
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

private struct DashboardModePicker: View {
    @EnvironmentObject private var appState: AppState
    let compact: Bool
    let iconOnly: Bool
    @State private var errorMessage: String?

    var body: some View {
        DashboardSegmentedControl(
            options: TavernLaunchMode.allCases,
            selection: appState.configuration.launchMode,
            title: \.title,
            symbol: \.symbol,
            action: select,
            compact: compact,
            iconOnly: iconOnly
        )
        .accessibilityElement(children: .contain)
        .accessibilityLabel("启动模式")
        .accessibilityValue(appState.configuration.launchMode.title)
        .alert("无法保存启动模式", isPresented: Binding(
            get: { errorMessage != nil },
            set: { if !$0 { errorMessage = nil } }
        )) {
            Button("好", role: .cancel) { errorMessage = nil }
        } message: {
            Text(errorMessage ?? "")
        }
    }

    private func select(_ mode: TavernLaunchMode) {
        do {
            try appState.setLaunchMode(mode)
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

private struct DashboardServerModePicker: View {
    @EnvironmentObject private var appState: AppState
    let compact: Bool
    let iconOnly: Bool
    @State private var errorMessage: String?

    var body: some View {
        DashboardSegmentedControl(
            options: TavernServerMode.allCases,
            selection: appState.configuration.serverMode,
            title: \.title,
            symbol: \.symbol,
            action: select,
            compact: compact,
            iconOnly: iconOnly
        )
        .accessibilityElement(children: .contain)
        .accessibilityLabel("服务模式")
        .accessibilityValue(appState.configuration.serverMode.title)
        .alert("无法保存服务模式", isPresented: Binding(
            get: { errorMessage != nil },
            set: { if !$0 { errorMessage = nil } }
        )) {
            Button("好", role: .cancel) { errorMessage = nil }
        } message: {
            Text(errorMessage ?? "")
        }
    }

    private func select(_ serverMode: TavernServerMode) {
        do {
            try appState.setServerMode(serverMode)
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}

private struct DashboardWidget: View {
    let title: String
    let value: String
    let detail: String
    let symbol: String
    let color: Color
    let compact: Bool
    var monospacedValue = false
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: compact ? 9 : 13) {
                HStack(spacing: 9) {
                    Image(systemName: symbol)
                        .font(.system(size: 15, weight: .medium))
                        .foregroundStyle(color)
                        .frame(width: compact ? 26 : 30, height: compact ? 26 : 30)
                        .background(color.opacity(0.10), in: RoundedRectangle(cornerRadius: 6))
                    Text(title)
                        .font(.caption)
                        .foregroundStyle(.secondary)
                        .lineLimit(1)
                    Spacer(minLength: 0)
                    Image(systemName: "arrow.up.right")
                        .font(.system(size: 9, weight: .semibold))
                        .foregroundStyle(.tertiary)
                }

                VStack(alignment: .leading, spacing: 5) {
                    Text(value)
                        .font(.system(size: monospacedValue ? 23 : 18, weight: .semibold,
                                      design: monospacedValue ? .monospaced : .default))
                        .lineLimit(1)
                        .minimumScaleFactor(0.85)
                        .frame(height: 28, alignment: .bottomLeading)
                    Text(detail)
                        .font(.caption)
                        .foregroundStyle(.tertiary)
                        .lineLimit(1)
                }
            }
            .padding(compact ? 14 : 16)
            .frame(maxWidth: .infinity, alignment: .leading)
            .frame(height: DashboardLayout.widgetHeight(compact: compact), alignment: .topLeading)
            .contentShape(Rectangle())
        }
        .buttonStyle(DashboardCardStyle())
        .help("打开\(title == "服务端口" ? "酒馆配置" : title)")
    }
}

private struct DashboardCardStyle: ButtonStyle {
    @State private var isHovered = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .background(Color(nsColor: .controlBackgroundColor), in: RoundedRectangle(cornerRadius: 8))
            .overlay {
                RoundedRectangle(cornerRadius: 8)
                    .strokeBorder(isHovered ? Color.accentColor.opacity(0.45) : .primary.opacity(0.07),
                                  lineWidth: 1)
            }
            .shadow(color: .black.opacity(0.035), radius: 5, x: 0, y: 2)
            .opacity(configuration.isPressed ? 0.75 : 1)
            .onHover { isHovered = $0 }
    }
}
