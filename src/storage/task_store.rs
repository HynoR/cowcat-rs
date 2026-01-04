use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use time::OffsetDateTime;

const TASK_CLEANUP_INTERVAL: u64 = 300;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TaskId(pub String);

impl TaskId {
    pub fn short_id(&self) -> &str {
        debug_assert!(self.0.is_empty() || self.0.is_ascii());
        &self.0[..6.min(self.0.len())]
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for TaskId {
    fn from(s: String) -> Self {
        TaskId(s)
    }
}

#[derive(Debug, Clone)]
pub struct Seed(pub String);

impl fmt::Display for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Seed {
    fn from(s: String) -> Self {
        Seed(s)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct UaHash(pub String);

impl fmt::Display for UaHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for UaHash {
    fn from(s: String) -> Self {
        UaHash(s)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IpHash(pub String);

impl fmt::Display for IpHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for IpHash {
    fn from(s: String) -> Self {
        IpHash(s)
    }
}

#[derive(Debug, Clone)]
pub struct Scope(pub String);

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Scope {
    fn from(s: String) -> Self {
        Scope(s)
    }
}

#[derive(Debug)]
pub enum ConsumeError {
    NotFound,
    Expired,
    ValidationFailed(&'static str),
}

#[derive(Debug, Clone)]
pub struct Task {
    pub task_id: TaskId,
    pub seed: Seed,
    pub bits: u32,
    pub exp: i64,
    pub scope: Scope,
    pub ua_hash: UaHash,
    pub ip_hash: IpHash,
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

    /// 插入新任务
    pub fn insert(&self, task: Task) {
        if let Ok(mut guard) = self.inner.lock() {
            guard.insert(task.task_id.0.clone(), task);
        }
    }

    /// 消费任务：取出并移除，然后验证
    /// 无论验证成功与否，任务都被消耗（防重放）
    pub fn consume_if<F>(&self, task_id: &str, validate: F) -> Result<Task, ConsumeError>
    where
        F: FnOnce(&Task) -> Result<(), ConsumeError>,
    {
        let mut guard = self.inner.lock().map_err(|_| ConsumeError::NotFound)?;
        
        // 先移除任务（任务被消耗）
        let task = guard.remove(task_id).ok_or(ConsumeError::NotFound)?;
        
        // 检查过期
        let now = OffsetDateTime::now_utc().unix_timestamp();
        if task.exp < now {
            return Err(ConsumeError::Expired);
        }
        
        // 调用验证闭包
        validate(&task)?;
        
        Ok(task)
    }

    fn spawn_cleanup(store: Arc<Self>) {
        thread::spawn(move || loop {
            thread::sleep(Duration::from_secs(TASK_CLEANUP_INTERVAL));
            store.cleanup();
        });
    }

    fn cleanup(&self) {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        tracing::debug!("cleaning up tasks");
        if let Ok(mut guard) = self.inner.lock() {
            guard.retain(|_, task| task.exp >= now);
        }
    }
}
