use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

pub type CacheMutex = Mutex<HashMap<Vec<u8>, Vec<u8>>>;
pub type Cache = Arc<CacheMutex>;
pub type LockedCache<'a> = MutexGuard<'a, HashMap<Vec<u8>, Vec<u8>>>;

pub fn new_cache() -> Cache {
    Arc::new(Mutex::new(HashMap::new()))
}
