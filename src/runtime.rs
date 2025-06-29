mod class_loader;
mod famous_classes;
mod heap;
mod inheritance;
mod interpreter;
mod native;
mod structs;

use crate::runtime::global::BOOTSTRAP_CLASS_LOADER;
pub use class_loader::*;
pub use interpreter::*;
use std::{
    path::{Path, PathBuf},
    sync::RwLock,
};
pub(crate) use structs::*;

use crate::runtime::{famous_classes::init_famous_classes, heap::Heap};
pub use native::*;

struct VmEnv<'a> {
    thread: &'a Thread<'a>,
    heap: &'static RwLock<Heap>,
}

impl<'a> VmEnv<'a> {
    pub fn new(thread: &'a Thread, heap: &'static RwLock<Heap>) -> Self {
        Self { thread, heap }
    }

    pub fn get_thread(&self) -> &Thread<'a> {
        self.thread
    }
}

fn init_bootstrap_class_loader(modules: Vec<Box<dyn ModuleLoader + Send + Sync + 'static>>) {
    let mut bootstrap_class_loader = BootstrapClassLoader::new();
    for module in modules {
        bootstrap_class_loader.add_module(module);
    }
    BOOTSTRAP_CLASS_LOADER.set(bootstrap_class_loader).unwrap()
}

pub fn genesis(java_home: impl AsRef<Path>, class_path: impl Into<PathBuf>) {
    init_bootstrap_class_loader(vec![
        Box::new(JModModule::new(java_home, "java.base")),
        Box::new(ClassPathModule::new("main", class_path)),
    ]);

    init_famous_classes();
}
