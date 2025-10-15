//! History data structures

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// History storage: workflow_id -> field_name -> list of values
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkflowHistory {
    pub workflows: HashMap<String, HashMap<String, Vec<String>>>,
}
