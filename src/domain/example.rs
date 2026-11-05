use serde::{Deserialize, Serialize};

use super::{collection::SavedRequest, response::HttpResponse};

/// A local, saved request/response pair belonging to one endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestExample {
    pub id: String,
    pub request_id: String,
    pub name: String,
    pub request: SavedRequest,
    pub response: HttpResponse,
}

impl RequestExample {
    pub fn new(request: SavedRequest, response: Option<HttpResponse>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            request_id: request.id.clone(),
            name: "New example".into(),
            request,
            response: response.unwrap_or_else(|| HttpResponse {
                status: 200,
                status_text: "OK".into(),
                ..Default::default()
            }),
        }
    }
}
