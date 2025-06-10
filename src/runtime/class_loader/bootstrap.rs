use dashmap::{DashMap, Entry};
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    fs::{self, File},
    io::{Read, Seek},
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};
use zip::{ZipArchive, read::ZipFile};

use crate::{
    class::{self, parser},
    runtime,
    runtime::AttributeInfo,
};

#[derive(Debug)]
pub(in crate::runtime) struct BootstrapClassLoader {
    modules: Vec<Box<dyn ModuleLoader + Send + Sync + 'static>>,
    package_to_module: HashMap<String, usize>,
    class_registry: DashMap<String, Arc<runtime::Class>>,
}

pub trait ModuleLoader: Debug {
    fn packages(&self) -> Vec<Arc<str>>;
    fn name(&self) -> &str;
    // must end with .class
    fn get_class_file(&self, class_name: &str) -> OwnedOrRef<class::Class>;
}

impl BootstrapClassLoader {
    pub(in crate::runtime) fn new() -> Self {
        Self {
            modules: vec![],
            package_to_module: HashMap::new(),
            class_registry: DashMap::new(),
        }
    }
    pub fn add_module(&mut self, module: Box<dyn ModuleLoader + Send + Sync + 'static>) {
        for package in module.packages() {
            self.package_to_module
                .insert(package.to_string(), self.modules.len());
        }
        self.modules.push(module);
    }

    pub(in crate::runtime) fn resolve_class(&self, class_name: &str) -> Arc<runtime::Class> {
        if let Some(class) = self.class_registry.get(class_name) {
            return Arc::clone(&class);
        }
        let mut need_init = false;
        let class = match self.class_registry.entry(class_name.to_string()) {
            Entry::Occupied(entry) => Arc::clone(entry.get()),
            Entry::Vacant(entry) => {
                let class = Arc::new(load_class(
                    &self.package_to_module,
                    &self.modules,
                    class_name,
                ));
                println!("loaded {}", class_name);
                entry.insert(Arc::clone(&class));
                need_init = true;
                class
            }
        };
        if need_init {
            // execute clinit
            if let Some(clinit) = class.methods.iter().find(|m| m.name.to_str() == "<clinit>") {
                println!("clinit found for {:?}", clinit);
                let mut init_thread = runtime::Thread::new(1024);
                init_thread.new_frame(
                    Arc::clone(&class),
                    &clinit.name.to_str(),
                    &clinit.descriptor.parameters,
                    0,
                );
                init_thread.execute();
            }
        }
        class
    }
}

fn load_class(
    package_to_module: &HashMap<String, usize>,
    modules: &[Box<dyn ModuleLoader + Send + Sync + 'static>],
    name: &str,
) -> runtime::Class {
    let package = if let Some((pkg, _)) = name.rsplit_once('/') {
        pkg
    } else {
        ""
    };
    // TODO: unwrap
    let module_id = package_to_module.get(package).unwrap();
    let module = &modules[*module_id];

    runtime::parse_class(&module.get_class_file(&(name.to_string() + ".class")))
}

#[derive(Debug)]
pub struct JModModule {
    name: String,
    class_files: HashMap<String, class::Class>,
    module_info: runtime::Class,
}

impl JModModule {
    pub fn new(java_home: impl AsRef<Path>, module_name: impl Into<String>) -> JModModule {
        let module_name = module_name.into();
        let jmod_path = java_home
            .as_ref()
            .join("jmods")
            .join(module_name.to_string() + ".jmod");
        // TODO: unwrap
        let mod_file = File::open(jmod_path).unwrap();
        let mut archive = ZipArchive::new(mod_file).unwrap();

        // load module info
        let mut module_info_file = archive.by_name("classes/module-info.class").unwrap();
        let module_info = Self::get_class_bytes(&mut module_info_file);
        drop(module_info_file);
        let (_, module_info) = parser::class_file(&module_info).unwrap();
        let module_info = runtime::parse_class(&module_info);

        // collect class files
        let mut class_files = HashMap::new();
        for i in 0..archive.len() {
            let mut file = archive.by_index(i).unwrap();
            if !file.name().starts_with("classes/") {
                continue;
            }
            if !file.name().ends_with(".class") {
                continue;
            }
            if file.name() == "classes/module-info.class" {
                continue;
            }
            let name = file.name().replace("classes/", "");
            let class_file = Self::get_class_bytes(&mut file);
            let (_, class_file) = parser::class_file(&class_file).expect(&name);
            class_files.insert(name, class_file);
        }

        JModModule {
            name: module_name,
            class_files,
            module_info,
        }
    }

    fn get_class_bytes<R: Read + Seek>(class_file: &mut ZipFile<R>) -> Vec<u8> {
        // TODO: unwrap
        let mut content = Vec::with_capacity(class_file.size() as usize);
        class_file.read_to_end(&mut content).unwrap();
        content
    }
}

impl ModuleLoader for JModModule {
    fn packages(&self) -> Vec<Arc<str>> {
        self.module_info
            .attributes
            .iter()
            .filter_map(|attr| match attr {
                AttributeInfo::ModulePackages(pkg) => Some(pkg.iter().map(|s| s.to_str().into())),
                _ => None,
            })
            .flatten()
            .collect()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn get_class_file(&self, class_name: &str) -> OwnedOrRef<class::Class> {
        self.class_files.get(class_name).unwrap().into()
    }
}

#[derive(Debug)]
pub struct ClassPathModule {
    name: String,
    base_path: PathBuf,
}

impl ClassPathModule {
    pub fn new(name: impl Into<String>, base_path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            base_path: base_path.into(),
        }
    }
}

impl ModuleLoader for ClassPathModule {
    fn packages(&self) -> Vec<Arc<str>> {
        // TODO: unwrap
        let mut packages = HashSet::new();
        fn traverse(path: &Path, packages: &mut HashSet<String>) {
            if !path.is_dir() {
                return;
            }
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_dir() {
                    traverse(&path, packages);
                } else if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "class" {
                            let dir_name = path
                                .parent()
                                .and_then(Path::to_str)
                                .unwrap_or("")
                                .to_string();
                            packages.insert(dir_name);
                        }
                    }
                }
            }
        }
        traverse(&self.base_path, &mut packages);

        packages.into_iter().map(Into::into).collect()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn get_class_file(&self, class_name: &str) -> OwnedOrRef<class::Class> {
        // TODO: unwrap
        let class_file = fs::read(self.base_path.join(class_name)).unwrap();
        let (_, class_file) = parser::class_file(&class_file).unwrap();
        class_file.into()
    }
}

enum OwnedOrRef<'a, T> {
    Owned(T),
    Ref(&'a T),
}

impl<T> Deref for OwnedOrRef<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            OwnedOrRef::Owned(o) => o,
            OwnedOrRef::Ref(r) => r,
        }
    }
}

impl<T> From<T> for OwnedOrRef<'_, T> {
    fn from(o: T) -> Self {
        OwnedOrRef::Owned(o)
    }
}

impl<'a, T> From<&'a T> for OwnedOrRef<'a, T> {
    fn from(r: &'a T) -> Self {
        OwnedOrRef::Ref(r)
    }
}
