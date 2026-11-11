use iced::widget::Text;

use crate::message::{AppMsg, ImportMsg, LayoutMsg, Message, RequestMsg, SettingsMsg, SidebarMsg, SidebarPanel};
use crate::ui::components::tooltip::shortcut;
use crate::ui::icons;

/// One action the palette can run.
pub struct Command {
    pub label: &'static str,
    /// The key that goes with the platform modifier ("T" for Ctrl T).
    pub key: Option<&'static str>,
    pub icon: fn() -> Text<'static>,
    pub action: fn() -> Message,
    /// Listed before anything is typed.
    pub suggested: bool,
}

impl Command {
    pub fn shortcut(&self) -> Option<String> {
        self.key.map(shortcut)
    }
}

const fn command(
    label: &'static str,
    key: Option<&'static str>,
    icon: fn() -> Text<'static>,
    action: fn() -> Message,
) -> Command {
    Command {
        label,
        key,
        icon,
        action,
        suggested: false,
    }
}

const fn suggested(command: Command) -> Command {
    Command {
        suggested: true,
        ..command
    }
}

pub static COMMANDS: [Command; 22] = [
    suggested(command("New Tab", Some("T"), icons::plus, || Message::Request(RequestMsg::NewTab))),
    command("Send Request", Some("Enter"), icons::send, || Message::Request(RequestMsg::Send)),
    command("Save Request", Some("S"), icons::check, || Message::Request(RequestMsg::SaveRequest)),
    command("Close Tab", Some("W"), icons::close, || Message::Request(RequestMsg::CloseCurrentTab)),
    command("Focus URL Bar", Some("L"), icons::globe, || Message::App(AppMsg::FocusUrlBar)),
    command("Export Request as cURL", Some("E"), icons::curl, || Message::Request(RequestMsg::ExportCurl)),
    command("Import cURL / HTTPie Command", None, icons::import, || {
        Message::Request(RequestMsg::OpenImportDialog)
    }),
    suggested(command("Import cURL From Clipboard", Some("I"), icons::clipboard_paste, || {
        Message::Request(RequestMsg::ImportFromClipboard)
    })),
    command("Import Collection From File", None, icons::file_input, || {
        Message::Import(ImportMsg::ImportCollectionFile)
    }),
    command("Export All Collections", None, icons::export, || Message::Import(ImportMsg::ExportAllCollections)),
    command("New Collection", None, icons::folder_plus, || Message::Sidebar(SidebarMsg::NewCollection)),
    command("New Environment", None, icons::environments, || Message::Sidebar(SidebarMsg::EnvironmentCreated)),
    command("Show Collections", None, icons::collections, || {
        Message::Sidebar(SidebarMsg::PanelSelected(SidebarPanel::Collections))
    }),
    command("Show History", None, icons::history, || {
        Message::Sidebar(SidebarMsg::PanelSelected(SidebarPanel::History))
    }),
    command("Show Environments", None, icons::environments, || {
        Message::Sidebar(SidebarMsg::PanelSelected(SidebarPanel::Environments))
    }),
    command("Show Source Control", None, icons::git_branch, || {
        Message::Sidebar(SidebarMsg::PanelSelected(SidebarPanel::Git))
    }),
    command("Open Settings", None, icons::settings, || {
        Message::Settings(SettingsMsg::PageSelected(crate::ui::settings::Page::General))
    }),
    command("Toggle Sidebar", None, icons::arrow_left, || Message::Sidebar(SidebarMsg::ToggleCollapsed)),
    command("Toggle Split Direction", None, icons::braces, || {
        Message::Settings(SettingsMsg::LayoutDirectionToggled)
    }),
    command("Zoom In", Some("="), icons::plus, || Message::Layout(LayoutMsg::ZoomIn)),
    command("Zoom Out", Some("-"), icons::arrow_down, || Message::Layout(LayoutMsg::ZoomOut)),
    command("Reset Zoom", Some("0"), icons::refresh, || Message::Layout(LayoutMsg::ZoomReset)),
];

#[cfg(test)]
mod tests {
    use super::COMMANDS;

    /// Distinct, Title Case labels: duplicates would be two identical rows,
    /// and the list is matched by substring.
    #[test]
    fn command_labels_are_unique_and_title_case() {
        let mut labels: Vec<&str> = COMMANDS.iter().map(|c| c.label).collect();
        for label in &labels {
            for word in label.split([' ', '-']) {
                let Some(first) = word.chars().next() else {
                    continue;
                };
                let brand = word.chars().skip(1).any(char::is_uppercase);
                assert!(
                    !first.is_alphabetic() || first.is_uppercase() || brand || word == "as",
                    "{label:?} is not Title Case"
                );
            }
        }
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), COMMANDS.len(), "duplicate command label");
    }
}
