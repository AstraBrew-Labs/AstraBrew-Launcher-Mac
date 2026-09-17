//! 统一样式的页码分页栏。
//!
//! 只用「上一页 / 下一页」翻页时，条目一多就得反复点击；这里提供带页码按钮的分页栏，
//! 可直接跳到任意页。样式走项目自己的 `crate::theme::button_style`，不使用 astra_ui
//! 自带的分页组件（它的翻页标签是硬编码英文，配色也是固定浅色常量）。

use iced::widget::{button, container, mouse_area, row, scrollable, space};
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Length, Padding, Shadow, Theme, Vector,
};
use lucide_icons::Icon;

use astra_ui::{ButtonVariant, WHITE};

use crate::lang::text;
use crate::theme::button_style;

/// 分页栏最多直接铺开的页数；超过后改为「首页 … 当前页附近 … 末页」。
const PAGE_WINDOW_LIMIT: usize = 7;

/// 页码按钮尺寸。
const PAGE_BUTTON_SIZE: f32 = 30.0;

/// 分页栏最右侧页码指示（`3/5`）的固定宽度。
///
/// 固定宽度是必须的：悬浮分页收起时露出的正是这一格，宽度固定才能算出精确的收起宽度，
/// 也不会因为页码位数变化而露出旁边的按钮。
const PAGE_INDICATOR_WIDTH: f32 = 44.0;

/// 收起状态露在外面的宽度：正好是页码指示那一格（指示宽度 + 行间距 + 面板内边距）。
const FLOATING_HANDLE_WIDTH: f32 = PAGE_INDICATOR_WIDTH + 4.0 + 16.0;

/// 展开状态允许的最大宽度：覆盖最宽的分页栏（7 个页码项约 450）+ 页码指示。
/// 只是上限——面板宽度始终取内容自然宽度，因此展开到位后不会多出空白。
const FLOATING_VIEW_WIDTH: f32 = 560.0;

/// 悬浮分页距离父容器右下角的间距。
const FLOATING_INSET: f32 = 10.0;

/// 分页栏中的一项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageItem {
    Page(usize),
    Ellipsis,
}

/// 生成分页栏：`[‹ 上一页] 1 2 3 … N [下一页 ›]`。
///
/// `current_page` 与 `on_page` 都使用**从 0 开始**的页索引，页码展示时再加一。
/// 返回的是普通行，需要居中的调用方自己套一层容器。
pub fn pagination<'a, Message: Clone + 'a>(
    current_page: usize,
    page_count: usize,
    on_page: impl Fn(usize) -> Message + Clone + 'a,
) -> Element<'a, Message> {
    let total = page_count.max(1);
    let current = current_page.min(total - 1);
    let displayed = current + 1;
    let mut items = vec![page_step(
        Icon::ChevronLeft,
        "上一页",
        (current > 0).then(|| on_page(current - 1)),
    )];
    for item in page_items(displayed, total) {
        items.push(match item {
            // 当前页不可点击，其余页码点击后跳到对应页。
            PageItem::Page(page) if page == displayed => active_page_number(page),
            PageItem::Page(page) => page_number(page, on_page.clone()(page - 1)),
            PageItem::Ellipsis => container(crate::theme::subtle_icon(Icon::Ellipsis, 15))
                .width(PAGE_BUTTON_SIZE)
                .height(PAGE_BUTTON_SIZE)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .into(),
        });
    }
    items.push(page_step(
        Icon::ChevronRight,
        "下一页",
        (current + 1 < total).then(|| on_page(current + 1)),
    ));
    row(items).spacing(4).align_y(Alignment::Center).into()
}

/// 悬浮在右下角的分页层，由调用方用 `stack!` 叠在内容之上。
///
/// - `progress`：0 = 完全收起（只露出最右侧一段），1 = 完全展开；由调用方按帧推进。
/// - `on_page`：点击页码 / 翻页按钮（0 起页索引）；`on_hover`：鼠标进入 / 离开该区域。
///
/// 整个过渡只有**一个**动作：面板宽度从右端的页码指示那一格连续放宽到整条分页栏，
/// 分页栏始终贴右、由 `scrollable` 按视口裁掉左侧未展开的部分，因此不会出现
/// 「内容整体换掉 + 再位移」这种两段式观感，也不会画到面板之外。
///
/// 收起态露出的固定是页码指示（`3/5`）那一格，而不是某颗按钮：文案中性、末页也不会
/// 变成禁用态，看起来就是一个收起来的拉手。
///
/// 两个实现要点，同时解决了「刚滑出又收回去」的抽搐：
///
/// 1. 视口由 `scrollable` 提供：它是 iced 里少数会真正裁剪内容的控件，
///    配合 `anchor_right()` 让分页栏贴住右端，宽度变化时右端固定、左侧逐段露出。
/// 2. 宽度用「上限」而非「位移」表达，且内容的自然宽度靠 `Shrink` 取得：
///    展开到位时面板正好等于分页栏宽度，不会多出空白，动画中间也不会挤扁按钮。
///    悬停区就是这个面板本身，右端固定且指针始终在区域内部，不会误发 `on_exit`。
pub fn floating<'a, Message: Clone + 'a>(
    current_page: usize,
    page_count: usize,
    progress: f32,
    on_page: impl Fn(usize) -> Message + Clone + 'a,
    on_hover: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    if page_count <= 1 {
        // 只有一页时仍占据浮层位置，避免控件树随浮层出现 / 消失而重建。
        return container(space::Space::new())
            .width(Fill)
            .height(Fill)
            .into();
    }

    let total = page_count.max(1);
    let current = current_page.min(total - 1);
    let progress = progress.clamp(0.0, 1.0);
    let max_width =
        FLOATING_HANDLE_WIDTH + progress * (FLOATING_VIEW_WIDTH - FLOATING_HANDLE_WIDTH);
    // 页码指示固定在最右端：收起时露出的就是它，展开后它作为整条栏的页码信息保留。
    let bar = row![
        pagination(current_page, page_count, on_page),
        page_indicator(current + 1, total),
    ]
    .spacing(4)
    .align_y(Alignment::Center);
    let drawer = scrollable(bar)
        .direction(scrollable::Direction::Horizontal(
            scrollable::Scrollbar::hidden(),
        ))
        .anchor_right()
        // Shrink 让滚动内容不被视口宽度压缩，同时给出分页栏的真实宽度。
        .width(Length::Shrink)
        .height(Length::Shrink);
    let panel = container(drawer)
        .width(Length::Shrink)
        .max_width(max_width)
        .padding([6.0, 8.0])
        .style(floating_surface);

    container(
        mouse_area(panel)
            .on_enter(on_hover(true))
            .on_exit(on_hover(false)),
    )
    .width(Fill)
    .height(Fill)
    .align_x(Alignment::End)
    .align_y(Alignment::End)
    .padding(Padding {
        right: FLOATING_INSET,
        bottom: FLOATING_INSET,
        ..Padding::default()
    })
    .into()
}

/// 浮层底色：与其它浮起的容器一样跟随主题，并带一点阴影把它和内容分开。
fn floating_surface(theme: &Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(crate::theme::surface(theme))),
        border: Border {
            color: crate::theme::line(theme),
            width: 1.0,
            radius: 10.0.into(),
        },
        shadow: Shadow {
            color: Color::from_rgba(
                0.0,
                0.0,
                0.0,
                if crate::theme::is_dark(theme) {
                    0.42
                } else {
                    0.16
                },
            ),
            offset: Vector::new(0.0, 6.0),
            blur_radius: 20.0,
        },
        ..container::Style::default()
    }
}

/// 分页栏右端的页码指示（`3/5`）。
///
/// 悬浮分页收起后露出的就是这一格：文案中性、末页时也不会变成禁用态，
/// 比直接露出「下一页」按钮更像一个拉手。
fn page_indicator<'a, Message: Clone + 'a>(page: usize, total: usize) -> Element<'a, Message> {
    container(
        text(format!("{page}/{total}"))
            .size(11)
            .font(crate::core::typography::medium())
            .style(crate::theme::muted_text_style),
    )
    .width(PAGE_INDICATOR_WIDTH)
    .height(PAGE_BUTTON_SIZE)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .into()
}

/// 当前页码：主色实心块，不参与点击。
///
/// 用容器而不是按钮：iced 会把没有 `on_press` 的按钮判为禁用态并淡化主色。
fn active_page_number<'a, Message: Clone + 'a>(page: usize) -> Element<'a, Message> {
    container(
        text(page.to_string())
            .size(12)
            .font(crate::core::typography::medium())
            .color(WHITE),
    )
    .width(PAGE_BUTTON_SIZE)
    .height(PAGE_BUTTON_SIZE)
    .align_x(Alignment::Center)
    .align_y(Alignment::Center)
    .style(active_surface)
    .into()
}

/// 可跳转的页码按钮。
fn page_number<'a, Message: Clone + 'a>(page: usize, on_press: Message) -> Element<'a, Message> {
    button(
        container(
            text(page.to_string())
                .size(12)
                .font(crate::core::typography::medium()),
        )
        .width(Fill)
        .height(Fill)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center),
    )
    .width(PAGE_BUTTON_SIZE)
    .height(PAGE_BUTTON_SIZE)
    .padding(0)
    .style(button_style(ButtonVariant::Outline))
    .on_press(on_press)
    .into()
}

/// 上一页 / 下一页按钮；`target` 为 `None` 时按钮自动进入禁用态。
fn page_step<'a, Message: Clone + 'a>(
    icon: Icon,
    label: &'static str,
    target: Option<Message>,
) -> Element<'a, Message> {
    // 图标跟随主题取色：可跳转时用主色，处在两端不可用时退为次要色。
    let glyph: Element<'a, Message> = if target.is_some() {
        crate::theme::primary_icon(icon, 15)
    } else {
        crate::theme::subtle_icon(icon, 15)
    };
    let content = button(
        row![
            glyph,
            text(label).size(12).font(crate::core::typography::medium()),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding([6, 10])
    .style(button_style(ButtonVariant::Outline));
    match target {
        Some(message) => content.on_press(message).into(),
        None => content.into(),
    }
}

/// 当前页码的主色实心背景，随主题的 primary 取色。
fn active_surface(theme: &Theme) -> container::Style {
    let primary = theme.palette().primary;
    container::Style {
        background: Some(Background::Color(primary)),
        border: Border {
            color: primary,
            width: 1.0,
            radius: 10.0.into(),
        },
        text_color: Some(WHITE),
        ..container::Style::default()
    }
}

/// 生成分页页码序列；页数超过窗口上限时，首尾各固定一页并用省略号连接当前页附近。
fn page_items(current: usize, page_count: usize) -> Vec<PageItem> {
    let total = page_count.max(1);
    let current = current.clamp(1, total);
    if total <= PAGE_WINDOW_LIMIT {
        return (1..=total).map(PageItem::Page).collect();
    }

    let mut items = vec![PageItem::Page(1)];
    if current > 3 {
        items.push(PageItem::Ellipsis);
    }
    let start = current.saturating_sub(1).max(2);
    let end = current.saturating_add(1).min(total - 1);
    items.extend((start..=end).map(PageItem::Page));
    if current < total.saturating_sub(2) {
        items.push(PageItem::Ellipsis);
    }
    items.push(PageItem::Page(total));
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_every_page_when_short() {
        assert_eq!(page_items(1, 1), vec![PageItem::Page(1)]);
        assert_eq!(
            page_items(3, 3),
            vec![PageItem::Page(1), PageItem::Page(2), PageItem::Page(3)]
        );
    }

    #[test]
    fn windows_long_lists_around_current_page() {
        for current in [1, 4, 12, 23, 25] {
            let items = page_items(current, 25);
            assert!(
                items.contains(&PageItem::Page(current)),
                "当前页 {current} 必须始终可见"
            );
            assert_eq!(items.first(), Some(&PageItem::Page(1)));
            assert_eq!(items.last(), Some(&PageItem::Page(25)));
            let ellipsis = items
                .iter()
                .filter(|item| matches!(item, PageItem::Ellipsis))
                .count();
            // 贴近首尾时只保留一段省略号。
            assert_eq!(ellipsis, if current <= 3 || current >= 23 { 1 } else { 2 });
            assert!(items.len() <= 9, "页码项数量必须受控");
        }
    }

    #[test]
    fn page_index_is_clamped_into_range() {
        // 越界页码按末页渲染，不会构造出无效页码。
        let items = page_items(99, 3);
        assert_eq!(items.last(), Some(&PageItem::Page(3)));
        assert_eq!(items.len(), 3);
    }
}
