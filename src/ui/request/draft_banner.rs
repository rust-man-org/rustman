//! Local draft banner above the request editor (workstream 01, C9).
//!
//! One banner per draft state, following the approved mockup
//! `agent/plan/mockup/pages/rustman-requester-draft.html`: local draft,
//! main changed, field conflicts, in review, deleted endpoint. Shows the
//! persisted draft; unsaved tab edits are captured on Ctrl+S or sync.
//!
//! Composition only: all visuals come from `ui::components`.

use iced::{Element, Length, widget::row};

use crate::{
    app::AppState,
    domain::draft::{DraftField, DraftState, RequestDraft, Side},
    message::{Message, draft::DraftMsg},
    ui::{
        components::{
            Tone,
            atoms::{Button, Mark, text},
            molecules::{Banner, DataTable},
        },
        icons,
    },
};

const NEEDS_WRITE: &str = "Needs write access to the collection repository";
const RESOLVE_FIRST: &str = "Resolve every conflict first";
const NOT_YET: &str = "Not available yet: review publishing (branch + PR) is not set up";

pub fn view(state: &AppState) -> Option<Element<'_, Message>> {
    let tab = state.tabs.active_tab();
    let (_, id) = tab.saved_as.as_ref()?;
    let draft = state.drafts.get(id)?;
    let can_write = state.auth.can_write();
    let diff_open = state.drafts.diff_open.as_deref() == Some(id.as_str());
    let changed = draft.changed_fields();

    let msg = |m: DraftMsg| Message::Draft(m);
    let discard = || Button::new("Discard").on_press(msg(DraftMsg::Discard(id.clone())));
    let view_diff = || {
        Button::new(if diff_open { "Hide diff" } else { "View diff" })
            .ghost()
            .on_press(msg(DraftMsg::ToggleDiff(id.clone())))
    };
    let save = || {
        let tip = if !can_write {
            NEEDS_WRITE
        } else if !draft.can_publish() {
            RESOLVE_FIRST
        } else {
            NOT_YET
        };
        // Enabled by 08 (Save for review, C12).
        Button::<Message>::new("Save for review").primary().tooltip(tip)
    };

    let banner = match draft.state() {
        DraftState::Orphaned => Banner::new(
            Tone::Subtle,
            Mark::Icon(icons::close()),
            "This endpoint was deleted",
        )
        .detail("Your draft is kept on this machine only. Restore it as a new endpoint or discard it.")
        .action(discard())
        .action({
            let b = Button::new("Restore as new").primary();
            if can_write {
                b.on_press(msg(DraftMsg::RestoreAsNew(id.clone())))
            } else {
                b.tooltip(NEEDS_WRITE)
            }
        }),
        DraftState::Conflict(fields) => Banner::new(
            Tone::Danger,
            Mark::Dot,
            format!(
                "{} field{} changed on main and in your draft",
                fields.len(),
                plural(fields.len())
            ),
        )
        .detail("Pick a side for each field. Save for review is blocked until all are resolved.")
        .action(Button::new("Discard draft").on_press(msg(DraftMsg::Discard(id.clone()))))
        .action(save())
        .body(conflict_table(draft, &fields)),
        DraftState::InReview(review) => Banner::new(
            Tone::Neutral,
            Mark::Icon(icons::git_branch()),
            match review.pr_number {
                Some(n) => format!("In review · PR #{n}"),
                None => "In review · branch pushed".to_owned(),
            },
        )
        .detail(format!(
            "{} → main. Saving again updates the same review.",
            review.branch
        ))
        .action(view_diff())
        .action(discard()),
        DraftState::UpstreamChanged => Banner::new(Tone::Info, Mark::Dot, "Main changed since your draft")
            .detail("Their changes were merged in; your edits are kept.")
            .action(view_diff())
            .action(discard())
            .action(save()),
        DraftState::Clean => Banner::new(Tone::Accent, Mark::Dot, "Local draft: only you see this")
            .detail(format!(
                "{} field{} differ{} from main: {}. Ctrl+S saves locally.",
                changed.len(),
                plural(changed.len()),
                if changed.len() == 1 { "s" } else { "" },
                field_list(&changed)
            ))
            .action(view_diff())
            .action(discard())
            .action(save()),
    };

    let banner = if tab.modified {
        banner.body(text::muted(
            "Unsaved edits in this tab are not in the draft yet (Ctrl+S).",
        ))
    } else {
        banner
    };
    let show_diff =
        diff_open && !changed.is_empty() && !matches!(draft.state(), DraftState::Conflict(_));
    let banner = if show_diff {
        banner.body(diff_table(draft, &changed))
    } else {
        banner
    };
    Some(banner.into())
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}

fn field_list(fields: &[DraftField]) -> String {
    const SHOWN: usize = 4;
    let mut names: Vec<String> = fields
        .iter()
        .take(SHOWN)
        .map(|f| f.label().to_owned())
        .collect();
    let more = fields.len().saturating_sub(SHOWN);
    if more > 0 {
        names.push(format!("+{more} more"));
    }
    names.join(", ")
}

fn diff_table<'a>(draft: &RequestDraft, fields: &[DraftField]) -> Element<'a, Message> {
    fields
        .iter()
        .fold(
            DataTable::new([
                ("Field", Length::Fixed(130.0)),
                ("main", Length::FillPortion(1)),
                ("Your draft", Length::FillPortion(1)),
            ]),
            |t, f| {
                t.row(vec![
                    text::body(f.label()),
                    text::code(f.preview(&draft.base), Tone::Neutral),
                    text::code(f.preview(&draft.data), Tone::Accent),
                ])
            },
        )
        .into()
}

fn conflict_table<'a>(draft: &RequestDraft, fields: &[DraftField]) -> Element<'a, Message> {
    fields
        .iter()
        .fold(
            DataTable::new([
                ("Field", Length::Fixed(130.0)),
                ("Yours", Length::FillPortion(1)),
                ("main", Length::FillPortion(1)),
                ("", Length::Fixed(170.0)),
            ]),
            |t, f| {
                let pick = |label: &'static str, side: Side| {
                    Button::new(label).on_press(Message::Draft(DraftMsg::ResolveField(
                        draft.request_id.clone(),
                        *f,
                        side,
                    )))
                };
                t.row(vec![
                    text::body(f.label()),
                    text::code(f.preview(&draft.data), Tone::Accent),
                    text::code(f.preview(&draft.base), Tone::Info),
                    row![pick("Keep mine", Side::Mine), pick("Use main", Side::Upstream)]
                        .spacing(6)
                        .into(),
                ])
            },
        )
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_list_caps_at_four_names() {
        let all = DraftField::ALL;
        assert_eq!(field_list(&all[..2]), "Method, URL");
        let long = field_list(&all[..6]);
        assert!(long.ends_with("+2 more"), "{long}");
    }
}
