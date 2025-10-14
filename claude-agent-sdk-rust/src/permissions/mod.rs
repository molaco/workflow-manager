//! Permission system for tool access control
//!
//! This module provides the permission system for controlling which tools
//! Claude can use and with what parameters.

use std::sync::Arc;

use crate::error::Result;
use crate::types::{
    CanUseToolCallback, PermissionResult, PermissionResultAllow, ToolName, ToolPermissionContext,
};

/// Permission manager for tool access control
pub struct PermissionManager {
    /// Tool permission callback
    callback: Option<CanUseToolCallback>,
    /// Allowed tools (None = all allowed)
    allowed_tools: Option<Vec<ToolName>>,
    /// Disallowed tools
    disallowed_tools: Vec<ToolName>,
}

impl PermissionManager {
    /// Create a new permission manager
    pub fn new() -> Self {
        Self {
            callback: None,
            allowed_tools: None,
            disallowed_tools: Vec::new(),
        }
    }

    /// Set the permission callback
    pub fn set_callback(&mut self, callback: CanUseToolCallback) {
        self.callback = Some(callback);
    }

    /// Set allowed tools (None = all allowed)
    pub fn set_allowed_tools(&mut self, tools: Option<Vec<ToolName>>) {
        self.allowed_tools = tools;
    }

    /// Set disallowed tools
    pub fn set_disallowed_tools(&mut self, tools: Vec<ToolName>) {
        self.disallowed_tools = tools;
    }

    /// Check if a tool can be used
    ///
    /// # Arguments
    /// * `tool_name` - Name of the tool
    /// * `tool_input` - Tool input parameters
    /// * `context` - Permission context
    ///
    /// # Returns
    /// Permission result (Allow/Deny)
    pub async fn can_use_tool(
        &self,
        tool_name: ToolName,
        tool_input: serde_json::Value,
        context: ToolPermissionContext,
    ) -> Result<PermissionResult> {
        // Check disallowed list first
        if self.disallowed_tools.contains(&tool_name) {
            return Ok(PermissionResult::Deny(crate::types::PermissionResultDeny {
                message: format!("Tool {} is disallowed", tool_name.as_str()),
                interrupt: false,
            }));
        }

        // Check allowed list if set
        if let Some(ref allowed) = self.allowed_tools {
            if !allowed.contains(&tool_name) {
                return Ok(PermissionResult::Deny(
                    crate::types::PermissionResultDeny {
                        message: format!("Tool {} is not in allowed list", tool_name.as_str()),
                        interrupt: false,
                    },
                ));
            }
        }

        // Invoke callback if set
        if let Some(ref callback) = self.callback {
            callback(tool_name, tool_input, context).await
        } else {
            // If there's an allowed_tools list and we've passed the check, allow it
            // Otherwise, default to allow for backward compatibility
            // Note: For stricter security, consider changing this to deny-by-default
            Ok(PermissionResult::Allow(PermissionResultAllow {
                updated_input: None,
                updated_permissions: None,
            }))
        }
    }

    /// Create a permission callback from a closure
    pub fn callback<F, Fut>(f: F) -> CanUseToolCallback
    where
        F: Fn(ToolName, serde_json::Value, ToolPermissionContext) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<PermissionResult>> + Send + 'static,
    {
        Arc::new(move |tool_name, tool_input, context| Box::pin(f(tool_name, tool_input, context)))
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for permission manager
pub struct PermissionManagerBuilder {
    callback: Option<CanUseToolCallback>,
    allowed_tools: Option<Vec<ToolName>>,
    disallowed_tools: Vec<ToolName>,
}

impl PermissionManagerBuilder {
    /// Create a new permission manager builder
    pub fn new() -> Self {
        Self {
            callback: None,
            allowed_tools: None,
            disallowed_tools: Vec::new(),
        }
    }

    /// Set the permission callback
    pub fn callback(mut self, callback: CanUseToolCallback) -> Self {
        self.callback = Some(callback);
        self
    }

    /// Set allowed tools
    pub fn allowed_tools(mut self, tools: Vec<ToolName>) -> Self {
        self.allowed_tools = Some(tools);
        self
    }

    /// Set disallowed tools
    pub fn disallowed_tools(mut self, tools: Vec<ToolName>) -> Self {
        self.disallowed_tools = tools;
        self
    }

    /// Build the permission manager
    pub fn build(self) -> PermissionManager {
        PermissionManager {
            callback: self.callback,
            allowed_tools: self.allowed_tools,
            disallowed_tools: self.disallowed_tools,
        }
    }
}

impl Default for PermissionManagerBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_permission_manager_default_allow() {
        let manager = PermissionManager::new();

        let result = manager
            .can_use_tool(
                ToolName::new("test_tool"),
                serde_json::json!({}),
                ToolPermissionContext {
                    suggestions: vec![],
                },
            )
            .await
            .unwrap();

        match result {
            PermissionResult::Allow(_) => {}
            PermissionResult::Deny(_) => panic!("Expected allow"),
        }
    }

    #[tokio::test]
    async fn test_permission_manager_disallowed() {
        let mut manager = PermissionManager::new();
        manager.set_disallowed_tools(vec![ToolName::new("bad_tool")]);

        let result = manager
            .can_use_tool(
                ToolName::new("bad_tool"),
                serde_json::json!({}),
                ToolPermissionContext {
                    suggestions: vec![],
                },
            )
            .await
            .unwrap();

        match result {
            PermissionResult::Allow(_) => panic!("Expected deny"),
            PermissionResult::Deny(_) => {}
        }
    }

    #[tokio::test]
    async fn test_permission_manager_allowed_list() {
        let mut manager = PermissionManager::new();
        manager.set_allowed_tools(Some(vec![ToolName::new("good_tool")]));

        // Should allow good_tool
        let result = manager
            .can_use_tool(
                ToolName::new("good_tool"),
                serde_json::json!({}),
                ToolPermissionContext {
                    suggestions: vec![],
                },
            )
            .await
            .unwrap();

        match result {
            PermissionResult::Allow(_) => {}
            PermissionResult::Deny(_) => panic!("Expected allow"),
        }

        // Should deny other_tool
        let result = manager
            .can_use_tool(
                ToolName::new("other_tool"),
                serde_json::json!({}),
                ToolPermissionContext {
                    suggestions: vec![],
                },
            )
            .await
            .unwrap();

        match result {
            PermissionResult::Allow(_) => panic!("Expected deny"),
            PermissionResult::Deny(_) => {}
        }
    }
}
