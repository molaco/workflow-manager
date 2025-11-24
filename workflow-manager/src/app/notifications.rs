//! Notification system for user-visible feedback
//!
//! This module provides a notification manager that displays
//! user-visible messages for operations, errors, and status updates.

use std::time::Instant;
use super::commands::NotificationLevel;

#[derive(Debug, Clone)]
pub struct Notification {
    pub id: usize,
    pub timestamp: Instant,
    pub level: NotificationLevel,
    pub title: String,
    pub message: String,
    pub dismissible: bool,
    pub auto_dismiss_after: Option<std::time::Duration>,
}

pub struct NotificationManager {
    notifications: Vec<Notification>,
    next_id: usize,
    max_notifications: usize,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self {
            notifications: Vec::new(),
            next_id: 0,
            max_notifications: 50,
        }
    }

    /// Add an error notification
    pub fn error(&mut self, title: impl Into<String>, message: impl Into<String>) -> usize {
        self.push(NotificationLevel::Error, title.into(), message.into())
    }

    /// Add a success notification
    pub fn success(&mut self, title: impl Into<String>, message: impl Into<String>) -> usize {
        self.push(NotificationLevel::Success, title.into(), message.into())
    }

    /// Add a warning notification
    pub fn warning(&mut self, title: impl Into<String>, message: impl Into<String>) -> usize {
        self.push(NotificationLevel::Warning, title.into(), message.into())
    }

    /// Add an info notification
    pub fn info(&mut self, title: impl Into<String>, message: impl Into<String>) -> usize {
        self.push(NotificationLevel::Info, title.into(), message.into())
    }

    /// Internal method to add notification
    pub fn push(&mut self, level: NotificationLevel, title: String, message: String) -> usize {
        let id = self.next_id;
        self.next_id += 1;

        self.notifications.push(Notification {
            id,
            timestamp: Instant::now(),
            level,
            title,
            message,
            dismissible: true,
            auto_dismiss_after: Some(std::time::Duration::from_secs(5)),
        });

        // Keep only recent notifications
        if self.notifications.len() > self.max_notifications {
            self.notifications.remove(0);
        }

        id
    }

    /// Dismiss a notification by ID
    pub fn dismiss(&mut self, id: usize) {
        self.notifications.retain(|n| n.id != id);
    }

    /// Get active (non-expired) notifications
    pub fn get_active(&self) -> Vec<&Notification> {
        let now = Instant::now();
        self.notifications
            .iter()
            .filter(|n| match n.auto_dismiss_after {
                Some(duration) => now.duration_since(n.timestamp) < duration,
                None => true,
            })
            .collect()
    }

    /// Remove expired notifications
    pub fn cleanup_expired(&mut self) {
        let now = Instant::now();
        self.notifications.retain(|n| match n.auto_dismiss_after {
            Some(duration) => now.duration_since(n.timestamp) < duration,
            None => true,
        });
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}
