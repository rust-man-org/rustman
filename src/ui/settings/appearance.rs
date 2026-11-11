use iced::border::Radius;
use iced::widget::{Space, column, container, row, stack, text};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow};

use crate::app::AppState;
use crate::message::{AppMsg, MenuId, Message, SettingsMsg};
use crate::ui::components::menu::{self, Align, Item};
use crate::ui::icons;
use crate::ui::theme::{
    CONTROL_LG, CONTROL_MD, HAIRLINE, ICON_SM, RADIUS_SM, SPACE_2XL, SPACE_LG, SPACE_MD,
    SPACE_XL, SPACE_XS, TEXT_SM, THEMES, Tokens,
};

use super::{rule, section, setting_row};

const COLUMNS: usize = 4;
const GAP: f32 = SPACE_LG;
/// The preview fill; with the hairline under it the preview is 68 px.
const PREVIEW_HEIGHT: f32 = 67.0;
const PREVIEW_RAIL: f32 = 10.0;
const PREVIEW_SIDEBAR: f32 = 34.0;
const PREVIEW_URL: f32 = 12.0;
const PREVIEW_SEND: f32 = 18.0;
const BAR: f32 = 3.0;
const BAR_RADIUS: f32 = 2.0;
const SMALL_RADIUS: f32 = 3.0;
/// Interface zoom levels offered by the menu.
pub const ZOOM_LEVELS: [f64; 8] = [0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];
const ZOOM_MENU_WIDTH: f32 = 120.0;

pub fn view<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let themes = grid(
        THEMES
            .iter()
            .enumerate()
            .map(|(i, spec)| theme_card(t, i, spec.name, i == state.theme_idx))
            .collect(),
    );
    let layouts = grid(vec![
        layout_card(t, "Left / Right", true, state.horizontal_layout),
        layout_card(t, "Top / Bottom", false, !state.horizontal_layout),
    ]);
    column![
        section(t, "Theme", "Each theme is a full token set, so method and status colors stay readable in all eight.", themes),
        Space::new().height(SPACE_2XL),
        section(t, "Layout", "Where the response goes. Side by side is the default from 1280 px wide.", layouts),
        Space::new().height(SPACE_2XL),
        rule(t),
        Space::new().height(SPACE_XL),
        setting_row(t, "Interface zoom", "Ctrl = and Ctrl − to change, Ctrl 0 to reset", zoom(state, t)),
    ]
    .into()
}

/// Cards in rows of [`COLUMNS`] equal columns; a short last row keeps the
/// same column widths.
fn grid<'a>(cards: Vec<Element<'a, Message>>) -> Element<'a, Message> {
    let mut rows = column![].spacing(GAP);
    let mut cards = cards.into_iter().peekable();
    while cards.peek().is_some() {
        let mut line = row![].spacing(GAP);
        let mut filled = 0;
        for card in cards.by_ref().take(COLUMNS) {
            line = line.push(container(card).width(Length::FillPortion(1)));
            filled += 1;
        }
        for _ in filled..COLUMNS {
            line = line.push(Space::new().width(Length::FillPortion(1)));
        }
        rows = rows.push(line);
    }
    rows.into()
}

/// A selectable card: a preview on top, its name and a check below.
fn card<'a>(t: &'static Tokens, preview: Element<'a, Message>, name: &'static str, on: bool, message: Message) -> Element<'a, Message> {
    let mut label = row![text(name).size(TEXT_SM).color(t.text), Space::new().width(Length::Fill)].align_y(Alignment::Center);
    if on {
        label = label.push(icons::check().size(ICON_SM).color(t.accent_text));
    }
    let label = container(label).height(CONTROL_LG).align_y(Alignment::Center).padding(Padding {
        left: SPACE_LG,
        right: SPACE_MD,
        ..Padding::ZERO
    });
    let ring = container(Space::new().width(Length::Fill).height(Length::Fill)).style(move |_| container::Style {
        border: Border {
            color: if on { t.accent } else { Color::TRANSPARENT },
            width: 2.0 * HAIRLINE,
            radius: RADIUS_SM.into(),
        },
        ..Default::default()
    });
    let button = iced::widget::button(column![preview, label])
        .on_press(message)
        .width(Length::Fill)
        .padding(HAIRLINE)
        .style(move |_, status| {
            use iced::widget::button::Status;
            let color = if matches!(status, Status::Hovered | Status::Pressed) {
                t.border_strong
            } else {
                t.on_raised.border
            };
            iced::widget::button::Style {
                background: None,
                text_color: t.text,
                border: Border {
                    color,
                    width: HAIRLINE,
                    radius: RADIUS_SM.into(),
                },
                shadow: Shadow::default(),
                ..Default::default()
            }
        });
    stack![button, ring].into()
}

/// The preview area: the given fill, rounded only at the card's top, with
/// a hairline under it.
fn preview_frame<'a>(fill: Color, line: Color, content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .height(PREVIEW_HEIGHT)
        .style(move |_| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                color: line,
                width: 0.0,
                radius: Radius {
                    top_left: RADIUS_SM - HAIRLINE,
                    top_right: RADIUS_SM - HAIRLINE,
                    bottom_right: 0.0,
                    bottom_left: 0.0,
                },
            },
            ..Default::default()
        })
        .into()
}

fn theme_card<'a>(t: &'static Tokens, index: usize, name: &'static str, on: bool) -> Element<'a, Message> {
    let p = Tokens::for_theme(index);
    let bar = |width: f32, color: Color| -> Element<'a, Message> {
        container(Space::new().width(width).height(BAR))
            .style(move |_| container::Style {
                background: Some(Background::Color(color)),
                border: Border {
                    radius: BAR_RADIUS.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .into()
    };
    let divided = |content: Element<'a, Message>, width: f32| -> Element<'a, Message> {
        row![content, vline(p.border)].width(width + HAIRLINE).into()
    };
    let side = column![bar(18.0, p.methods.get), bar(14.0, p.methods.post), bar(20.0, p.text_subtle), bar(12.0, p.methods.delete)]
        .spacing(SPACE_XS + 2.0)
        .padding([SPACE_MD, 5.0]);
    let url = container(row![Space::new().width(Length::Fill), container(Space::new().width(PREVIEW_SEND).height(Length::Fill)).style(move |_| container::Style {
        background: Some(Background::Color(p.accent_fill)),
        ..Default::default()
    })])
    .height(PREVIEW_URL)
    .width(Length::Fill)
    .style(move |_| container::Style {
        background: Some(Background::Color(p.surface)),
        border: Border {
            color: p.border,
            width: HAIRLINE,
            radius: SMALL_RADIUS.into(),
        },
        ..Default::default()
    })
    .clip(true);
    let code = |key: f32, value: f32, color: Color| row![bar(key, p.syntax.key), bar(value, color)].spacing(SPACE_XS);
    let main = column![
        url,
        code(22.0, 30.0, p.syntax.string),
        code(16.0, 14.0, p.syntax.number),
        code(26.0, 20.0, p.syntax.literal),
    ]
    .spacing(SPACE_XS + 2.0)
    .padding(SPACE_MD)
    .width(Length::Fill);
    let preview = column![
        preview_frame(
            p.canvas,
            t.on_raised.border,
            row![
                divided(Space::new().width(PREVIEW_RAIL).height(Length::Fill).into(), PREVIEW_RAIL),
                divided(side.width(PREVIEW_SIDEBAR).into(), PREVIEW_SIDEBAR),
                main,
            ]
        ),
        hline(t.on_raised.border),
    ];
    card(t, preview.into(), name, on, Message::Settings(SettingsMsg::ThemeChanged(index)))
}

fn layout_card<'a>(t: &'static Tokens, name: &'static str, horizontal: bool, on: bool) -> Element<'a, Message> {
    let pane = || -> Element<'a, Message> {
        container(Space::new().width(Length::Fill).height(Length::Fill))
            .style(move |_| container::Style {
                background: Some(Background::Color(t.surface)),
                border: Border {
                    color: t.border_strong,
                    width: HAIRLINE,
                    radius: SMALL_RADIUS.into(),
                },
                ..Default::default()
            })
            .into()
    };
    let panes: Element<'a, Message> = if horizontal {
        row![pane(), pane()].spacing(SPACE_XS).into()
    } else {
        column![pane(), pane()].spacing(SPACE_XS).into()
    };
    let preview = column![
        preview_frame(t.canvas, t.on_raised.border, container(panes).padding(SPACE_MD)),
        hline(t.on_raised.border),
    ];
    let message = if on {
        Message::App(AppMsg::Noop)
    } else {
        Message::Settings(SettingsMsg::LayoutDirectionToggled)
    };
    card(t, preview.into(), name, on, message)
}

fn zoom<'a>(state: &'a AppState, t: &'static Tokens) -> Element<'a, Message> {
    let open = state.chrome.open_menu == Some(MenuId::SettingsZoom);
    let label = row![
        text(zoom_label(state.ui_scale)).size(TEXT_SM).color(t.text),
        icons::chevron_down().size(ICON_SM).color(t.on_raised.text_muted),
    ]
    .spacing(SPACE_MD)
    .align_y(Alignment::Center);
    let trigger = iced::widget::button(container(label).center_y(Length::Fill))
        .on_press(Message::App(AppMsg::MenuToggled(MenuId::SettingsZoom)))
        .height(CONTROL_MD)
        .padding(Padding {
            left: SPACE_LG + HAIRLINE,
            right: SPACE_MD + HAIRLINE,
            ..Padding::ZERO
        })
        .style(move |_, status| {
            use iced::widget::button::Status;
            iced::widget::button::Style {
                background: Some(Background::Color(t.surface)),
                text_color: t.text,
                border: Border {
                    color: if open || matches!(status, Status::Hovered) { t.border_strong } else { t.border },
                    width: HAIRLINE,
                    radius: RADIUS_SM.into(),
                },
                ..Default::default()
            }
        });
    let panel = open.then(|| {
        let items = ZOOM_LEVELS
            .iter()
            .map(|&level| {
                Item::new(zoom_label(level), Message::Settings(SettingsMsg::ZoomSelected(level)))
                    .checked((state.ui_scale - level).abs() < f64::EPSILON)
                    .into()
            })
            .collect();
        menu::panel(t, items, ZOOM_MENU_WIDTH)
    });
    menu::popover(trigger, panel, Align::End, Message::App(AppMsg::MenuDismissed))
}

/// "100%".
pub fn zoom_label(scale: f64) -> String {
    format!("{:.0}%", scale * 100.0)
}

fn vline<'a>(color: Color) -> Element<'a, Message> {
    container(Space::new().width(HAIRLINE).height(Length::Fill))
        .style(move |_| container::Style {
            background: Some(Background::Color(color)),
            ..Default::default()
        })
        .into()
}

fn hline<'a>(color: Color) -> Element<'a, Message> {
    container(Space::new().width(Length::Fill).height(HAIRLINE))
        .style(move |_| container::Style {
            background: Some(Background::Color(color)),
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_levels_read_as_percentages() {
        assert_eq!(zoom_label(1.0), "100%");
        assert_eq!(zoom_label(1.25), "125%");
        assert!(ZOOM_LEVELS.contains(&1.0), "the default is one of the choices");
    }
}
