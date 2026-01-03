use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct Task {
    pub task_id: String,
    pub seed: String,
    pub bits: i32,
    pub exp: i64,
    pub scope: String,
    pub ua_hash: String,
    pub ip_hash: String,
    pub used: bool,
    #[allow(dead_code)]
    pub created_at: Instant,
}

#[derive(Clone)]
pub struct TaskStore {
    inner: Arc<Mutex<HashMap<String, Task>>>,
}

impl TaskStore {
    pub fn new() -> Arc<Self> {
        let store = Arc::new(Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        });
        Self::spawn_cleanup(store.clone());
        store
    }

    pub fn set(&self, task: Task) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.insert(task.task_id.clone(), task);
        }
    }

    pub fn get(&self, task_id: &str) -> Option<Task> {
        self.inner.lock().ok().and_then(|guard| guard.get(task_id).cloned())
    }

    pub fn mark_used(&self, task_id: &str) -> bool {
        if let Ok(mut guard) = self.inner.lock() {
            if let Some(task) = guard.get_mut(task_id) {
                if !task.used {
                    task.used = true;
                    return true;
                }
            }
        }
        false
    }

    fn spawn_cleanup(store: Arc<Self>) {
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(30));
            store.cleanup();
        });
    }

    fn cleanup(&self) {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        if let Ok(mut guard) = self.inner.lock() {
            guard.retain(|_, task| !task.used && task.exp >= now);
        }
    }
}
