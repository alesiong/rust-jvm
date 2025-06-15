mod class_loader;
mod interpreter;
mod native;
mod structs;
mod inheritance;

use crate::runtime::global::BOOTSTRAP_CLASS_LOADER;
pub use class_loader::*;
pub use interpreter::*;
pub(crate) use structs::*;

pub use native::*;

struct VmEnv<'a, 'b> {
    thread: &'a Thread<'a>,
    // TODO: make cur frame into thread
    cur_frame: Option<&'b Frame>,
}

impl<'a, 'b> VmEnv<'a, 'b> {
    pub fn new(thread: &'a Thread) -> Self {
        Self {
            thread,
            cur_frame: None,
        }
    }

    pub fn get_thread(&self) -> &Thread {
        self.thread
    }

}

pub fn init_bootstrap_class_loader(modules: Vec<Box<dyn ModuleLoader + Send + Sync + 'static>>) {
    let mut bootstrap_class_loader = BootstrapClassLoader::new();
    for module in modules {
        bootstrap_class_loader.add_module(module);
    }
    BOOTSTRAP_CLASS_LOADER.set(bootstrap_class_loader).unwrap()
}
