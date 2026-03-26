use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(test)]
const LOCK_WAIT_TIMEOUT: Duration = Duration::from_millis(150);
#[cfg(not(test))]
const LOCK_WAIT_TIMEOUT: Duration = Duration::from_secs(5);

const LOCK_POLL_INTERVAL: Duration = Duration::from_millis(25);
const STALE_LOCK_THRESHOLD: Duration = Duration::from_secs(30);

pub(super) struct LockGuard {
    lock_path: PathBuf,
}

impl LockGuard {
    pub(super) fn acquire(lock_path: &Path) -> Result<Self, String> {
        let started = Instant::now();

        loop {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(lock_path)
            {
                Ok(mut file) => {
                    let metadata = LockMetadata::current()?;
                    file.write_all(metadata.render().as_bytes())
                        .map_err(|error| {
                            format!(
                                "failed to write index lock {}: {error}",
                                lock_path.display()
                            )
                        })?;
                    file.sync_all().map_err(|error| {
                        format!("failed to sync index lock {}: {error}", lock_path.display())
                    })?;
                    return Ok(Self {
                        lock_path: lock_path.to_path_buf(),
                    });
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    if is_stale_lock(lock_path)? {
                        match fs::remove_file(lock_path) {
                            Ok(()) => continue,
                            Err(remove_error)
                                if remove_error.kind() == std::io::ErrorKind::NotFound =>
                            {
                                continue;
                            }
                            Err(remove_error) => {
                                return Err(format!(
                                    "failed to remove stale index lock {}: {remove_error}",
                                    lock_path.display()
                                ));
                            }
                        }
                    }

                    if started.elapsed() >= LOCK_WAIT_TIMEOUT {
                        return Err(format!(
                            "timed out waiting for index lock {}",
                            lock_path.display()
                        ));
                    }
                    thread::sleep(LOCK_POLL_INTERVAL);
                }
                Err(error) => {
                    return Err(format!(
                        "failed to create index lock {}: {error}",
                        lock_path.display()
                    ));
                }
            }
        }
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

#[derive(Debug, Clone, Copy)]
struct LockMetadata {
    pid: u32,
    created_at_unix_secs: u64,
}

impl LockMetadata {
    fn current() -> Result<Self, String> {
        Ok(Self {
            pid: std::process::id(),
            created_at_unix_secs: current_unix_timestamp_secs()?,
        })
    }

    fn render(self) -> String {
        format!(
            "pid={}\ncreated_at_unix_secs={}\n",
            self.pid, self.created_at_unix_secs
        )
    }
}

pub(crate) fn current_unix_timestamp_secs() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .map_err(|error| format!("failed to read system time: {error}"))
}

fn is_stale_lock(lock_path: &Path) -> Result<bool, String> {
    if let Some(age) = lock_age_from_contents(lock_path)? {
        return Ok(age >= STALE_LOCK_THRESHOLD);
    }

    let metadata = match fs::metadata(lock_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(format!(
                "failed to read index lock metadata {}: {error}",
                lock_path.display()
            ));
        }
    };

    let modified_at = metadata.modified().map_err(|error| {
        format!(
            "failed to read index lock modified time {}: {error}",
            lock_path.display()
        )
    })?;
    let age = SystemTime::now()
        .duration_since(modified_at)
        .unwrap_or(Duration::ZERO);
    Ok(age >= STALE_LOCK_THRESHOLD)
}

fn lock_age_from_contents(lock_path: &Path) -> Result<Option<Duration>, String> {
    let content = match fs::read_to_string(lock_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to read index lock {}: {error}",
                lock_path.display()
            ));
        }
    };

    let Some(created_at_unix_secs) = parse_created_at_unix_secs(&content) else {
        return Ok(None);
    };
    let now = current_unix_timestamp_secs()?;
    Ok(Some(Duration::from_secs(
        now.saturating_sub(created_at_unix_secs),
    )))
}

fn parse_created_at_unix_secs(content: &str) -> Option<u64> {
    content
        .lines()
        .find_map(|line| line.strip_prefix("created_at_unix_secs="))
        .and_then(|value| value.parse::<u64>().ok())
}
