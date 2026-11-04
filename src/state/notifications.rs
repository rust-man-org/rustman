#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastPlacement {
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub placement: ToastPlacement,
    pub id: String,
    pub title: String,
    pub detail: Option<String>,
}
