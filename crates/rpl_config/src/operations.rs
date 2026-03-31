use std::collections::HashMap;

use crate::RplConfig;

/// The type used for operations throughout the config system.
/// Maps operation name -> list of fully-qualified function paths.
pub type Operations = HashMap<String, Vec<String>>;

pub(crate) fn load_operations(config: Option<&RplConfig>) -> Operations {
    config
        .and_then(|c| c.operations.as_ref())
        .cloned()
        .unwrap_or_default()
}
