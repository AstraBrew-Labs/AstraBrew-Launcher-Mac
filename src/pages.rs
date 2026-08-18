//! 主界面各功能页面的路由枚举与占位视图。
//!
//! 本模块定义主界面左侧导航对应的页面枚举，并为每个页面提供居中的占位内容。
//! 后续接入真实页面时，只需在 [`page_view`] 中按 [`Page`] 分支替换占位实现。

use iced::widget::{column, container, text};
use iced::{Alignment, Element, Fill};
use lucide_icons::Icon;

use astra_ui::{INK_MUTED, fonts};

use crate::app::Message;

/// 主界面导航页面。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    /// 主页
    Home,
    /// 酒馆配置
    TavernConfig,
    /// 版本管理
    Version,
    /// 扩展管理
    Extensions,
    /// 资源管理
    Resources,
    /// 控制台
    Console,
    /// 设置
    Settings,
}

impl Page {
    /// 页面在导航栏中的中文标题。
    pub const fn title(self) -> &'static str {
        match self {
            Page::Home => "主页",
            Page::TavernConfig => "酒馆配置",
            Page::Version => "版本管理",
            Page::Extensions => "扩展管理",
            Page::Resources => "资源管理",
            Page::Console => "控制台",
            Page::Settings => "设置",
        }
    }

    /// 页面在导航栏中对应的 Lucide 图标。
    pub const fn icon(self) -> Icon {
        match self {
            Page::Home => Icon::House,
            Page::TavernConfig => Icon::SlidersHorizontal,
            Page::Version => Icon::GitBranch,
            Page::Extensions => Icon::Puzzle,
            Page::Resources => Icon::FolderOpen,
            Page::Console => Icon::SquareTerminal,
            Page::Settings => Icon::Settings,
        }
    }
}

/// 渲染指定页面的内容。
///
/// 当前为占位视图：居中显示页面标题与说明，后续替换为各页面的真实实现。
pub fn page_view(page: Page) -> Element<'static, Message> {
    container(
        column![
            text(page.title()).size(20).font(fonts::BOLD),
            text("该页面功能开发中")
                .size(12)
                .font(fonts::REGULAR)
                .color(INK_MUTED),
        ]
        .spacing(8)
        .align_x(Alignment::Center),
    )
    .width(Fill)
    .height(Fill)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}
