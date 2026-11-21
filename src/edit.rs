//! Edits to a source, so a model need not repeat what stays.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Edit {
    /// Exact text occurring once in the source.
    pub find: String,
    pub replace: String,
}

/// The edits applied in order; `None` if an edit's text does not occur exactly once.
#[must_use]
pub fn apply(source: &str, edits: &[Edit]) -> Option<String> {
    let mut source = source.to_owned();
    for edit in edits {
        if source.matches(&edit.find).count() != 1 {
            return None;
        }
        source = source.replacen(&edit.find, &edit.replace, 1);
    }
    Some(source)
}
