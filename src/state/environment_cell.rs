//! One bounded editor for a key/value cell in the environment workspace.
pub struct EnvironmentCellEditor {
    pub token: String,
    pub environment_id: String,
    pub row: usize,
    pub value: bool,
    pub editor: super::tabs::FormValueEditor,
}
