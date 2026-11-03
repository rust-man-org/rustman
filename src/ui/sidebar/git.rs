use iced::{
    Background, Border, Color, Element, Length, Padding,
    widget::{Space, button, column, container, row, scrollable, text, text_input},
};

use crate::{
    app::AppState,
    message::{GitMsg, Message, RepoSummary},
    services::repos::LOCAL_ID,
    ui::{
        icons,
        theme::{MONO, Palette},
        tree_label::{tree_label, tree_label_styled},
    },
};

const SIDE: f32 = 12.0;

pub fn view(state: &AppState) -> Element<'_, Message> {
    let mut root = column![
        title_bar(),
        repo_card(state),
        changes_card(state),
        history_card(state),
        repos_card(state),
    ]
    .spacing(6);

    if state.git_busy {
        root = root.push(
            container(text("Working...").size(11).color(Palette::accent()))
                .padding([3, 14])
                .width(Length::Fill),
        );
    }

    scrollable(root)
        .height(Length::Fill)
        .spacing(8)
        .style(crate::ui::theme::hidden_scrollbar)
        .into()
}

fn title_bar() -> Element<'static, Message> {
    container(
        row![
            text("Source Control")
                .size(crate::ui::theme::TEXT_LG)
                .color(Palette::text())
                .font(crate::ui::theme::UI_FONT_MEDIUM),
            Space::new().width(Length::Fill),
            button(text("↻").size(14).color(Palette::text_muted()))
                .on_press(Message::Git(GitMsg::Refresh))
                .style(iced::widget::button::text)
                .padding([2, 6]),
        ]
        .align_y(iced::Alignment::Center),
    )
    .padding([8, SIDE as u16])
    .width(Length::Fill)
    .into()
}

// ── Card wrapper ──

fn card<'a>(content: impl Into<Element<'a, Message>>) -> container::Container<'a, Message> {
    container(content)
        .width(Length::Fill)
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::surface())),
            border: Border {
                color: Palette::border_subtle(),
                width: 1.0,
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
}

fn card_header(
    icon: char,
    label: &'static str,
    count: Option<usize>,
    actions: Option<Element<'static, Message>>,
) -> Element<'static, Message> {
    let mut h = row![
        text(icon)
            .size(13)
            .color(Palette::text_muted())
            .font(icons::ICON_FONT),
        text(label).size(11).color(Palette::text()),
    ]
    .spacing(5)
    .align_y(iced::Alignment::Center);
    if let Some(n) = count {
        h = h.push(count_badge(n));
    }
    h = h.push(Space::new().width(Length::Fill));
    if let Some(a) = actions {
        h = h.push(a);
    }
    container(h)
        .padding(Padding {
            top: 0.0,
            right: 0.0,
            bottom: 6.0,
            left: 0.0,
        })
        .width(Length::Fill)
        .into()
}

// ── Repository card (branch + status + commit + sync) ──

fn repo_card(state: &AppState) -> Element<'_, Message> {
    let branch = state
        .git_status
        .as_ref()
        .map(|s| s.branch.clone())
        .unwrap_or_else(|| "main".to_owned());
    let changes = state
        .git_status
        .as_ref()
        .map(|s| s.changes.len())
        .unwrap_or(0);
    let (ahead, behind) = state
        .git_status
        .as_ref()
        .map(|s| (s.ahead, s.behind))
        .unwrap_or((0, 0));
    let has_remote = state
        .git_status
        .as_ref()
        .and_then(|s| s.remote_url.as_ref())
        .is_some();

    // Status row: branch + clean/dirty + ahead/behind
    let status_color = if changes == 0 {
        Palette::SUCCESS
    } else {
        Palette::WARNING
    };
    let status_text: String = if changes == 0 {
        "Clean".to_owned()
    } else {
        format!("{} modified", changes)
    };

    let mut status_row = row![].spacing(4).align_y(iced::Alignment::Center);
    status_row = status_row.push(icons::git_branch().size(11).color(Palette::text_muted()));
    // Long branch names clip + popover instead of wrapping the row taller.
    // The label is Fill so it right-aligns the status badge on its own.
    status_row = status_row.push(tree_label_styled(
        branch,
        11.0,
        state.sidebar_width - 115.0,
        Some(MONO),
        Palette::text(),
    ));

    if changes > 0 {
        status_row = status_row.push(
            container(text("\u{25CF}").size(8).color(Palette::WARNING)).padding(Padding {
                top: 0.0,
                right: 2.0,
                bottom: 0.0,
                left: 0.0,
            }),
        );
    }

    status_row = status_row.push(
        container(text(status_text).size(9).color(status_color))
            .style(move |_| container::Style {
                background: Some(Background::Color(soft_bg(status_color))),
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .padding([1, 5]),
    );

    // Commit area
    let can_commit = !state.git_user_name.is_empty() && !state.git_commit_message.trim().is_empty();
    let msg_input = text_input("Summary", &state.git_commit_message)
        .on_input(|s| Message::Git(GitMsg::CommitMessageChanged(s)))
        .on_submit(Message::Git(GitMsg::Commit))
        .size(12)
        .padding([7, 10])
        .width(Length::Fill)
        .style(crate::ui::styles::field_input);

    let commit_btn = button(
        container(
            row![
                text("\u{2713}").size(11).color(Color::WHITE),
                text(" Commit").size(11).color(Color::WHITE),
            ]
            .align_y(iced::Alignment::Center),
        )
        .center_x(Length::Fill)
        .center_y(Length::Fill),
    )
    .on_press_maybe(if can_commit {
        Some(Message::Git(GitMsg::Commit))
    } else {
        None
    })
    .style(commit_style)
    // Fixed height + vertically centred content: iced places button content at
    // the top padding edge and adds no centering of its own (issue #29).
    .height(Length::Fixed(
        crate::ui::widgets::centered_button::BUTTON_HEIGHT,
    ))
    .padding([0, 14])
    .width(Length::Fill);

    // Sync row
    let mut sync_row = row![].spacing(4);
    if ahead > 0 {
        sync_row = sync_row.push(
            text(format!("↑{ahead}"))
                .size(9)
                .color(Palette::accent())
                .font(MONO),
        );
    }
    if behind > 0 {
        sync_row = sync_row.push(
            text(format!("↓{behind}"))
                .size(9)
                .color(Palette::WARNING)
                .font(MONO),
        );
    }
    sync_row = sync_row.push(Space::new().width(Length::Fill));
    if has_remote {
        sync_row = sync_row.push(ghost_button("Fetch", Message::Git(GitMsg::Fetch)));
        sync_row = sync_row.push(ghost_button("Pull", Message::Git(GitMsg::Pull)));
        sync_row = sync_row.push(primary_button("Push", Message::Git(GitMsg::Push)));
    } else if state
        .git_repos
        .iter()
        .any(|r| r.id == state.git_active_repo && r.id != LOCAL_ID)
    {
        sync_row = sync_row.push(
            row![
                text_input("Remote", &state.git_remote_input)
                    .on_input(|v| Message::Git(GitMsg::RemoteUrlChanged(v)))
                    .on_submit(Message::Git(GitMsg::SetRemote))
                    .size(11)
                    .padding([5, 8])
                    .width(Length::Fill)
                    .style(crate::ui::styles::field_input),
                primary_button("Set", Message::Git(GitMsg::SetRemote)),
            ]
            .spacing(4),
        );
    }

    // Branches (compact)
    let current_branch = state
        .git_status
        .as_ref()
        .map(|s| s.branch.clone())
        .unwrap_or_default();
    let mut branch_col = column![].spacing(1);
    for b in state.git_branches.iter().take(20) {
        let is_current = b.name == current_branch;
        let name = b.name.clone();
        let row = button(
            row![
                text("\u{25CF}").size(9).color(if is_current {
                    Palette::accent()
                } else {
                    Palette::text_subtle()
                }),
                tree_label_styled(
                    b.name.clone(),
                    11.0,
                    state.sidebar_width - 70.0,
                    Some(MONO),
                    if is_current {
                        Palette::text()
                    } else {
                        Palette::text_muted()
                    },
                ),
            ]
            .spacing(5)
            .align_y(iced::Alignment::Center),
        )
        .on_press(Message::Git(GitMsg::SwitchBranch(name)))
        .style(move |_t, status| repo_row_style(status, is_current))
        .padding([2, 4])
        .width(Length::Fill);
        branch_col = branch_col.push(row);
    }

    let new_branch = row![
        text_input("New branch", &state.git_new_branch)
            .on_input(|v| Message::Git(GitMsg::NewBranchNameChanged(v)))
            .on_submit(Message::Git(GitMsg::CreateBranch))
            .size(11)
            .padding([5, 8])
            .width(Length::Fill)
            .style(crate::ui::styles::field_input),
        Space::new().width(4),
        icon_btn("\u{2795}", Message::Git(GitMsg::CreateBranch)),
    ]
    .align_y(iced::Alignment::Center);

    card(
        column![
            card_header('\u{e0e2}', "Repository", None, None),
            status_row,
            Space::new().height(8),
            msg_input,
            Space::new().height(4),
            commit_btn,
            if !has_remote
                && state
                    .git_repos
                    .iter()
                    .any(|r| r.id == state.git_active_repo && r.id != LOCAL_ID)
            {
                container(sync_row)
                    .padding(Padding {
                        top: 6.0,
                        right: 0.0,
                        bottom: 0.0,
                        left: 0.0,
                    })
                    .into()
            } else if has_remote {
                container(sync_row)
                    .padding(Padding {
                        top: 6.0,
                        right: 0.0,
                        bottom: 0.0,
                        left: 0.0,
                    })
                    .into()
            } else {
                {
                    let e: Element<'_, Message> = Space::new().height(0).into();
                    e
                }
            },
            if !state.git_branches.is_empty() {
                container(column![
                    Space::new().height(8),
                    text("Branches").size(9).color(Palette::text_muted()),
                    branch_col,
                    new_branch,
                ])
                .into()
            } else {
                {
                    let e: Element<'_, Message> = Space::new().height(0).into();
                    e
                }
            },
        ]
        .spacing(0)
        .padding([10, SIDE as u16]),
    )
    .into()
}

// ── Changes card ──

fn changes_card(state: &AppState) -> Element<'_, Message> {
    let changes = state
        .git_status
        .as_ref()
        .map(|s| s.changes.as_slice())
        .unwrap_or(&[]);

    let mut content = column![].spacing(1);

    if changes.is_empty() {
        content = content.push(
            container(
                row![
                    text("\u{2714}").size(12).color(Palette::SUCCESS),
                    text(" Working tree clean")
                        .size(11)
                        .color(Palette::text_subtle()),
                ]
                .spacing(4)
                .align_y(iced::Alignment::Center),
            )
            .padding([2, 0]),
        );
    } else {
        for change in changes.iter().take(80) {
            let name = pretty_name(state, &change.path);
            let st = &change.state;
            let line = container(
                button(
                    row![
                        badge(st),
                        tree_label(name, 12.0, state.sidebar_width - 80.0),
                    ]
                    .spacing(6)
                    .align_y(iced::Alignment::Center),
                )
                .on_press(Message::Git(GitMsg::ToggleDiff))
                .style(|_t, _s| iced::widget::button::Style::default())
                .padding([2, 4])
                .width(Length::Fill),
            )
            .width(Length::Fill);
            content = content.push(line);
        }
    }

    if let Some(diff) = &state.git_diff {
        // Long diff lines scroll horizontally instead of wrapping: wrapping
        // code destroys alignment and stretches the panel. The panel scrolls
        // vertically; this box scrolls both ways.
        let diff_container = container(
            scrollable(
                text(diff.clone())
                    .size(10)
                    .font(MONO)
                    .color(Palette::text_muted()),
            )
            .height(160)
            .direction(iced::widget::scrollable::Direction::Both {
                vertical: crate::ui::theme::grabbable_scrollbar(),
                horizontal: crate::ui::theme::grabbable_scrollbar(),
            })
            .style(crate::ui::theme::hidden_scrollbar),
        )
        .padding([6, 8])
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::background())),
            border: Border {
                color: Palette::border(),
                width: 1.0,
                radius: 6.0.into(),
                ..Default::default()
            },
            ..Default::default()
        });
        content = content.push(Space::new().height(2));
        content = content.push(diff_container);
    }

    let actions = if state.git_diff.is_some() {
        Some(tool_text("Hide", Message::Git(GitMsg::ToggleDiff)))
    } else if !changes.is_empty() {
        Some(tool_text("Diff", Message::Git(GitMsg::ToggleDiff)))
    } else {
        None
    };

    card(
        column![
            card_header('\u{e0d7}', "Changes", Some(changes.len()), actions),
            content,
        ]
        .spacing(0)
        .padding([10, SIDE as u16]),
    )
    .into()
}

// ── History card ──

fn history_card(state: &AppState) -> Element<'_, Message> {
    let search = &state.git_history_search;
    let filtered: Vec<_> = if search.is_empty() {
        state.git_log.iter().take(40).collect()
    } else {
        // `contains_ignore_ascii_case`-style matching without allocating a
        // lowercase copy of every commit message and id on every redraw (this
        // runs for the whole log, each frame, while the search box has text).
        let q = search.to_lowercase();
        state
            .git_log
            .iter()
            .filter(|c| contains_ignore_case(&c.message, &q) || contains_ignore_case(&c.id, &q))
            .take(40)
            .collect()
    };

    let search_input = row![
        icons::search().size(11).color(Palette::text_subtle()),
        text_input("Search commits", search)
            .on_input(|v| Message::Git(GitMsg::HistorySearchChanged(v)))
            .size(11)
            .padding([5, 8])
            .width(Length::Fill)
            .style(crate::ui::styles::field_input),
    ]
    .spacing(4)
    .align_y(iced::Alignment::Center);

    let mut content = column![].spacing(0);

    if filtered.is_empty() {
        content = content.push(
            container(
                text("No commits yet")
                    .size(11)
                    .color(Palette::text_subtle()),
            )
            .padding([2, 0]),
        );
    } else {
        // Read the clock once per frame instead of twice per row.
        // `date_label` and `time_ago` each called `Utc::now()`, so a 40-commit
        // history made 80 clock syscalls on every single redraw. One shared
        // "now" is also more consistent: every row is dated against the same
        // instant.
        let now = chrono::Utc::now();
        let mut last_date_group = String::new();
        for (i, commit) in filtered.iter().enumerate() {
            let hash: String = commit.id.chars().take(7).collect();
            let when = chrono::DateTime::from_timestamp(commit.timestamp, 0);
            let date_group = date_label(&when, now);
            let is_last = i == filtered.len() - 1;
            let msg_first_line = commit
                .message
                .trim()
                .lines()
                .next()
                .unwrap_or("")
                .to_owned();
            let id = commit.id.clone();

            if date_group != last_date_group {
                if !last_date_group.is_empty() {
                    content = content.push(Space::new().height(2));
                }
                content = content.push(
                    text(date_group.clone())
                        .size(9)
                        .color(Palette::text_subtle()),
                );
                last_date_group = date_group;
            }

            // Single-line commit: ● message • hash • time ago. The message
            // clips + popover instead of wrapping; the row is too dense for
            // long messages at narrow widths either way.
            let row = container(
                row![
                    column![text("\u{25CF}").size(8).color(Palette::accent())]
                        .width(14)
                        .align_x(iced::alignment::Horizontal::Center),
                    tree_label(msg_first_line, 11.0, state.sidebar_width - 160.0),
                    text(hash).size(8).color(Palette::accent()).font(MONO),
                    text(format!(" \u{00B7} {}", time_ago(commit.timestamp, now)))
                        .size(8)
                        .color(Palette::text_subtle()),
                    icon_btn("\u{21BB}", Message::Git(GitMsg::AskRestore(id))),
                ]
                .spacing(3)
                .align_y(iced::Alignment::Center),
            )
            .padding(Padding {
                top: 3.0,
                right: 0.0,
                bottom: 3.0,
                left: 0.0,
            })
            .width(Length::Fill);

            content = content.push(row);

            if !is_last {
                content = content.push(
                    container(
                        text("\u{2502}")
                            .size(10)
                            .color(Palette::border_subtle())
                            .font(MONO),
                    )
                    .padding(Padding {
                        top: 0.0,
                        right: 0.0,
                        bottom: 0.0,
                        left: 6.0,
                    })
                    .width(Length::Fill),
                );
            }
        }
    }

    card(
        column![
            card_header('\u{e1f5}', "History", Some(state.git_log.len()), None),
            search_input,
            Space::new().height(4),
            content,
        ]
        .spacing(0)
        .padding([10, SIDE as u16]),
    )
    .into()
}

// ── Repositories card ──

fn repos_card(state: &AppState) -> Element<'_, Message> {
    let mut content = column![].spacing(1);

    for repo in &state.git_repos {
        let active = repo.id == state.git_active_repo;
        let summary = summary_for(state, &repo.id);
        let branch = summary
            .map(|s| s.branch.clone())
            .unwrap_or_else(|| "-".to_owned());
        let changes = summary.map(|s| s.changes).unwrap_or(0);

        let mut line = row![
            if active {
                text("\u{25CF}").size(10).color(Palette::accent())
            } else {
                text("\u{25CB}").size(10).color(Palette::text_subtle())
            },
            tree_label(repo.name.clone(), 11.0, state.sidebar_width - 140.0),
            // Shrink-sized: short branches cost nothing, long ones clip at
            // the panel edge instead of wrapping. Single-line either way.
            text(branch)
                .size(9)
                .color(Palette::text_subtle())
                .font(MONO)
                .wrapping(iced_core::text::Wrapping::None,),
        ]
        .spacing(5)
        .align_y(iced::Alignment::Center);

        if changes > 0 {
            line = line.push(count_badge(changes));
        }

        let select = button(line)
            .on_press(Message::Git(GitMsg::SelectRepo(repo.id.clone())))
            .style(move |_t, status| repo_row_style(status, active))
            .padding([3, 6])
            .width(Length::Fill);

        let mut entry = row![select].spacing(2).align_y(iced::Alignment::Center);
        if repo.id != LOCAL_ID {
            entry = entry.push(icon_btn(
                "\u{2716}",
                Message::Git(GitMsg::RemoveRepo(repo.id.clone())),
            ));
        }
        content = content.push(entry);
    }

    let clone_form = container(
        column![
            text_input("Clone URL", &state.git_clone_url)
                .on_input(|v| Message::Git(GitMsg::CloneUrlChanged(v)))
                .on_submit(Message::Git(GitMsg::CloneRepo))
                .size(11)
                .padding([5, 8])
                .width(Length::Fill)
                .style(crate::ui::styles::field_input),
            Space::new().height(4),
            row![
                ghost_button("Clone", Message::Git(GitMsg::CloneRepo)),
                ghost_button("Open folder", Message::Git(GitMsg::OpenFolder)),
            ]
            .spacing(4),
        ]
        .spacing(0),
    );

    card(
        column![
            card_header(
                '\u{e0d7}',
                "Repositories",
                Some(state.git_repos.len()),
                None
            ),
            content,
            Space::new().height(6),
            clone_form,
        ]
        .spacing(0)
        .padding([10, SIDE as u16]),
    )
    .into()
}

// ── Helpers ──

fn summary_for<'a>(state: &'a AppState, id: &str) -> Option<&'a RepoSummary> {
    state.git_repo_summaries.iter().find(|s| s.id == id)
}

fn pretty_name(state: &AppState, path: &str) -> String {
    let id = path.trim_end_matches(".json");
    state
        .collections
        .iter()
        .find(|c| c.id == id)
        .map(|c| c.name.clone())
        .unwrap_or_else(|| path.to_owned())
}

/// Case-insensitive substring test that allocates nothing.
///
/// `needle` must already be lowercase. Used for the commit-history filter, which
/// otherwise built a fresh lowercase `String` for every commit message and id on
/// every frame the search box was non-empty.
///
/// Matching is ASCII-case-insensitive: commit hashes are ASCII, and for message
/// text this is the same trade-off `str::eq_ignore_ascii_case` makes. Non-ASCII
/// still matches exactly, so nothing becomes unsearchable.
fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let hay = haystack.as_bytes();
    let ndl = needle.as_bytes();
    if ndl.len() > hay.len() {
        return false;
    }
    hay.windows(ndl.len())
        .any(|window| window.eq_ignore_ascii_case(ndl))
}

fn count_badge(n: usize) -> Element<'static, Message> {
    container(text(n.to_string()).size(9).color(Palette::text()))
        .style(|_| container::Style {
            background: Some(Background::Color(Palette::surface_high())),
            border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .padding([1, 6])
        .into()
}

fn badge(state: &str) -> Element<'static, Message> {
    let (ch, color) = match state {
        "new" => ("A", Palette::SUCCESS),
        "deleted" => ("D", Palette::ERROR),
        "renamed" => ("R", Palette::PUT),
        _ => ("M", Palette::WARNING),
    };
    container(text(ch).size(10).color(color).font(MONO))
        .style(move |_| container::Style {
            background: Some(Background::Color(soft_bg(color))),
            border: Border {
                radius: 3.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .padding([1, 5])
        .into()
}

/// `now` is passed in so the caller can read the clock once per frame rather
/// than once per rendered row.
fn date_label(
    when: &Option<chrono::DateTime<chrono::Utc>>,
    now: chrono::DateTime<chrono::Utc>,
) -> String {
    let Some(dt) = when else {
        return "Unknown".to_owned();
    };
    if dt.date_naive() == now.date_naive() {
        "Today".to_owned()
    } else if dt.date_naive() == (now - chrono::Duration::days(1)).date_naive() {
        "Yesterday".to_owned()
    } else {
        dt.format("%b %e").to_string()
    }
}

/// `now` is passed in for the same reason as `date_label`.
fn time_ago(timestamp: i64, now: chrono::DateTime<chrono::Utc>) -> String {
    let Some(dt) = chrono::DateTime::from_timestamp(timestamp, 0) else {
        return "".to_owned();
    };
    let diff = now - dt;
    let secs = diff.num_seconds();
    if secs < 60 {
        return format!("{secs}s");
    }
    let mins = diff.num_minutes();
    if mins < 60 {
        return format!("{mins}m");
    }
    let hours = diff.num_hours();
    if hours < 24 {
        return format!("{hours}h");
    }
    let days = diff.num_days();
    if days < 7 {
        return format!("{days}d");
    }
    dt.format("%b %d").to_string()
}

fn soft_bg(color: Color) -> Color {
    if Palette::is_dark() {
        Color {
            r: color.r * 0.22,
            g: color.g * 0.22,
            b: color.b * 0.24,
            a: 1.0,
        }
    } else {
        Color {
            r: color.r + (1.0 - color.r) * 0.78,
            g: color.g + (1.0 - color.g) * 0.78,
            b: color.b + (1.0 - color.b) * 0.76,
            a: 1.0,
        }
    }
}

// ── Styles ──

fn commit_style(
    _t: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let accent = Palette::accent();
    let bg = match status {
        iced::widget::button::Status::Hovered => Palette::accent_hover(),
        iced::widget::button::Status::Disabled => Color {
            r: 0.3,
            g: 0.3,
            b: 0.3,
            a: 0.4,
        },
        _ => accent,
    };
    iced::widget::button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border {
            radius: 6.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn primary_button(label: &str, msg: Message) -> Element<'static, Message> {
    use crate::ui::widgets::centered_button::{BUTTON_HEIGHT_SM, centered_button};
    centered_button(
        text(label.to_owned()).size(11).color(Color::WHITE),
        BUTTON_HEIGHT_SM,
        10.0,
    )
    .on_press(msg)
    .style(primary_style)
    .into()
}

fn primary_style(
    _t: &iced::Theme,
    status: iced::widget::button::Status,
) -> iced::widget::button::Style {
    let accent = Palette::accent();
    let bg = match status {
        iced::widget::button::Status::Hovered => Palette::accent_hover(),
        _ => accent,
    };
    iced::widget::button::Style {
        background: Some(Background::Color(bg)),
        text_color: Color::WHITE,
        border: Border {
            radius: 5.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}

fn ghost_button(label: &str, msg: Message) -> Element<'static, Message> {
    use crate::ui::widgets::centered_button::{BUTTON_HEIGHT_SM, centered_button};
    centered_button(
        text(label.to_owned()).size(11).color(Palette::text()),
        BUTTON_HEIGHT_SM,
        8.0,
    )
    .on_press(msg)
    .style(|_t, status| {
        let hovered = matches!(status, iced::widget::button::Status::Hovered);
        iced::widget::button::Style {
            background: Some(Background::Color(if hovered {
                Palette::surface_high()
            } else {
                Palette::surface_raised()
            })),
            text_color: Palette::text(),
            border: Border {
                color: Palette::border(),
                width: 1.0,
                radius: 5.0.into(),
            },
            ..Default::default()
        }
    })
    // No .padding() here: `centered_button` already sets the horizontal
    // padding and a zero vertical padding, and a later call would override
    // both and reintroduce the off-centre label.
    .into()
}

fn icon_btn(glyph: &str, msg: Message) -> Element<'static, Message> {
    button(text(glyph.to_owned()).size(11).color(Palette::text_muted()))
        .on_press(msg)
        .style(|_t, status| {
            let hovered = matches!(status, iced::widget::button::Status::Hovered);
            iced::widget::button::Style {
                background: if hovered {
                    Some(Background::Color(Palette::surface_high()))
                } else {
                    None
                },
                text_color: if hovered {
                    Palette::text()
                } else {
                    Palette::text_muted()
                },
                ..Default::default()
            }
        })
        .padding([2, 5])
        .into()
}

fn tool_text(label: &str, msg: Message) -> Element<'static, Message> {
    button(text(label.to_owned()).size(10).color(Palette::accent()))
        .on_press(msg)
        .style(iced::widget::button::text)
        .padding([2, 6])
        .into()
}

fn repo_row_style(
    status: iced::widget::button::Status,
    active: bool,
) -> iced::widget::button::Style {
    let hovered = matches!(status, iced::widget::button::Status::Hovered);
    iced::widget::button::Style {
        background: if active {
            Some(Background::Color(Palette::surface_high()))
        } else if hovered {
            Some(Background::Color(Palette::surface_raised()))
        } else {
            None
        },
        text_color: Palette::text(),
        border: Border {
            color: if active {
                Palette::accent()
            } else {
                Color::TRANSPARENT
            },
            width: if active { 1.0 } else { 0.0 },
            radius: 4.0.into(),
        },
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::contains_ignore_case;

    #[test]
    fn search_matches_case_insensitively() {
        assert!(contains_ignore_case("Fix: Handle NULL input", "null"));
        assert!(contains_ignore_case("Fix: Handle NULL input", "fix"));
        assert!(contains_ignore_case("ABCDEF0123", "abcdef"));
        assert!(contains_ignore_case("anything", ""));
    }

    #[test]
    fn search_rejects_non_matches() {
        assert!(!contains_ignore_case("fix: typo", "feature"));
        assert!(!contains_ignore_case("short", "much longer needle"));
    }

    /// Must agree with the previous `to_lowercase().contains()` implementation
    /// on ASCII input, which is what commit hashes and most messages are.
    #[test]
    fn search_agrees_with_the_lowercase_implementation() {
        let cases = [
            ("Merge pull request #35 from dev", "merge"),
            ("Merge pull request #35 from dev", "DEV"),
            ("a1b2c3d4e5", "C3D4"),
            ("nothing here", "zzz"),
        ];
        for (haystack, needle) in cases {
            let needle_lower = needle.to_lowercase();
            assert_eq!(
                contains_ignore_case(haystack, &needle_lower),
                haystack.to_lowercase().contains(&needle_lower),
                "mismatch for {haystack:?} / {needle:?}"
            );
        }
    }
}
