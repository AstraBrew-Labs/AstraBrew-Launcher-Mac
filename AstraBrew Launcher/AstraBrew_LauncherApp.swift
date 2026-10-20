//
//  AstraBrew_LauncherApp.swift
//  AstraBrew Launcher
//
//  Created by 小阿狼是也 on 2026/10/20.
//

import SwiftUI

@main
struct AstraBrew_LauncherApp: App {
    @StateObject private var appState = AppState()
    @AppStorage("appearance") private var appearance = AppAppearance.system.rawValue

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(appState)
                .preferredColorScheme(AppAppearance(rawValue: appearance)?.colorScheme)
                .frame(minWidth: 940, minHeight: 640)
        }
        .defaultSize(width: 1180, height: 780)
        .windowResizability(.contentMinSize)
        .commands {
            CommandGroup(replacing: .newItem) { }
            CommandGroup(replacing: .appSettings) {
                Button("设置…") {
                    appState.selection = .settings
                }
                .keyboardShortcut(",", modifiers: .command)
            }
            CommandGroup(replacing: .appInfo) {
                Button("关于 AstraBrew Launcher") {
                    appState.selection = .about
                }
            }
        }
    }
}
