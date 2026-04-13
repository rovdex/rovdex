use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub prompt: String,
}

impl Task {
    pub fn new(id: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            prompt: prompt.into(),
        }
    }
}
