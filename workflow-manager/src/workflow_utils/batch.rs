//! Batch execution utilities for parallel task processing

use anyhow::{anyhow, Result};
use futures::{stream::FuturesUnordered, Future, StreamExt};
use std::pin::Pin;
use std::sync::Arc;
use tokio::sync::Semaphore;

/// Context provided to each task in a batch
#[derive(Debug, Clone, Copy)]
pub struct TaskContext {
    /// Phase number (for logging)
    pub phase: usize,
    /// Task number (1-indexed for display)
    pub task_number: usize,
    /// Total number of tasks in this batch
    pub total_tasks: usize,
}

/// Execute items in parallel batches with concurrency control
///
/// # Arguments
/// - `phase`: Phase number for context
/// - `items`: Items to process
/// - `batch_size`: Maximum concurrent tasks
/// - `task_executor`: Function that processes each item, receives (item, context)
///
/// # Returns
/// Vector of results in order of completion (not input order)
///
/// # Error Handling
/// Fails fast - if any task fails, execution stops and error is returned
///
/// # Example
/// ```rust
/// let results = execute_batch(
///     2,  // phase
///     prompts,
///     3,  // batch_size
///     |prompt, ctx| async move {
///         // Process prompt
///         execute_research_task(prompt, ctx).await
///     }
/// ).await?;
/// ```
pub async fn execute_batch<T, F, Fut, R>(
    phase: usize,
    items: Vec<T>,
    batch_size: usize,
    task_executor: F,
) -> Result<Vec<R>>
where
    T: Send + 'static,
    R: Send + 'static,
    F: Fn(T, TaskContext) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<R>> + Send + 'static,
{
    let total = items.len();
    let sem = Arc::new(Semaphore::new(batch_size));
    let executor = Arc::new(task_executor);
    let mut tasks = FuturesUnordered::new();

    // Push all tasks to FuturesUnordered with semaphore control
    for (idx, item) in items.into_iter().enumerate() {
        let sem = sem.clone();
        let executor = executor.clone();
        let task_num = idx + 1;

        let ctx = TaskContext {
            phase,
            task_number: task_num,
            total_tasks: total,
        };

        tasks.push(async move {
            // Acquire permit (blocks if batch_size tasks are running)
            let _permit = sem
                .acquire()
                .await
                .map_err(|_| anyhow!("Semaphore closed"))?;

            // Execute task
            executor(item, ctx).await
        });
    }

    // Collect results as they complete (fail-fast on first error)
    let mut results = Vec::new();
    while let Some(result) = tasks.next().await {
        results.push(result?);
    }

    Ok(results)
}

/// Execute items in parallel batches (boxed future version for complex closures)
///
/// Use this variant when the compiler has trouble inferring types with complex closures.
pub async fn execute_batch_boxed<T, R, F>(
    phase: usize,
    items: Vec<T>,
    batch_size: usize,
    task_executor: F,
) -> Result<Vec<R>>
where
    T: Send + 'static,
    R: Send + 'static,
    F: Fn(T, TaskContext) -> Pin<Box<dyn Future<Output = Result<R>> + Send>> + Send + Sync + 'static,
{
    let total = items.len();
    let sem = Arc::new(Semaphore::new(batch_size));
    let executor = Arc::new(task_executor);
    let mut tasks = FuturesUnordered::new();

    for (idx, item) in items.into_iter().enumerate() {
        let sem = sem.clone();
        let executor = executor.clone();
        let task_num = idx + 1;

        let ctx = TaskContext {
            phase,
            task_number: task_num,
            total_tasks: total,
        };

        tasks.push(async move {
            let _permit = sem
                .acquire()
                .await
                .map_err(|_| anyhow!("Semaphore closed"))?;

            executor(item, ctx).await
        });
    }

    let mut results = Vec::new();
    while let Some(result) = tasks.next().await {
        results.push(result?);
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_batch() {
        let items = vec![1, 2, 3, 4, 5];

        let results = execute_batch(
            1,
            items,
            2, // batch_size
            |item, ctx| async move {
                assert!(ctx.task_number >= 1 && ctx.task_number <= 5);
                assert_eq!(ctx.total_tasks, 5);
                Ok(item * 2)
            }
        ).await.unwrap();

        assert_eq!(results.len(), 5);
        // Results may not be in order
        assert!(results.contains(&2));
        assert!(results.contains(&10));
    }

    #[tokio::test]
    async fn test_execute_batch_fail_fast() {
        let items = vec![1, 2, 3, 4, 5];

        let result = execute_batch(
            1,
            items,
            2,
            |item, _ctx| async move {
                if item == 3 {
                    Err(anyhow!("Failed at 3"))
                } else {
                    Ok(item * 2)
                }
            }
        ).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Failed at 3"));
    }
}
