import SwiftUI

enum LauncherPage: String, CaseIterable, Identifiable {
    case dashboard, configuration, versions, extensions, resources, console, settings, about

    var id: Self { self }

    static let workspacePages: [Self] = [.dashboard, .configuration, .versions, .extensions, .resources]
    static let utilityPages: [Self] = [.console, .settings, .about]

    var title: String {
        switch self {
        case .dashboard: "仪表盘"
        case .configuration: "酒馆配置"
        case .versions: "版本管理"
        case .extensions: "扩展管理"
        case .resources: "资源管理"
        case .console: "控制台"
        case .settings: "设置"
        case .about: "关于"
        }
    }

    var symbol: String {
        switch self {
        case .dashboard: "square.grid.2x2"
        case .configuration: "slider.horizontal.3"
        case .versions: "shippingbox"
        case .extensions: "puzzlepiece.extension"
        case .resources: "folder"
        case .console: "terminal"
        case .settings: "gearshape"
        case .about: "info.circle"
        }
    }
}

enum AppAppearance: String, CaseIterable, Identifiable {
    case system, light, dark

    var id: Self { self }

    var title: String {
        switch self {
        case .system: "跟随系统"
        case .light: "浅色"
        case .dark: "深色"
        }
    }

    var colorScheme: ColorScheme? {
        switch self {
        case .system: nil
        case .light: .light
        case .dark: .dark
        }
    }
}
