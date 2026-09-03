//! 主界面左侧导航栏。
//!
//! 结构（自上而下）：Logo 占位区 → 分割线 → 主功能导航组 →
//! 弹性留白 → 分割线 → 底部导航组。
//! 导航按钮采用「顶部图标 + 底部文字」布局，选中态使用蓝色强调高亮。

use iced::widget::{button, column, container, space};
use iced::{Alignment, Background, Border, Color, Element, Fill, Theme};
use lucide_icons::Icon;

use astra_ui::{Avatar, AvatarColor, AvatarShape, AvatarSize, WHITE, fonts, icons};

use crate::app::Message;
use crate::lang::text;
use crate::pages::Page;

/// 侧边栏固定宽度（像素）。内容区宽度 = SIDEBAR_WIDTH - 左右内边距（各 12），
/// 恰好容纳 72px 的正方形导航按钮。
const SIDEBAR_WIDTH: f32 = 96.0;

/// 导航按钮圆角（与 astra_ui 的 RADIUS_FIELD 保持一致）。
const NAV_RADIUS: f32 = 12.0;

/// 导航按钮边长（像素）。宽高相等，构成正方形，图标在上、文字在下。
const NAV_ITEM_SIZE: f32 = 72.0;

/// 主功能导航项（上方组，位于第一道与第二道分割线之间）。
const PRIMARY_PAGES: [Page; 5] = [
    Page::Home,
    Page::TavernConfig,
    Page::Version,
    Page::Extensions,
    Page::Resources,
];

/// 底部导航项（下方组，固定在侧边栏底部）。
const SECONDARY_PAGES: [Page; 2] = [Page::Console, Page::Settings];

/// 渲染主界面左侧导航栏。
pub fn sidebar(page: Page) -> Element<'static, Message> {
    let primary = PRIMARY_PAGES.iter().map(|&item| nav_button(item, page));
    let secondary = SECONDARY_PAGES.iter().map(|&item| nav_button(item, page));

    container(
        column![
            logo_section(),
            crate::theme::separator(),
            column(primary)
                .spacing(4)
                .align_x(Alignment::Center)
                .width(Fill),
            space::vertical(),
            crate::theme::separator(),
            column(secondary)
                .spacing(4)
                .align_x(Alignment::Center)
                .width(Fill),
        ]
        .spacing(10)
        .height(Fill)
        .width(Fill),
    )
    .width(SIDEBAR_WIDTH)
    .height(Fill)
    .padding([16, 12])
    .style(crate::theme::sidebar_style)
    .into()
}

/// Logo 占位区：应用图标，以及预留的「酒馆版本」「启动模式」信息位。
/// 窄侧边栏下以图标为主，文字信息居中排列。
fn logo_section() -> Element<'static, Message> {
    column![
        Avatar::new("AstraBrew")
            .fallback(icons::icon(Icon::Beer, 22, WHITE))
            .size(AvatarSize::Medium)
            .shape(AvatarShape::Rounded)
            .color(AvatarColor::Accent),
        // 预留信息位：后续接入真实的酒馆版本与启动模式数据
        text("酒馆版本 —")
            .size(10)
            .font(fonts::REGULAR)
            .style(crate::theme::subtle_text_style),
        text("启动模式 —")
            .size(10)
            .font(fonts::REGULAR)
            .style(crate::theme::subtle_text_style),
    ]
    .spacing(6)
    .align_x(Alignment::Center)
    .width(Fill)
    .into()
}

/// 渲染单个导航按钮：顶部图标 + 底部文字，选中态蓝色高亮。
///
/// iced 的 `button` 不会自动居中内容，因此将图标与文字的列包在一个
/// `Fill` 且水平/垂直居中的 `container` 中，使内容在正方形按钮内居中。
fn nav_button(page: Page, current: Page) -> Element<'static, Message> {
    let active = page == current;

    button(
        container(
            column![
                if active {
                    crate::theme::primary_icon(page.icon(), 22)
                } else {
                    crate::theme::muted_icon(page.icon(), 22)
                },
                text(page.title())
                    .size(11)
                    .font(fonts::MEDIUM)
                    .style(move |theme| iced::widget::text::Style {
                        color: Some(if active {
                            theme.palette().primary
                        } else {
                            crate::theme::text_muted(theme)
                        }),
                    }),
            ]
            .spacing(6)
            .align_x(Alignment::Center),
        )
        .width(Fill)
        .height(Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center),
    )
    .width(NAV_ITEM_SIZE)
    .height(NAV_ITEM_SIZE)
    .padding(0)
    .on_press(Message::Navigate(page))
    .style(nav_item_style(active))
    .into()
}

/// 导航按钮样式：选中态蓝色 tint 背景，悬停态浅蓝 tint，默认透明。
fn nav_item_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let hovered = matches!(status, button::Status::Hovered);
        let background = if active {
            Some(Background::Color(tint(theme.palette().primary, 0.16)))
        } else if hovered {
            Some(Background::Color(tint(theme.palette().primary, 0.10)))
        } else {
            None
        };

        button::Style {
            background,
            text_color: if active { theme.palette().primary } else { crate::theme::text_muted(theme) },
            border: Border {
                radius: NAV_RADIUS.into(),
                ..Border::default()
            },
            ..button::Style::default()
        }
    }
}

/// 以指定透明度构造一个颜色，用于生成蓝色 tint 背景。
fn tint(color: Color, alpha: f32) -> Color {
    Color::from_rgba(color.r, color.g, color.b, alpha)
}
