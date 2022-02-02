use std::sync::atomic::AtomicU64;

pub struct IdCounter {
    pub counter: AtomicU64
}

impl IdCounter {
    pub fn new() -> IdCounter {
        IdCounter {
            counter: AtomicU64::new(0)
        }
    }

    pub fn next(&self) -> u64 {
        self.counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    }
}
