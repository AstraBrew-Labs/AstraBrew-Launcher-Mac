import Foundation

@main
struct LauncherStateChecks {
    @MainActor
    static func main() throws {
        let suite = "cn.astrabrew.launcher-state-checks.\(UUID().uuidString)"
        let defaults = UserDefaults(suiteName: suite)!
        let temporary = FileManager.default.temporaryDirectory
            .appendingPathComponent("astrabrew-state-checks-\(UUID().uuidString)")
        try FileManager.default.createDirectory(at: temporary, withIntermediateDirectories: true)
        defer {
            defaults.removePersistentDomain(forName: suite)
            try? FileManager.default.removeItem(at: temporary)
        }

        let legacy: [String: Any] = ["name": "Existing Tavern", "directory": "/tmp/existing", "port": 9876]
        defaults.set(try JSONSerialization.data(withJSONObject: legacy), forKey: "tavernConfiguration")
        let state = AppState(defaults: defaults)
        precondition(state.configuration.name == "Existing Tavern")
        precondition(state.configuration.directory == "/tmp/existing")
        precondition(state.configuration.port == 9876)
        precondition(state.configuration.launchMode == .normal)
        precondition(state.configuration.browser == .system)
        print("PASS: legacy configuration migration preserves existing settings")

        let first = try fixture(in: temporary, name: "Tavern A", version: "1.0.0")
        let second = try fixture(in: temporary, name: "Tavern B", version: "2.0.0")
        try state.importVersion(from: first)
        let firstID = state.selectedVersion!.id
        try state.setLaunchMode(.desktop)
        try state.importVersion(from: second)
        let secondID = state.selectedVersion!.id
        precondition(state.versions.count == 2)
        precondition(state.configuration.directory == second.standardizedFileURL.path)
        precondition(state.configuration.launchMode == .desktop)
        precondition(state.configuration.port == 9876)
        try state.selectVersion(firstID)
        precondition(state.selectedVersion?.version == "1.0.0")
        precondition(state.configuration.directory == first.standardizedFileURL.path)
        print("PASS: selecting versions updates the directory and preserves mode and port")

        let restored = AppState(defaults: defaults)
        precondition(restored.selectedVersion?.id == firstID)
        precondition(restored.versions.count == 2)
        precondition(restored.configuration.launchMode == .desktop)
        for mode in TavernLaunchMode.allCases {
            try restored.setLaunchMode(mode)
            precondition(AppState(defaults: defaults).configuration.launchMode == mode)
        }
        try restored.setBrowser(.firefox)
        precondition(AppState(defaults: defaults).configuration.browser == .firefox)
        print("PASS: version list, selection, launch modes, and browser survive restart")

        _ = try fixture(in: temporary, name: "Tavern A", version: "1.0.1")
        try state.importVersion(from: first)
        precondition(state.versions.count == 2)
        precondition(state.selectedVersion?.id == firstID)
        precondition(state.selectedVersion?.version == "1.0.1")
        print("PASS: reimport updates metadata without duplicating a version")

        let beforeInvalidImport = state.configuration
        do {
            try state.importVersion(from: temporary)
            preconditionFailure("Invalid directory was accepted")
        } catch TavernSelectionError.invalidDirectory { }
        let unrelated = try fixture(in: temporary, name: "Unrelated", version: "1.0.0",
                                    packageName: "unrelated-app")
        do {
            try state.importVersion(from: unrelated)
            preconditionFailure("Unrelated package was accepted")
        } catch TavernSelectionError.invalidDirectory { }
        precondition(state.configuration == beforeInvalidImport)
        precondition(state.versions.count == 2)
        do {
            try state.selectVersion(UUID())
            preconditionFailure("Missing version was accepted")
        } catch TavernSelectionError.versionMissing { }
        precondition(state.configuration == beforeInvalidImport)
        print("PASS: invalid imports and stale selections preserve existing state")

        var manual = state.configuration
        manual.directory = "/tmp/manual"
        try state.saveConfiguration(manual)
        precondition(state.configuration.selectedVersionID == nil)
        precondition(state.versionTitle == "自定义目录")
        try state.selectVersion(secondID)
        try state.selectVersion(nil)
        precondition(state.configuration.selectedVersionID == nil)
        precondition(state.configuration.directory == second.standardizedFileURL.path)
        print("PASS: manual directories clear the version association without deleting paths")
    }

    static func fixture(in root: URL, name: String, version: String,
                        packageName: String = "sillytavern") throws -> URL {
        let directory = root.appendingPathComponent(name)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        let package = try JSONSerialization.data(withJSONObject: ["name": packageName, "version": version])
        try package.write(to: directory.appendingPathComponent("package.json"))
        return directory
    }
}
