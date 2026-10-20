import SwiftUI

struct BrandMark: View {
    var size: CGFloat = 48

    var body: some View {
        Image("LauncherIcon")
            .resizable()
            .scaledToFit()
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }
}

struct BrandLogo: View {
    var body: some View {
        Image("LauncherLogo")
            .resizable()
            .scaledToFit()
            .accessibilityLabel("星酿启动器")
    }
}

struct PageHeading: View {
    let title: String
    let subtitle: String

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            Text(title).font(.system(size: 26, weight: .bold))
            Text(subtitle).font(.callout).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
    }
}

struct DetailRow: View {
    let title: String
    let value: String

    var body: some View {
        LabeledContent(title) {
            Text(value)
                .foregroundStyle(.secondary)
                .lineLimit(2)
                .textSelection(.enabled)
                .multilineTextAlignment(.trailing)
        }
        .padding(.vertical, 6)
    }
}

struct NavigationRow: View {
    let page: LauncherPage
    let subtitle: String
    @EnvironmentObject private var appState: AppState

    var body: some View {
        Button {
            appState.selection = page
        } label: {
            HStack(spacing: 14) {
                Image(systemName: page.symbol)
                    .font(.system(size: 18))
                    .foregroundStyle(Color.accentColor)
                    .frame(width: 28)
                VStack(alignment: .leading, spacing: 4) {
                    Text(page.title).font(.system(size: 13, weight: .medium))
                    Text(subtitle).font(.caption).foregroundStyle(.secondary)
                }
                Spacer()
                Image(systemName: "chevron.right")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.tertiary)
            }
            .padding(.vertical, 14)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
