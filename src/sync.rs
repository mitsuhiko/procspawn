use std::sync::{Mutex, MutexGuard, PoisonError};

pub(crate) trait MutexExt<T: ?Sized> {
    fn lock_unpoisoned(&self) -> MutexGuard<'_, T>;
}

impl<T: ?Sized> MutexExt<T> for Mutex<T> {
    fn lock_unpoisoned(&self) -> MutexGuard<'_, T> {
        self.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locks_after_holder_panics() {
        let mutex = Mutex::new(0);
        let result = std::panic::catch_unwind(|| {
            let mut guard = mutex.lock_unpoisoned();
            *guard = 42;
            panic!("poison the mutex");
        });
        assert!(result.is_err());
        assert_eq!(*mutex.lock_unpoisoned(), 42);
    }
}
