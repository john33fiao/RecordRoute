use crate::types::{TaskInfo, TaskStatus};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct JobManager {
    tasks: Arc<RwLock<HashMap<String, TaskInfo>>>,
}

impl JobManager {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_task(&self, task_type: String, file_uuid: String) -> String {
        let task_id = Uuid::new_v4().to_string();
        let task_info = TaskInfo {
            task_id: task_id.clone(),
            task_type,
            file_uuid,
            status: TaskStatus::Running,
            progress: 0,
            message: "Starting...".to_string(),
            started_at: Utc::now(),
        };

        self.tasks.write().await.insert(task_id.clone(), task_info);
        task_id
    }

    pub async fn update_progress(&self, task_id: &str, progress: u8, message: String) {
        if let Some(task) = self.tasks.write().await.get_mut(task_id) {
            task.progress = progress;
            task.message = message;
        }
    }

    pub async fn complete_task(&self, task_id: &str) {
        if let Some(task) = self.tasks.write().await.get_mut(task_id) {
            task.status = TaskStatus::Completed;
            task.progress = 100;
            task.message = "Completed".to_string();
        }
    }

    pub async fn fail_task(&self, task_id: &str, error: String) {
        if let Some(task) = self.tasks.write().await.get_mut(task_id) {
            task.status = TaskStatus::Failed;
            task.message = error;
        }
    }

    pub async fn get_tasks(&self) -> Vec<TaskInfo> {
        self.tasks.read().await.values().cloned().collect()
    }

    pub async fn cancel_task(&self, task_id: &str) -> bool {
        if let Some(task) = self.tasks.write().await.get_mut(task_id) {
            task.status = TaskStatus::Cancelled;
            task.message = "Cancelled by user".to_string();
            true
        } else {
            false
        }
    }
}
