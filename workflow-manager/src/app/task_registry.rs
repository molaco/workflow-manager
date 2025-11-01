//! Task registry for background task lifecycle management
//!
//! Background tasks (like log streamers) spawn tokio tasks that return JoinHandles.
//! JoinHandles cannot be sent through the command channel because they're not Clone/Debug.
//! This registry provides a separate mechanism for MCP tools to register tasks,
//! allowing the App to cancel them when tabs are closed.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use uuid::Uuid;

/// Global registry for background task handles
pub struct TaskRegistry {
    tasks: Arc<Mutex<HashMap<Uuid, Vec<JoinHandle<()>>>>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a background task for a workflow
    ///
    /// Call this after spawning a tokio::spawn task to track it for cleanup.
    pub async fn register(&self, workflow_id: Uuid, handle: JoinHandle<()>) {
        let mut tasks = self.tasks.lock().await;
        tasks.entry(workflow_id).or_insert_with(Vec::new).push(handle);
    }

    /// Cancel all tasks for a workflow
    ///
    /// Called when user closes a tab to clean up log streamers and other background tasks.
    pub async fn cancel_all(&self, workflow_id: &Uuid) {
        let mut tasks = self.tasks.lock().await;
        if let Some(handles) = tasks.remove(workflow_id) {
            for handle in handles {
                handle.abort();
            }
        }
    }

    /// Cancel all tasks (on app shutdown)
    pub async fn cancel_everything(&self) {
        let mut tasks = self.tasks.lock().await;
        for (_, handles) in tasks.drain() {
            for handle in handles {
                handle.abort();
            }
        }
    }
}

impl Clone for TaskRegistry {
    fn clone(&self) -> Self {
        Self {
            tasks: Arc::clone(&self.tasks),
        }
    }
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}
