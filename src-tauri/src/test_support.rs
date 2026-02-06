#[cfg(test)]
use std::sync::{Mutex, MutexGuard, OnceLock};

#[cfg(test)]
pub fn lock_env() -> MutexGuard<'static, ()> {
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    ENV_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("lock test env mutex")
}
