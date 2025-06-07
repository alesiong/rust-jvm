use std::collections::HashMap;
use std::convert::identity;
use std::sync::{Arc, RwLock};

use nom::bytes::complete::take;
use nom::multi::count;
use nom::number::complete::{be_u16, be_u32, u8};
use nom::{error_position, IResult, Parser, Slice};

use crate::runtime::{global, Module, ModuleExport};
use crate::{
    class,
    consts::FieldAccessFlag,
    descriptor::{
        self, parse_field_descriptor, parse_method_descriptor, parse_return_type_descriptor,
        FieldDescriptor, FieldType, MethodDescriptor,
    },
    runtime::{
        self, Annotation, Const, CpClassInfo, CpNameAndTypeInfo, ElementValuePair, FieldIndex,
        FieldInfo,
    },
};

use super::{ElementValue, LocalVariable};

mod bootstrap;
pub(super) use bootstrap::BootstrapClassLoader;

pub fn parse_class(class_file: &class::Class) -> runtime::Class {
    let mut constant_pool = parse_constant_pool(&class_file.constant_pool);

    let super_class = load_super_class(&class_file.constant_pool, class_file.super_class);
    let interfaces = load_interfaces(&class_file.constant_pool, &class_file.interfaces);
    let fields: Vec<_> = class_file
        .fields
        .iter()
        .map(|f| parse_field(&class_file.constant_pool, f))
        .collect();
    let methods = class_file
        .methods
        .iter()
        .map(|m| parse_method(&class_file.constant_pool, m))
        .collect();
    let attributes = class_file
        .attributes
        .iter()
        .map(convert_attribute(&class_file.constant_pool))
        .collect();

    let class_name = resolve_cp_class(&class_file.constant_pool, class_file.this_class).name;
    let (field_var_size, static_field_size) =
        resolve_this_class_field_ref(&fields, &mut constant_pool, &class_name);

    runtime::Class {
        access_flags: class_file.access_flags,
        class_name: Arc::clone(&class_name),
        super_class,
        interfaces,
        fields,
        methods,
        attributes,
        constant_pool,
        field_var_size,
        static_fields: RwLock::new(Vec::with_capacity(static_field_size)),
    }
}

fn load_super_class(
    cp: &[class::ConstantPoolInfo],
    class_index: u16,
) -> Option<Arc<runtime::Class>> {
    // java.lang.Object
    if class_index == 0 {
        return None;
    }
    // TODO:
    None
}

fn load_interfaces(cp: &[class::ConstantPoolInfo], interfaces: &[u16]) -> Vec<Arc<runtime::Class>> {
    // TODO:
    vec![]
}

fn parse_constant_pool(cp: &Vec<class::ConstantPoolInfo>) -> Vec<runtime::ConstantPoolInfo> {
    let mut constant_pool = Vec::with_capacity(cp.len());
    for cp_info in cp {
        type Cpi = runtime::ConstantPoolInfo;
        let constant_pool_info = match cp_info {
            class::ConstantPoolInfo::Utf8(v) => Cpi::Utf8(Arc::clone(v)),
            class::ConstantPoolInfo::Integer(v) => Cpi::Integer(*v),
            class::ConstantPoolInfo::Float(v) => Cpi::Float(*v),
            class::ConstantPoolInfo::Long(v) => Cpi::Long(*v),
            class::ConstantPoolInfo::Double(v) => Cpi::Double(*v),
            class::ConstantPoolInfo::Class { name_index } => Cpi::Class(CpClassInfo {
                name: resolve_cp_utf8(cp, *name_index),
                class: RwLock::new(None),
            }),
            class::ConstantPoolInfo::String { string_index } => {
                Cpi::String(resolve_cp_utf8(cp, *string_index))
            }
            class::ConstantPoolInfo::Fieldref {
                class_index,
                name_and_type_index,
            } => Cpi::Fieldref {
                class: resolve_cp_class(cp, *class_index),
                name_and_type: resolve_cp_name_and_type_field(cp, *name_and_type_index),
                field_index: FieldIndex::Unresolved,
            },
            class::ConstantPoolInfo::Methodref {
                class_index,
                name_and_type_index,
            } => Cpi::Methodref {
                class: resolve_cp_class(cp, *class_index),
                name_and_type: resolve_cp_name_and_type_method(cp, *name_and_type_index),
            },
            class::ConstantPoolInfo::InterfaceMethodref {
                class_index,
                name_and_type_index,
            } => Cpi::InterfaceMethodref {
                class: resolve_cp_class(cp, *class_index),
                name_and_type: resolve_cp_name_and_type_method(cp, *name_and_type_index),
            },
            class::ConstantPoolInfo::NameAndType {
                name_index,
                descriptor_index,
            } => Cpi::NameAndType(resolve_cp_name_and_type(cp, *name_index, *descriptor_index)),
            class::ConstantPoolInfo::MethodHandle => Cpi::MethodHandle,
            class::ConstantPoolInfo::MethodType => Cpi::MethodType,
            class::ConstantPoolInfo::Dynamic => Cpi::Dynamic,
            class::ConstantPoolInfo::InvokeDynamic => Cpi::InvokeDynamic,
            class::ConstantPoolInfo::Module { name_index } => {
                Cpi::Module(resolve_cp_utf8(cp, *name_index))
            }
            class::ConstantPoolInfo::Package { name_index } => {
                Cpi::Package(resolve_cp_utf8(cp, *name_index))
            }
            class::ConstantPoolInfo::Empty => Cpi::Empty,
        };
        constant_pool.push(constant_pool_info);
    }

    constant_pool
}

fn parse_field(cp: &[class::ConstantPoolInfo], field: &class::FieldInfo) -> runtime::FieldInfo {
    let descriptor = resolve_cp_utf8(cp, field.descriptor_index);
    let (_, descriptor) = parse_field_descriptor(&descriptor).unwrap();
    runtime::FieldInfo {
        access_flags: field.access_flags,
        name: resolve_cp_utf8(cp, field.name_index),
        descriptor,
        attributes: field.attributes.iter().map(convert_attribute(cp)).collect(),
    }
}

fn parse_method(cp: &[class::ConstantPoolInfo], method: &class::MethodInfo) -> runtime::MethodInfo {
    let descriptor = resolve_cp_utf8(cp, method.descriptor_index);
    let (_, descriptor) = parse_method_descriptor(&descriptor).unwrap();
    runtime::MethodInfo {
        access_flags: method.access_flags,
        name: resolve_cp_utf8(cp, method.name_index),
        descriptor,
        attributes: method
            .attributes
            .iter()
            .map(convert_attribute(cp))
            .collect(),
    }
}

fn convert_attribute(
    constant_pool: &[class::ConstantPoolInfo],
) -> impl FnMut(&class::AttributeInfo) -> runtime::AttributeInfo + '_ {
    move |a| {
        // TODO: unwrap
        parse_attribute(a.attribute_name_index, &a.info, constant_pool)
            .unwrap()
            .1
    }
}

fn resolve_this_class_field_ref(
    fields: &[FieldInfo],
    cp: &mut [runtime::ConstantPoolInfo],
    class_name: &str,
) -> (usize, usize) {
    let mut field_map: HashMap<(&str, &FieldDescriptor), FieldIndex> = HashMap::new();
    let mut static_size = 0;
    let mut instance_size = 0;
    for field in fields {
        let size = if field.descriptor.0.is_long() { 2 } else { 1 };
        if field.access_flags.contains(FieldAccessFlag::STATIC) {
            field_map.insert(
                (&field.name, &field.descriptor),
                FieldIndex::Static(static_size),
            );
            static_size += size;
        } else {
            field_map.insert(
                (&field.name, &field.descriptor),
                FieldIndex::Instance(instance_size),
            );
            instance_size += size;
        }
    }
    for cp_in_file in cp.iter_mut() {
        match cp_in_file {
            runtime::ConstantPoolInfo::Fieldref {
                class,
                name_and_type,
                field_index,
            } => {
                if class.name.as_str() != class_name {
                    *field_index = FieldIndex::NotThisClass;
                    continue;
                }

                *field_index = field_map[&(name_and_type.name.as_str(), &name_and_type.descriptor)];
            }
            _ => continue,
        }
    }
    (instance_size as usize, static_size as usize)
}

fn resolve_cp_utf8(constant_pool: &[class::ConstantPoolInfo], index: u16) -> Arc<String> {
    let class::ConstantPoolInfo::Utf8(string) = &constant_pool[index as usize - 1] else {
        panic!("cannot find string {}", index);
    };
    Arc::clone(string)
}

fn resolve_cp_package(constant_pool: &[class::ConstantPoolInfo], index: u16) -> Arc<String> {
    let class::ConstantPoolInfo::Package { name_index } = &constant_pool[index as usize - 1] else {
        panic!("cannot find package {}", index);
    };
    resolve_cp_utf8(constant_pool, *name_index)
}

fn resolve_cp_module(constant_pool: &[class::ConstantPoolInfo], index: u16) -> Arc<String> {
    let class::ConstantPoolInfo::Module { name_index } = &constant_pool[index as usize - 1] else {
        panic!("cannot find module {}", index);
    };
    resolve_cp_utf8(constant_pool, *name_index)
}

fn resolve_cp_class(constant_pool: &[class::ConstantPoolInfo], class_index: u16) -> CpClassInfo {
    let class::ConstantPoolInfo::Class { name_index } = &constant_pool[class_index as usize - 1]
    else {
        panic!("cannot find class {}", class_index);
    };
    CpClassInfo {
        name: resolve_cp_utf8(constant_pool, *name_index),
        class: RwLock::new(None),
    }
}

fn resolve_cp_name_and_type_field(
    constant_pool: &[class::ConstantPoolInfo],
    index: u16,
) -> CpNameAndTypeInfo<FieldDescriptor> {
    let class::ConstantPoolInfo::NameAndType {
        name_index,
        descriptor_index,
    } = &constant_pool[index as usize - 1]
    else {
        panic!("cannot find name_and_type {}", index);
    };

    let name = resolve_cp_utf8(constant_pool, *name_index);
    let descriptor = resolve_cp_utf8(constant_pool, *descriptor_index);

    // TODO: unwrap
    let (_, descriptor) =
        descriptor::parse_field_descriptor(&descriptor).expect("invalid descriptor");

    CpNameAndTypeInfo::<FieldDescriptor> {
        name: Arc::clone(&name),
        descriptor,
    }
}

fn resolve_cp_name_and_type_method(
    constant_pool: &[class::ConstantPoolInfo],
    index: u16,
) -> CpNameAndTypeInfo<MethodDescriptor> {
    let class::ConstantPoolInfo::NameAndType {
        name_index,
        descriptor_index,
    } = &constant_pool[index as usize - 1]
    else {
        panic!("cannot find name_and_type {}", index);
    };

    let name = resolve_cp_utf8(constant_pool, *name_index);
    let descriptor = resolve_cp_utf8(constant_pool, *descriptor_index);

    // TODO: unwrap
    let (_, descriptor) =
        descriptor::parse_method_descriptor(&descriptor).expect("invalid descriptor");

    CpNameAndTypeInfo::<MethodDescriptor> {
        name: Arc::clone(&name),
        descriptor,
    }
}

fn resolve_cp_name_and_type(
    constant_pool: &[class::ConstantPoolInfo],
    name_index: u16,
    descriptor_index: u16,
) -> CpNameAndTypeInfo<Arc<String>> {
    let name = resolve_cp_utf8(constant_pool, name_index);
    let descriptor = resolve_cp_utf8(constant_pool, descriptor_index);
    CpNameAndTypeInfo::<Arc<String>> { name, descriptor }
}

fn resolve_constant_value(constant_pool: &[class::ConstantPoolInfo], index: u16) -> Const {
    match &constant_pool[index as usize - 1] {
        class::ConstantPoolInfo::Integer(i) => Const::Int(*i),
        class::ConstantPoolInfo::Float(f) => Const::Float(*f),
        class::ConstantPoolInfo::Long(l) => Const::Long(*l),
        class::ConstantPoolInfo::Double(d) => Const::Double(*d),
        class::ConstantPoolInfo::String { string_index } => {
            Const::String(resolve_cp_utf8(constant_pool, *string_index))
        }
        class::ConstantPoolInfo::Utf8(string) => Const::String(Arc::clone(string)),
        _ => {
            panic!("cannot find constant_value {}", index);
        }
    }
}

fn parse_attribute<'a>(
    attribute_name_index: u16,
    mut input: &'a [u8],
    constant_pool: &[class::ConstantPoolInfo],
) -> IResult<&'a [u8], runtime::AttributeInfo> {
    let attribute_name = resolve_cp_utf8(constant_pool, attribute_name_index);

    let attribute_info = match attribute_name.as_str() {
        "Code" => {
            let attribute_info;
            (input, attribute_info) = parse_code_attribute(input, constant_pool)?;
            attribute_info
        }
        "ConstantValue" => {
            let constantvalue_index;
            (input, constantvalue_index) = be_u16(input)?;
            runtime::AttributeInfo::ConstantValue(resolve_constant_value(
                constant_pool,
                constantvalue_index,
            ))
        }
        "RuntimeVisibleAnnotations" => {
            let (num_annotations, annotations);
            (input, num_annotations) = be_u16(input)?;
            (input, annotations) =
                count(parse_annotation(constant_pool), num_annotations as _)(input)?;

            runtime::AttributeInfo::RuntimeVisibleAnnotations(annotations)
        }
        "LocalVariableTable" => {
            let (local_variable_table_length, local_variable_table);
            (input, local_variable_table_length) = be_u16(input)?;
            (input, local_variable_table) = count(
                parse_local_variable(constant_pool),
                local_variable_table_length as _,
            )(input)?;

            runtime::AttributeInfo::LocalVariableTable(local_variable_table)
        }
        "Signature" => {
            let signature_index;
            (input, signature_index) = be_u16(input)?;
            runtime::AttributeInfo::Signature(resolve_cp_utf8(constant_pool, signature_index))
        }
        "Deprecated" => runtime::AttributeInfo::Deprecated,
        // TODO: only used for verification
        "StackMapTable" => runtime::AttributeInfo::StackMapTable(vec![]),
        // TODO: checked exception only
        "Exceptions" => runtime::AttributeInfo::Exceptions,
        "SourceFile" => {
            let sourcefile_index;
            (input, sourcefile_index) = be_u16(input)?;
            runtime::AttributeInfo::SourceFile(resolve_cp_utf8(constant_pool, sourcefile_index))
        }
        "LineNumberTable" => {
            let (line_number_table_length, line_number_table);
            (input, line_number_table_length) = be_u16(input)?;
            (input, line_number_table) = count(
                |input| {
                    let (input, start_pc) = be_u16(input)?;
                    let (input, line_number) = be_u16(input)?;
                    Ok((
                        input,
                        runtime::LineNumberTableItem {
                            start_pc,
                            line_number,
                        },
                    ))
                },
                line_number_table_length as _,
            )(input)?;
            runtime::AttributeInfo::LineNumberTable(line_number_table)
        }
        "Module" => {
            let (module_name_index, module_flags, module_version_index);
            (input, module_name_index) = be_u16(input)?;
            (input, module_flags) = be_u16(input)?;
            (input, module_version_index) = be_u16(input)?;

            let (requires_count, requires);
            (input, requires_count) = be_u16(input)?;
            (input, requires) = count(
                |input| {
                    let (input, requires_index) = be_u16(input)?;
                    let (input, requires_flags) = be_u16(input)?;
                    let (input, requires_version_index) = be_u16(input)?;
                    Ok((input, ()))
                },
                requires_count as _,
            )(input)?;

            let (exports_count, exports);
            (input, exports_count) = be_u16(input)?;
            (input, exports) = count(
                |input| {
                    let (input, exports_index) = be_u16(input)?;
                    let (input, exports_flags) = be_u16(input)?;
                    let (input, exports_to_count) = be_u16(input)?;
                    let (input, exports_to_index) = count(be_u16, exports_to_count as _)(input)?;

                    Ok((
                        input,
                        ModuleExport {
                            exports: resolve_cp_package(constant_pool, exports_index),
                            exports_flags,
                            exports_to: exports_to_index
                                .iter()
                                .map(|index| resolve_cp_module(constant_pool, *index))
                                .collect(),
                        },
                    ))
                },
                exports_count as _,
            )(input)?;

            let (opens_count, opens);
            (input, opens_count) = be_u16(input)?;
            (input, opens) = count(
                |input| {
                    let (input, opens_index) = be_u16(input)?;
                    let (input, opens_flags) = be_u16(input)?;
                    let (input, opens_to_count) = be_u16(input)?;
                    let (input, opens_to_index) = count(be_u16, opens_to_count as _)(input)?;
                    Ok((input, ()))
                },
                opens_count as _,
            )(input)?;

            let (uses_count, uses_index);
            (input, uses_count) = be_u16(input)?;
            (input, uses_index) = count(be_u16, uses_count as _)(input)?;

            let (provides_count, provides);
            (input, provides_count) = be_u16(input)?;
            (input, provides) = count(
                |input| {
                    let (input, provides_index) = be_u16(input)?;
                    let (input, provides_with_count) = be_u16(input)?;
                    let (input, provides_with_index) =
                        count(be_u16, provides_with_count as _)(input)?;
                    Ok((input, ()))
                },
                provides_count as _,
            )(input)?;
            runtime::AttributeInfo::Module(Module {
                // TODO:
                exports,
            })
        }
        "ModulePackages" => {
            let (package_count, package_index);
            (input, package_count) = be_u16(input)?;
            (input, package_index) = count(be_u16, package_count as _)(input)?;

            let packages = package_index
                .iter()
                .map(|package_index| resolve_cp_package(constant_pool, *package_index))
                .collect();

            runtime::AttributeInfo::ModulePackages(packages)
        }

        // ModuleHashes
        //  ModuleHashes_attribute {
        //    // index to CONSTANT_utf8_info structure in constant pool representing
        //    // the string "ModuleHashes"
        //    u2 attribute_name_index;
        //    u4 attribute_length;
        //
        //    // index to CONSTANT_utf8_info structure with algorithm name
        //    u2 algorithm_index;
        //
        //    // the number of entries in the hashes table
        //    u2 hashes_count;
        //    {   u2 module_name_index (index to CONSTANT_Module_info structure)
        //        u2 hash_length;
        //        u1 hash[hash_length];
        //    } hashes[hashes_count];
        //
        //  }
        // ModuleTarget
        // TargetPlatform_attribute {
        //    // index to CONSTANT_utf8_info structure in constant pool representing
        //    // the string "ModuleTarget"
        //    u2 attribute_name_index;
        //    u4 attribute_length;
        //
        //    // index to CONSTANT_utf8_info structure with the target platform
        //    u2 target_platform_index;
        //  }
        _ => {
            // TODO:
            eprintln!("Unknown attribute {}", attribute_name);
            // return Err(nom::Err::Error(error_position!(
            //     input,
            //     nom::error::ErrorKind::Tag
            // )));
            runtime::AttributeInfo::Unknown(attribute_name)
        }
    };

    Ok((input, attribute_info))
}

fn parse_attribute_raw(
    constant_pool: &[class::ConstantPoolInfo],
) -> impl FnMut(&[u8]) -> IResult<&[u8], runtime::AttributeInfo> + '_ {
    move |input| {
        let (input, attribute_name_index) = be_u16(input)?;
        let (input, attribute_length) = be_u32(input)?;
        let (_, attribute) = parse_attribute(
            attribute_name_index,
            input.slice(..attribute_length as _),
            constant_pool,
        )?;
        Ok((input.slice((attribute_length as _)..), attribute))
    }
}

fn parse_attributes<'a>(
    input: &'a [u8],
    constant_pool: &[class::ConstantPoolInfo],
) -> IResult<&'a [u8], Vec<runtime::AttributeInfo>> {
    let (input, attributes_count) = be_u16(input)?;

    let (input, attributes) =
        count(parse_attribute_raw(constant_pool), attributes_count as _)(input)?;

    Ok((input, attributes))
}

fn parse_code_attribute<'a>(
    input: &'a [u8],
    constant_pool: &[class::ConstantPoolInfo],
) -> IResult<&'a [u8], runtime::AttributeInfo> {
    let (input, max_stack) = be_u16(input)?;
    let (input, max_locals) = be_u16(input)?;

    let (input, code_length) = be_u32(input)?;
    let (input, code) = take(code_length)(input)?;

    let (input, exception_table_length) = be_u16(input)?;

    let (input, exception_table) = count(
        parse_exception_table(constant_pool),
        exception_table_length as _,
    )(input)?;

    let (input, attributes) = parse_attributes(input, constant_pool)?;

    Ok((
        input,
        runtime::AttributeInfo::Code(runtime::CodeAttribute {
            max_stack,
            max_locals,
            code: code.into(),
            exception_table,
            attributes,
        }),
    ))
}

fn parse_exception_table(
    constant_pool: &[class::ConstantPoolInfo],
) -> impl FnMut(&[u8]) -> IResult<&[u8], runtime::ExceptionTableItem> + '_ {
    move |input| {
        let (input, start_pc) = be_u16(input)?;
        let (input, end_pc) = be_u16(input)?;
        let (input, handler_pc) = be_u16(input)?;
        let (input, catch_type) = be_u16(input)?;

        let catch_type_info = if catch_type == 0 {
            None
        } else {
            Some(resolve_cp_class(constant_pool, catch_type))
        };

        Ok((
            input,
            runtime::ExceptionTableItem {
                start_pc,
                end_pc,
                handler_pc,
                catch_type: catch_type_info,
            },
        ))
    }
}

fn parse_annotation(
    constant_pool: &[class::ConstantPoolInfo],
) -> impl FnMut(&[u8]) -> IResult<&[u8], Annotation> + '_ {
    move |input| {
        let (input, type_index) = be_u16(input)?;
        let type_str = resolve_cp_utf8(constant_pool, type_index);
        let (input, num_element_value_pairs) = be_u16(input)?;
        let (input, element_value_pairs) = count(
            parse_element_value_pair(constant_pool),
            num_element_value_pairs as _,
        )(input)?;

        Ok((
            input,
            Annotation {
                // TODO: unwrap
                type_descriptor: parse_field_descriptor(&type_str).unwrap().1,
                element_value_pairs,
            },
        ))
    }
}

fn parse_element_value_pair(
    constant_pool: &[class::ConstantPoolInfo],
) -> impl FnMut(&[u8]) -> IResult<&[u8], ElementValuePair> + '_ {
    move |input| {
        let (input, element_name_index) = be_u16(input)?;
        let element_name = resolve_cp_utf8(constant_pool, element_name_index);
        let (input, value) = parse_element_value(constant_pool)(input)?;
        Ok((
            input,
            ElementValuePair {
                element_name,
                value,
            },
        ))
    }
}

fn parse_element_value(
    constant_pool: &[class::ConstantPoolInfo],
) -> impl FnMut(&[u8]) -> IResult<&[u8], ElementValue> + '_ {
    move |input| {
        let (input, tag) = u8(input)?;
        let mut input = input;
        let value = match tag {
            b'B' | b'C' | b'D' | b'F' | b'I' | b'J' | b'S' | b'Z' | b's' => {
                let converter = match tag {
                    b'B' => Const::to_byte,
                    b'C' => Const::to_char,
                    b'S' => Const::to_short,
                    b'Z' => Const::to_boolean,
                    b'D' | b'F' | b'I' | b'J' | b's' => identity,
                    _ => unreachable!("all case covered"),
                };

                let c;
                (input, c) = parse_constant_value(input, constant_pool, converter)?;
                ElementValue::Const(c)
            }
            b'e' => {
                let (type_name_index, const_name_index);
                (input, type_name_index) = be_u16(input)?;
                (input, const_name_index) = be_u16(input)?;
                ElementValue::Enum {
                    type_name: resolve_cp_utf8(constant_pool, type_name_index),
                    const_name: resolve_cp_utf8(constant_pool, const_name_index),
                }
            }
            b'c' => {
                let class_info_index;
                (input, class_info_index) = be_u16(input)?;
                let class_info = resolve_cp_utf8(constant_pool, class_info_index);
                // TODO: unwrap
                let class = parse_return_type_descriptor(&class_info).unwrap().1;

                ElementValue::Class(class)
            }
            b'@' => {
                let annotation;
                (input, annotation) = parse_annotation(constant_pool)(input)?;
                ElementValue::Annotation(annotation)
            }
            b'[' => {
                let (num_values, values);
                (input, num_values) = be_u16(input)?;
                (input, values) =
                    count(parse_element_value(constant_pool), num_values as _)(input)?;
                ElementValue::Array(values)
            }
            _ => {
                eprintln!("unkonwn element value tag {}", tag);
                return Err(nom::Err::Error(error_position!(
                    input,
                    nom::error::ErrorKind::Tag
                )));
            }
        };
        Ok((input, value))
    }
}

fn parse_constant_value<'a>(
    input: &'a [u8],
    constant_pool: &[class::ConstantPoolInfo],
    converter: impl FnOnce(runtime::Const) -> runtime::Const,
) -> IResult<&'a [u8], runtime::Const> {
    let (input, const_value_index) = be_u16(input)?;
    let value = resolve_constant_value(constant_pool, const_value_index);
    Ok((input, converter(value)))
}

fn parse_local_variable(
    constant_pool: &[class::ConstantPoolInfo],
) -> impl FnMut(&[u8]) -> IResult<&[u8], LocalVariable> + '_ {
    move |input| {
        let (input, start_pc) = be_u16(input)?;
        let (input, length) = be_u16(input)?;
        let (input, name_index) = be_u16(input)?;
        let (input, descriptor_index) = be_u16(input)?;
        let descriptor = resolve_cp_utf8(constant_pool, descriptor_index);
        // TODO: unwrap
        let (_, descriptor) = parse_field_descriptor(&descriptor).unwrap();
        let (input, index) = be_u16(input)?;

        Ok((
            input,
            LocalVariable {
                start_pc,
                length,
                name: resolve_cp_utf8(constant_pool, name_index),
                descriptor,
                index,
            },
        ))
    }
}
