//! Task execution utilities with automatic logging

use crate::workflow_utils::batch::TaskContext;
use anyhow::Result;
use std::future::Future;
use workflow_manager_sdk::{log_task_complete, log_task_failed, log_task_start};

/// Execute a single task with automatic logging
///
/// Wraps task execution with:
/// - `log_task_start` before execution
/// - `log_task_complete` on success
/// - `log_task_failed` on error
///
/// # Arguments
/// - `task_id`: Unique identifier for this task
/// - `description`: Human-readable description
/// - `ctx`: Task context (phase, task number, total)
/// - `executor`: Async function that performs the work and returns (result, summary_message)
///
/// # Example
/// ```rust
/// let result = execute_task(
///     "research_1",
///     "Research security patterns",
///     ctx,
///     || async {
///         let data = do_research().await?;
///         Ok((data, "Research completed successfully".to_string()))
///     }
/// ).await?;
/// ```
pub async fn execute_task<F, Fut, R>(
    task_id: impl Into<String>,
    description: impl Into<String>,
    ctx: TaskContext,
    executor: F,
) -> Result<R>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<(R, String)>>,
{
    let task_id = task_id.into();
    let description = description.into();

    // Log task start
    log_task_start!(ctx.phase, &task_id, &description, ctx.total_tasks);

    // Execute task
    match executor().await {
        Ok((result, summary)) => {
            log_task_complete!(&task_id, summary);
            Ok(result)
        }
        Err(e) => {
            log_task_failed!(&task_id, e.to_string());
            Err(e)
        }
    }
}

/// Execute a single task without automatic completion logging
///
/// Use this when you want manual control over task completion messages
/// or when the task involves multiple steps that need individual logging.
///
/// You must manually call `log_task_complete!` or `log_task_failed!`.
pub async fn execute_task_manual<F, Fut, R>(
    task_id: impl Into<String>,
    description: impl Into<String>,
    ctx: TaskContext,
    executor: F,
) -> Result<R>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<R>>,
{
    let task_id = task_id.into();
    let description = description.into();

    log_task_start!(ctx.phase, &task_id, &description, ctx.total_tasks);

    executor().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_task_success() {
        let ctx = TaskContext {
            phase: 1,
            task_number: 1,
            total_tasks: 1,
        };

        let result = execute_task("test_task", "Test task", ctx, || async {
            Ok((42, "Success".to_string()))
        })
        .await
        .unwrap();

        assert_eq!(result, 42);
    }

    #[tokio::test]
    async fn test_execute_task_failure() {
        let ctx = TaskContext {
            phase: 1,
            task_number: 1,
            total_tasks: 1,
        };

        let result = execute_task("test_task", "Test task", ctx, || async {
            Err::<(i32, String), _>(anyhow::anyhow!("Task failed"))
        })
        .await;

        assert!(result.is_err());
    }
}
