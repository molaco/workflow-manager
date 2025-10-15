//! Application view routing

/// Application view/route
#[derive(Debug, Clone, PartialEq)]
pub enum View {
    WorkflowList,
    WorkflowDetail(usize),  // workflow index
    WorkflowEdit(usize),    // workflow index
    WorkflowRunning(usize), // workflow index (will be deprecated)
    Tabs,                   // Main tabbed view
    Chat,                   // Chat interface with Claude
}
