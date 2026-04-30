use std::sync::{Mutex, MutexGuard, OnceLock};

pub fn filesystem_test_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .expect("filesystem test lock should not be poisoned")
}
