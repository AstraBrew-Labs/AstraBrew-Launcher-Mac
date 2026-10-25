import AppKit
import Combine
import Foundation

struct TavernConfiguration: Codable, Equatable {
    var name = "默认酒馆"
    var directory = ""
    var port = 8000
    var launchMode: TavernLaunchMode = .normal
    var browser: TavernBrowser = .system
    var serverMode: TavernServerMode = .localNetwork
    var selectedVersionID: UUID?

    enum CodingKeys: String, CodingKey {
        case name, directory, port, launchMode, browser, serverMode, selectedVersionID
    }

    init(name: String = "默认酒馆", directory: String = "", port: Int = 8000,
         launchMode: TavernLaunchMode = .normal, browser: TavernBrowser = .system,
         serverMode: TavernServerMode = .localNetwork,
         selectedVersionID: UUID? = nil) {
        self.name = name
        self.directory = directory
        self.port = port
        self.launchMode = launchMode
        self.browser = browser
        self.serverMode = serverMode
        self.selectedVersionID = selectedVersionID
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        name = try values.decodeIfPresent(String.self, forKey: .name) ?? "默认酒馆"
        directory = try values.decodeIfPresent(String.self, forKey: .directory) ?? ""
        port = try values.decodeIfPresent(Int.self, forKey: .port) ?? 8000
        // Older configurations predate the home-page quick switches.
        launchMode = (try? values.decode(TavernLaunchMode.self, forKey: .launchMode)) ?? .normal
        browser = (try? values.decode(TavernBrowser.self, forKey: .browser)) ?? .system
        serverMode = (try? values.decode(TavernServerMode.self, forKey: .serverMode)) ?? .localNetwork
        selectedVersionID = try values.decodeIfPresent(UUID.self, forKey: .selectedVersionID)
    }
}

struct LogEntry: Identifiable {
    enum Level: String, CaseIterable, Identifiable {
        case info = "信息"
        case warning = "警告"
        var id: Self { self }
    }

    let id = UUID()
    let date = Date()
    let level: Level
    let message: String
}

@MainActor
final class AppState: ObservableObject {
    @Published var selection: LauncherPage? = .dashboard
    @Published private(set) var configuration: TavernConfiguration
    @Published private(set) var versions: [TavernVersion]
    @Published private(set) var logs: [LogEntry] = []
    @Published var showsTimestamps: Bool {
        didSet { defaults.set(showsTimestamps, forKey: "showsTimestamps") }
    }

    private let defaults: UserDefaults
    private static let configurationKey = "tavernConfiguration"
    private static let versionsKey = "tavernVersions"

    var selectedVersion: TavernVersion? {
        versions.first { $0.id == configuration.selectedVersionID }
    }

    var versionTitle: String {
        selectedVersion?.version ?? (configuration.directory.isEmpty ? "未选择版本" : "自定义目录")
    }

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
        showsTimestamps = defaults.object(forKey: "showsTimestamps") as? Bool ?? true
        if let data = defaults.data(forKey: Self.versionsKey),
           let stored = try? JSONDecoder().decode([TavernVersion].self, from: data) {
            versions = stored
        } else {
            versions = []
        }
        if let data = defaults.data(forKey: Self.configurationKey),
           let stored = try? JSONDecoder().decode(TavernConfiguration.self, from: data) {
            configuration = stored
        } else {
            configuration = TavernConfiguration()
        }
        if let id = configuration.selectedVersionID, !versions.contains(where: { $0.id == id }) {
            configuration.selectedVersionID = nil
        }
        addLog("AstraBrew Launcher 已就绪。")
        addLog("酒馆进程服务尚未接入。", level: .warning)
    }

    func saveConfiguration(_ value: TavernConfiguration) throws {
        var value = value
        if let id = value.selectedVersionID,
           !versions.contains(where: { $0.id == id && $0.directory == value.directory }) {
            value.selectedVersionID = nil
        }
        let data = try JSONEncoder().encode(value)
        defaults.set(data, forKey: Self.configurationKey)
        configuration = value
        addLog("已保存酒馆配置：\(value.name)。")
    }

    func selectVersion(_ id: UUID?) throws {
        var next = configuration
        if let id {
            guard let version = versions.first(where: { $0.id == id }) else {
                throw TavernSelectionError.versionMissing
            }
            next.directory = version.directory
        }
        next.selectedVersionID = id
        try saveConfiguration(next)
    }

    func setLaunchMode(_ mode: TavernLaunchMode) throws {
        guard mode != configuration.launchMode else { return }
        var next = configuration
        next.launchMode = mode
        try saveConfiguration(next)
    }

    func setBrowser(_ browser: TavernBrowser) throws {
        guard browser != configuration.browser else { return }
        var next = configuration
        next.browser = browser
        try saveConfiguration(next)
    }

    func setServerMode(_ serverMode: TavernServerMode) throws {
        guard serverMode != configuration.serverMode else { return }
        var next = configuration
        next.serverMode = serverMode
        try saveConfiguration(next)
    }

    func launchTavern() {
        guard !configuration.directory.isEmpty else {
            addLog("启动酒馆前需要先选择酒馆目录。", level: .warning)
            selection = .configuration
            return
        }

        let launchTarget: String
        switch configuration.launchMode {
        case .server:
            launchTarget = "服务模式：\(configuration.serverMode.title)"
        default:
            launchTarget = "浏览器：\(configuration.browser.title)"
        }
        addLog("已准备启动酒馆：\(versionTitle)，\(configuration.launchMode.title)，\(launchTarget)。")
        selection = .console
    }

    func importVersion(from directory: URL) throws {
        var version = try TavernVersion.read(from: directory)
        var nextVersions = versions
        if let index = nextVersions.firstIndex(where: { $0.directory == version.directory }) {
            version.id = nextVersions[index].id
            nextVersions[index] = version
        } else {
            nextVersions.append(version)
        }
        var nextConfiguration = configuration
        nextConfiguration.directory = version.directory
        nextConfiguration.selectedVersionID = version.id
        let versionsData = try JSONEncoder().encode(nextVersions)
        let configurationData = try JSONEncoder().encode(nextConfiguration)
        defaults.set(versionsData, forKey: Self.versionsKey)
        defaults.set(configurationData, forKey: Self.configurationKey)
        versions = nextVersions
        configuration = nextConfiguration
        addLog("已选择酒馆版本 \(version.version)（\(version.directoryName)）。")
    }

    func addLog(_ message: String, level: LogEntry.Level = .info) {
        logs.append(LogEntry(level: level, message: message))
        if logs.count > 1000 {
            logs.removeFirst(logs.count - 1000)
        }
    }

    func clearLogs() {
        logs.removeAll()
    }
}
