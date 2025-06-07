use std::sync::{Mutex, OnceLock, RwLock};

use crate::runtime::class_loader::BootstrapClassLoader;
use crate::runtime::Heap;

pub(in crate::runtime) static HEAP: RwLock<Heap> = RwLock::new(Heap::new());

// TODO: use rwlock inside
pub(in crate::runtime) static BOOTSTRAP_CLASS_LOADER: OnceLock<Mutex<BootstrapClassLoader>> =
    OnceLock::new();
