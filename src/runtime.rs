mod class_loader;
mod interpreter;
mod structs;

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use crate::runtime::global::BOOTSTRAP_CLASS_LOADER;
pub use class_loader::*;
pub use interpreter::*;
pub(crate) use structs::*;

pub fn init_bootstrap_class_loader(rt_path: impl Into<PathBuf>, modules: &[&str]) {
    BOOTSTRAP_CLASS_LOADER
        .set(BootstrapClassLoader::new(rt_path, modules))
        .unwrap()
}
