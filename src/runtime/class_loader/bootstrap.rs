use dashmap::DashMap;
use once_cell::sync::OnceCell;
use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    fs::{self, File},
    io::{Read, Seek},
    mem,
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};
use zip::{ZipArchive, read::ZipFile};

use crate::runtime::gen_array_class;
use crate::{
    class::{self, parser},
    descriptor::FieldType,
    runtime,
    runtime::AttributeInfo,
    runtime::FieldResolve,
    runtime::class_loader::resolve_cp_class,
    runtime::class_loader::resolve_static_field,
    runtime::structs::ClinitStatus,
    runtime::{NativeResult, VmEnv},
};

#[derive(Debug)]
pub(in crate::runtime) struct BootstrapClassLoader {
    modules: Vec<Box<dyn ModuleLoader + Send + Sync + 'static>>,
    package_to_module: HashMap<String, usize>,
    // TODO: use Arc<String>
    class_registry: DashMap<String, Arc<OnceCell<Arc<runtime::Class>>>>,
}

pub trait ModuleLoader: Debug {
    fn packages(&self) -> Vec<Arc<str>>;
    fn name(&self) -> &str;
    // must end with .class
    fn get_class_file(&self, class_name: &str) -> OwnedOrRef<'_, class::Class>;
}

impl BootstrapClassLoader {
    pub(in crate::runtime) fn new() -> Self {
        Self {
            modules: vec![],
            package_to_module: HashMap::new(),
            class_registry: Default::default(),
        }
    }
    pub fn add_module(&mut self, module: Box<dyn ModuleLoader + Send + Sync + 'static>) {
        for package in module.packages() {
            self.package_to_module
                .insert(package.to_string(), self.modules.len());
        }
        self.modules.push(module);
    }

    pub(in crate::runtime) fn resolve_class(
        &self,
        env: &VmEnv,
        class_name: &str,
    ) -> NativeResult<Arc<runtime::Class>> {
        let class_cell = Arc::clone(
            self.class_registry
                .entry(class_name.to_string())
                .or_default()
                .value(),
        );

        let class = class_cell.get_or_try_init(|| self.define_class(env, class_name))?;

        self.run_clinit(env, class)?;

        Ok(Arc::clone(class))
    }

    pub(in crate::runtime) fn resolve_primitive_array_class(
        &self,
        env: &VmEnv,
        field_type: &FieldType,
    ) -> NativeResult<Arc<runtime::Class>> {
        let class_name_string = "[".to_string() + &field_type.to_descriptor();
        let class_name: Arc<str> = Arc::from(class_name_string.as_str());
        let class_cell = Arc::clone(
            self.class_registry
                .entry(class_name_string)
                .or_default()
                .value(),
        );
        let class = class_cell.get_or_try_init(|| self.define_array(env, class_name, None))?;

        // array has no clinit
        Ok(Arc::clone(class))
    }

    pub(in crate::runtime) fn resolve_object_array_class(
        &self,
        env: &VmEnv,
        ele_class: &Arc<runtime::Class>,
    ) -> NativeResult<Arc<runtime::Class>> {
        let class_name_string = "[".to_string() + &ele_class.class_name;
        let class_name: Arc<str> = Arc::from(class_name_string.as_str());
        let class_cell = Arc::clone(
            self.class_registry
                .entry(class_name_string)
                .or_default()
                .value(),
        );
        let class =
            class_cell.get_or_try_init(|| self.define_array(env, class_name, Some(ele_class)))?;

        // array has no clinit
        Ok(Arc::clone(class))
    }

    fn run_clinit(&self, env: &VmEnv, class: &Arc<runtime::Class>) -> NativeResult<()> {
        let clinit_status = class.clinit_call.lock();
        if clinit_status.get() == ClinitStatus::Init {
            return Ok(());
        }

        // TODO: record error
        clinit_status.set(ClinitStatus::Init);
        // execute clinit
        if let Some(clinit) = class.methods.iter().find(|m| m.name.to_str() == "<clinit>") {
            println!("clinit found for {:?}", clinit);
            let mut init_thread = env.get_thread().new_native_frame_group(None);
            init_thread.new_frame(
                Arc::clone(&class),
                &clinit.name.to_str(),
                &clinit.descriptor.parameters,
                0,
            );
            init_thread.execute()?;
        }
        println!("loaded {}", class.class_name);

        Ok(())
    }

    fn define_class(&self, env: &VmEnv, name: &str) -> NativeResult<Arc<runtime::Class>> {
        let package = if let Some((pkg, _)) = name.rsplit_once('/') {
            pkg
        } else {
            ""
        };
        // TODO: unwrap
        let module_id = self.package_to_module.get(package).unwrap();
        let module = &self.modules[*module_id];

        let class_file = &module.get_class_file(&(name.to_string() + ".class"));
        let mut class = runtime::parse_class(class_file);
        self.load_super_class(env, &mut class, class_file.super_class)?;
        self.load_interfaces(env, &mut class, &class_file.interfaces)?;

        Self::resolve_this_class_field_ref(&mut class);

        let class = Arc::new(class);
        Self::resolve_this_class_field_ref_static(&class);

        println!("defined {}", name);

        Ok(class)
    }

    fn define_array(
        &self,
        env: &VmEnv,
        class_name: Arc<str>,
        ele_class: Option<&Arc<runtime::Class>>,
    ) -> NativeResult<Arc<runtime::Class>> {
        let mut class = gen_array_class(class_name);

        class.super_class = Some(self.resolve_class(env, "java/lang/Object")?);
        class
            .interfaces
            .push(self.resolve_class(env, "java/lang/Cloneable")?);
        class
            .interfaces
            .push(self.resolve_class(env, "java/io/Serializable")?);
        class.array_element_type = ele_class.map(Arc::clone);

        Ok(Arc::new(class))
    }

    fn load_super_class(
        &self,
        env: &VmEnv,
        class: &mut runtime::Class,
        class_index: u16,
    ) -> NativeResult<()> {
        // java.lang.Object
        if class_index == 0 {
            return Ok(());
        }
        let super_class = resolve_cp_class(&class.constant_pool, class_index);
        let loaded = self.resolve_class(env, &super_class.name)?;
        super_class.set_class(&loaded);
        class.super_class.replace(Arc::clone(&loaded));
        Ok(())
    }
    fn load_interfaces(
        &self,
        env: &VmEnv,
        class: &mut runtime::Class,
        interfaces: &[u16],
    ) -> NativeResult<()> {
        for index in interfaces {
            let interface = resolve_cp_class(&class.constant_pool, *index);
            let loaded = self.resolve_class(env, &interface.name)?;
            interface.set_class(&loaded);
            class.interfaces.push(loaded);
        }
        Ok(())
    }
    fn resolve_this_class_field_ref(class: &mut runtime::Class) {
        // allocates field index for instance fields
        let mut instance_field_num = class
            .super_class
            .as_ref()
            .and_then(|s| s.instance_fields_info.last())
            .map(|f| {
                1 + if f.descriptor.0.is_long() {
                    f.index + 1
                } else {
                    f.index
                }
            })
            .unwrap_or(0);
        for field_info in class.instance_fields_info.iter_mut() {
            field_info.index = instance_field_num;
            if field_info.descriptor.0.is_long() {
                instance_field_num += 2;
            } else {
                instance_field_num += 1;
            }
        }

        // set up map, with fields in current class overwriting fields in super class
        let field_map: HashMap<_, _> = class
            .super_class
            .as_ref()
            .map(|s| &s.instance_fields_info)
            .into_iter()
            .flatten()
            .chain(&class.instance_fields_info)
            .map(|field| ((field.name.as_ref(), &field.descriptor), field.index))
            .collect();

        for cp_in_file in &class.constant_pool {
            let runtime::ConstantPoolInfo::Fieldref(field_ref) = cp_in_file else {
                continue;
            };

            if field_ref.class_name != class.class_name {
                // not in this class, to be resolved at runtime
                continue;
            }
            let name_and_type = &field_ref.name_and_type;

            let key = &(name_and_type.name.as_ref(), &name_and_type.descriptor);

            let index = field_map.get(key);
            if let Some(&index) = index {
                // inside this class
                field_ref
                    .resolve
                    .set(FieldResolve::InThisClass(index))
                    .expect("must be empty now");
            }
            // not found, must be a static field or an error
        }

        let total_field_len = class.instance_fields_info.len()
            + class
                .super_class
                .as_ref()
                .map(|s| s.instance_fields_info.len())
                .unwrap_or(0);
        let instance_fields = mem::replace(
            &mut class.instance_fields_info,
            Vec::with_capacity(total_field_len),
        );
        class.instance_fields_info.extend(
            class
                .super_class
                .as_ref()
                .map(|s| &s.instance_fields_info)
                .into_iter()
                .flatten()
                .cloned(),
        );
        class.instance_fields_info.extend(instance_fields);
    }

    fn resolve_this_class_field_ref_static(class: &Arc<runtime::Class>) {
        let field_map: HashMap<_, _> = class
            .static_fields_info
            .iter()
            .map(|field| ((field.name.as_ref(), &field.descriptor), field.index))
            .collect();

        // for filter
        let instance_field: HashSet<_> = class
            .instance_fields_info
            .iter()
            .map(|field| (field.name.as_ref(), &field.descriptor))
            .collect();

        for cp_in_file in &class.constant_pool {
            let runtime::ConstantPoolInfo::Fieldref(field_ref) = cp_in_file else {
                continue;
            };

            if field_ref.class_name != class.class_name {
                // not in this class, to be resolved at runtime
                continue;
            }
            let name_and_type = &field_ref.name_and_type;

            let key = &(name_and_type.name.as_ref(), &name_and_type.descriptor);
            if instance_field.contains(key) {
                // ignore instance field
                continue;
            }

            let index = field_map.get(key);
            if let Some(&index) = index {
                // inside this class
                field_ref
                    .resolve
                    .set(FieldResolve::InThisClass(index))
                    .expect("must be empty now");
            } else {
                let Some(resolve) = resolve_static_field(class, field_ref, true) else {
                    // instance fields from super class must be put into instance_field_info before this function
                    // TODO: exception ?
                    panic!("static field cannot be resolved");
                };
                field_ref.resolve.set(resolve).expect("must be empty now");
            }
        }
    }
}

#[derive(Debug)]
pub struct JModModule {
    name: String,
    class_files: HashMap<String, Vec<u8>>,
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
        let module_info = parser::class_file(&module_info).unwrap();
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
                AttributeInfo::ModulePackages(pkg) => {
                    Some(pkg.iter().map(|s| Arc::clone(s).to_str_arc()))
                }
                _ => None,
            })
            .flatten()
            .collect()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn get_class_file(&self, class_name: &str) -> OwnedOrRef<'_, class::Class> {
        let class_file = &self.class_files[class_name];
        let class_file = parser::class_file(class_file).expect(class_name);

        class_file.into()
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
        fn traverse(path: &Path, packages: &mut HashSet<String>, base_path: &Path) {
            if !path.is_dir() {
                return;
            }
            for entry in fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();
                if path.is_dir() {
                    traverse(&path, packages, base_path);
                } else if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "class" {
                            let dir_name = path
                                .parent()
                                .and_then(Path::to_str)
                                .unwrap_or("")
                                .to_string();
                            packages.insert(
                                dir_name
                                    .strip_prefix(base_path.to_str().expect("must be utf-8"))
                                    .expect("must have base path as prefix")
                                    .to_string(),
                            );
                        }
                    }
                }
            }
        }
        traverse(&self.base_path, &mut packages, &self.base_path);

        packages.into_iter().map(Into::into).collect()
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn get_class_file(&self, class_name: &str) -> OwnedOrRef<'_, class::Class> {
        // TODO: unwrap
        let class_file = fs::read(self.base_path.join(class_name)).unwrap();
        let class_file = parser::class_file(&class_file).unwrap();
        class_file.into()
    }
}

pub enum OwnedOrRef<'a, T> {
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
