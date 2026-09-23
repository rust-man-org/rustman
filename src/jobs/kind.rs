
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobKind {
    Request,
    Parse,
    Format,
    /// Parsing an HTML body into its preview document. Kept separate from
    /// `Parse` so it cannot cancel — or be cancelled by — the raw-source and
    /// JSON-pretty-printing job running against the same response.
    HtmlPreview,
}

impl JobKind {
    pub(crate) const COUNT: usize = 4;

    pub(crate) const fn index(self) -> usize {
        match self {
            JobKind::Request => 0,
            JobKind::Parse => 1,
            JobKind::Format => 2,
            JobKind::HtmlPreview => 3,
        }
    }
}
