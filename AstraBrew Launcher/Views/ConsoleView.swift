import AppKit
import SwiftUI

struct ConsoleView: View {
    @EnvironmentObject private var appState: AppState
    @State private var query = ""
    @State private var level: LogEntry.Level?

    private var filteredLogs: [LogEntry] {
        appState.logs.filter {
            (level == nil || $0.level == level) &&
            (query.isEmpty || $0.message.localizedCaseInsensitiveContains(query))
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            PageHeading(title: "控制台", subtitle: "启动器日志")
                .padding(32)

            HStack(spacing: 12) {
                Picker("级别", selection: $level) {
                    Text("全部级别").tag(nil as LogEntry.Level?)
                    ForEach(LogEntry.Level.allCases) { value in
                        Text(value.rawValue).tag(Optional(value))
                    }
                }
                .frame(width: 160)
                TextField("筛选日志", text: $query).textFieldStyle(.roundedBorder)
                Button {
                    let text = filteredLogs.map { entry in
                        let timestamp = appState.showsTimestamps
                            ? "[\(entry.date.formatted(date: .omitted, time: .standard))] " : ""
                        return "\(timestamp)[\(entry.level.rawValue)] \(entry.message)"
                    }.joined(separator: "\n")
                    NSPasteboard.general.clearContents()
                    NSPasteboard.general.setString(text, forType: .string)
                } label: {
                    Label("复制日志", systemImage: "doc.on.doc")
                }
                .labelStyle(.iconOnly)
                .help("复制筛选后的日志")
                .disabled(filteredLogs.isEmpty)

                Button(role: .destructive) {
                    appState.clearLogs()
                } label: {
                    Label("清空日志", systemImage: "trash")
                }
                .labelStyle(.iconOnly)
                .help("清空日志")
                .disabled(appState.logs.isEmpty)
            }
            .padding(.horizontal, 24)
            .padding(.bottom, 16)

            Divider()
            if filteredLogs.isEmpty {
                ContentUnavailableView(
                    appState.logs.isEmpty ? "暂无日志" : "没有匹配的日志",
                    systemImage: "terminal"
                )
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            } else {
                ScrollView {
                    LazyVStack(alignment: .leading, spacing: 12) {
                        ForEach(filteredLogs) { entry in
                            HStack(alignment: .top, spacing: 14) {
                                if appState.showsTimestamps {
                                    Text(entry.date, format: .dateTime.hour().minute().second())
                                        .foregroundStyle(.tertiary)
                                        .frame(width: 76, alignment: .leading)
                                }
                                Text(entry.level.rawValue)
                                    .foregroundStyle(entry.level == .warning ? Color.orange : .secondary)
                                    .frame(width: 32, alignment: .leading)
                                Text(entry.message)
                                    .frame(maxWidth: .infinity, alignment: .leading)
                            }
                        }
                    }
                    .font(.system(size: 12, design: .monospaced))
                    .textSelection(.enabled)
                    .padding(24)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .background(Color(nsColor: .textBackgroundColor))
            }
            Divider()
            HStack {
                Text("\(filteredLogs.count) 条日志")
                Spacer()
                Text("当前会话")
            }
            .font(.caption)
            .foregroundStyle(.secondary)
            .padding(.horizontal, 24)
            .padding(.vertical, 10)
        }
    }
}
