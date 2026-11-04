#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderSide {
    Request,
    Response,
    Params,
}

#[derive(Debug, Clone)]
pub enum ExampleMsg {
    Create(String),
    SaveResponse,
    Open(String),
    Toggle(String),
    Name(String),
    Method(String),
    Url(String),
    Status(String),
    StatusText(String),
    RequestBody(iced_code_editor::Message),
    ResponseBody(iced_code_editor::Message),
    BodyFormat(String),
    FormatBody(HeaderSide),
    HeaderKey(HeaderSide, usize, String),
    HeaderValue(HeaderSide, usize, String),
    HeaderAdd(HeaderSide),
    HeaderRemove(HeaderSide, usize),
    ShowHeaders(HeaderSide, bool),
    Save,
    Revert,
    Close,
    Discard,
    Delete,
}
