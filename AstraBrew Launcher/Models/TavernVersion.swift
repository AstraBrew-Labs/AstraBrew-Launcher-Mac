import Foundation

enum TavernBrowser: String, Codable, CaseIterable, Identifiable {
    case system, chrome, firefox, edge

    var id: Self { self }

    var title: String {
        switch self {
        case .system: "系统默认"
        case .chrome: "Chrome"
        case .firefox: "Firefox"
        case .edge: "Edge"
        }
    }

    var symbol: String {
        switch self {
        case .system: "globe"
        case .chrome: "display"
        case .firefox: "safari"
        case .edge: "rectangle.grid.2x2"
        }
    }
}

enum TavernLaunchMode: String, Codable, CaseIterable, Identifiable {
    case normal, desktop, server

    var id: Self { self }

    var title: String {
        switch self {
        case .normal: "普通模式"
        case .desktop: "桌面模式"
        case .server: "服务器模式"
        }
    }

    var symbol: String {
        switch self {
        case .normal: "globe"
        case .desktop: "macwindow"
        case .server: "server.rack"
        }
    }
}

struct TavernVersion: Codable, Equatable, Identifiable {
    var id = UUID()
    let version: String
    let directory: String

    var directoryName: String {
        URL(fileURLWithPath: directory).lastPathComponent
    }

    var menuTitle: String { "\(version) · \(directoryName)" }

    static func read(from directory: URL) throws -> Self {
        struct Package: Decodable {
            let name: String
            let version: String
        }

        let root = directory.standardizedFileURL.resolvingSymlinksInPath()
        let package: Package
        do {
            let data = try Data(contentsOf: root.appendingPathComponent("package.json"))
            package = try JSONDecoder().decode(Package.self, from: data)
        } catch {
            throw TavernSelectionError.invalidDirectory
        }
        let name = package.name.lowercased()
        guard ["sillytavern", "astrabrew-tavern", "@astrabrew/tavern"].contains(name),
              !package.version.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw TavernSelectionError.invalidDirectory
        }
        return Self(version: package.version, directory: root.path)
    }
}

enum TavernSelectionError: LocalizedError {
    case invalidDirectory
    case versionMissing

    var errorDescription: String? {
        switch self {
        case .invalidDirectory:
            "请选择包含有效 package.json 和版本号的 SillyTavern 或星酿酒馆目录。"
        case .versionMissing:
            "所选版本已不在本地列表中，请重新选择。"
        }
    }
}
