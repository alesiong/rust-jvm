use std::sync::Arc;

use nom::{
    bytes::complete::{tag, take},
    combinator::eof,
    error_position,
    multi::{count, fold_many_m_n},
    number::complete::{be_f32, be_f64, be_i32, be_i64, be_u16, be_u32, u8},
    IResult,
};

use crate::{
    descriptor::{parse_field_descriptor, parse_method_descriptor},
    structs::{
        AttributeInfo, Class, ClassAccessFlag, CodeAttribute, ConstantPoolInfo, ExceptionTableItem,
        FieldAccessFlag, FieldInfo, LineNumberTableItem, MethodAccessFlag, MethodInfo,
    },
};

pub fn class_file(input: &[u8]) -> IResult<&[u8], Class> {
    let (input, (minor, major)) = parse_header(input)?;
    let (input, constant_pool) = parse_constant_pool(input)?;

    let (input, access_flags) = be_u16(input)?;
    let (input, this_class) = be_u16(input)?;
    let (input, super_class) = be_u16(input)?;
    let (input, interfaces) = parse_interfaces(input)?;
    let (input, fields) = parse_fields(input, &constant_pool)?;
    let (input, methods) = parse_methods(input, &constant_pool)?;
    let (input, attributes) = parse_attributes(input, &constant_pool)?;

    eof(input)?;

    Ok((
        input,
        Class {
            major_version: major,
            minor_version: minor,
            // SAFETY: extra bits allowed
            access_flags: unsafe { ClassAccessFlag::from_bits_unchecked(access_flags) },
            // TODO: unwrap
            this_class: Class::resolve_class_constant(&constant_pool, this_class).unwrap(),
            super_class: Class::resolve_class_constant(&constant_pool, super_class),
            constant_pool,
            interfaces,
            fields,
            methods,
            attributes,
        },
    ))
}

fn parse_header(input: &[u8]) -> IResult<&[u8], (u16, u16)> {
    let (input, _) = tag(&[0xca, 0xfe, 0xba, 0xbe])(input)?;
    let (input, minor) = be_u16(input)?;
    let (input, major) = be_u16(input)?;
    Ok((input, (minor, major)))
}

fn parse_constant_pool(input: &[u8]) -> IResult<&[u8], Vec<ConstantPoolInfo>> {
    let (input, constant_pool_count) = be_u16(input)?;

    let mut constant_pool = Vec::with_capacity(constant_pool_count as usize - 1);

    let mut input = input;

    while constant_pool.len() < constant_pool_count as usize - 1 {
        let constant;
        (input, constant) = parse_constant(input)?;
        let need_empty = matches!(
            constant,
            ConstantPoolInfo::Long(_) | ConstantPoolInfo::Double(_)
        );
        constant_pool.push(constant);
        if need_empty {
            constant_pool.push(ConstantPoolInfo::Empty);
        }
    }

    Ok((input, constant_pool))
}

fn parse_constant(mut input: &[u8]) -> IResult<&[u8], ConstantPoolInfo> {
    let tag;
    (input, tag) = u8(input)?;
    let cp_info = match tag {
        1 => {
            let length;
            (input, length) = be_u16(input)?;
            let bytes;
            (input, bytes) = take(length)(input)?;
            ConstantPoolInfo::Utf8 {
                // TODO: unwrap
                bytes: Arc::new(cesu8::from_java_cesu8(bytes).unwrap().into_owned()),
            }
        }
        3 => {
            let int;
            (input, int) = be_i32(input)?;
            ConstantPoolInfo::Integer(int)
        }
        4 => {
            let float;
            (input, float) = be_f32(input)?;
            ConstantPoolInfo::Float(float)
        }
        5 => {
            let long;
            (input, long) = be_i64(input)?;
            // TODO: extra cp entry
            ConstantPoolInfo::Long(long)
        }
        6 => {
            let double;
            (input, double) = be_f64(input)?;
            ConstantPoolInfo::Double(double)
        }
        7 => {
            let name_index;
            (input, name_index) = be_u16(input)?;

            ConstantPoolInfo::Class { name_index }
        }
        8 => {
            let string_index;
            (input, string_index) = be_u16(input)?;

            ConstantPoolInfo::String { string_index }
        }
        9 => {
            let (class_index, name_and_type_index);
            (input, class_index) = be_u16(input)?;
            (input, name_and_type_index) = be_u16(input)?;
            ConstantPoolInfo::Fieldref {
                class_index,
                name_and_type_index,
            }
        }
        10 => {
            let (class_index, name_and_type_index);
            (input, class_index) = be_u16(input)?;
            (input, name_and_type_index) = be_u16(input)?;
            ConstantPoolInfo::Methodref {
                class_index,
                name_and_type_index,
            }
        }
        11 => {
            let (class_index, name_and_type_index);
            (input, class_index) = be_u16(input)?;
            (input, name_and_type_index) = be_u16(input)?;
            ConstantPoolInfo::InterfaceMethodref {
                class_index,
                name_and_type_index,
            }
        }
        12 => {
            let (name_index, descriptor_index);
            (input, name_index) = be_u16(input)?;
            (input, descriptor_index) = be_u16(input)?;
            ConstantPoolInfo::NameAndType {
                name_index,
                descriptor_index,
            }
        }
        _ => {
            eprintln!("unkonwn constant type {}", tag);
            return Err(nom::Err::Error(error_position!(
                input,
                nom::error::ErrorKind::Tag
            )));
        }
    };
    Ok((input, cp_info))
}

fn parse_interfaces(input: &[u8]) -> IResult<&[u8], Vec<u16>> {
    let (input, interface_count) = be_u16(input)?;

    let (input, interfaces) = count(be_u16, interface_count as _)(input)?;

    Ok((input, interfaces))
}

fn parse_fields<'a>(
    input: &'a [u8],
    pool: &[ConstantPoolInfo],
) -> IResult<&'a [u8], Vec<FieldInfo>> {
    let (input, field_count) = be_u16(input)?;
    let (input, fields) = count(parse_field(pool), field_count as _)(input)?;
    Ok((input, fields))
}

fn parse_field(pool: &[ConstantPoolInfo]) -> impl FnMut(&[u8]) -> IResult<&[u8], FieldInfo> + '_ {
    move |input| {
        let (input, access_flags) = be_u16(input)?;
        let (input, name_index) = be_u16(input)?;
        let (input, descriptor_index) = be_u16(input)?;

        let (input, attributes) = parse_attributes(input, pool)?;

        // TODO: unwrap
        let descriptor = Class::resolve_utf8_constant(pool, descriptor_index).unwrap();
        let (_, descriptor) = parse_field_descriptor(&descriptor).unwrap();

        Ok((
            input,
            FieldInfo {
                // SAFETY: allow extra bits
                access_flags: unsafe { FieldAccessFlag::from_bits_unchecked(access_flags) },
                // TODO: unwrap
                name: Class::resolve_utf8_constant(pool, name_index).unwrap(),
                descriptor,
                attributes,
            },
        ))
    }
}

fn parse_attributes<'a>(
    input: &'a [u8],
    pool: &[ConstantPoolInfo],
) -> IResult<&'a [u8], Vec<AttributeInfo>> {
    let (input, attributes_count) = be_u16(input)?;

    let (input, attributes) = count(parse_attribute(pool), attributes_count as _)(input)?;

    Ok((input, attributes))
}

fn parse_attribute(
    pool: &[ConstantPoolInfo],
) -> impl FnMut(&[u8]) -> IResult<&[u8], AttributeInfo> + '_ {
    move |input| {
        let (input, attribute_name_index) = be_u16(input)?;
        let (input, attribute_length) = be_u32(input)?;
        // let (input, info) = take(attribute_length)(input)?;

        let mut input = input;

        // TODO: unwrap
        let attribute_name = Class::resolve_utf8_constant(pool, attribute_name_index).unwrap();

        let attribute_info = match attribute_name.as_str() {
            "Code" => {
                let (max_stack, max_locals);
                (input, max_stack) = be_u16(input)?;
                (input, max_locals) = be_u16(input)?;

                let (code_length, code);

                (input, code_length) = be_u32(input)?;
                (input, code) = take(code_length)(input)?;

                let (exception_table_length, exception_table);
                (input, exception_table_length) = be_u16(input)?;

                (input, exception_table) =
                    count(parse_exception_table, exception_table_length as _)(input)?;

                let attributes;
                (input, attributes) = parse_attributes(input, pool)?;

                AttributeInfo::Code(CodeAttribute {
                    max_stack,
                    max_locals,
                    code: code.to_vec(),
                    exception_table,
                    attributes,
                })
            }
            "SourceFile" => {
                let sourcefile_index;
                (input, sourcefile_index) = be_u16(input)?;
                // TODO: unwrap
                AttributeInfo::SourceFile {
                    sourcefile: Class::resolve_utf8_constant(pool, sourcefile_index).unwrap(),
                }
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
                            LineNumberTableItem {
                                start_pc,
                                line_number,
                            },
                        ))
                    },
                    line_number_table_length as _,
                )(input)?;
                AttributeInfo::LineNumberTable(line_number_table)
            }
            _ => {
                // TODO:
                // return Err(nom::Err::Error(error_position!(
                //     input,
                //     nom::error::ErrorKind::Tag
                // )));
                let info;
                (input, info) = take(attribute_length)(input)?;
                AttributeInfo::Unknown(Arc::clone(&attribute_name), info.to_vec())
            }
        };

        Ok((input, attribute_info))
    }
}

fn parse_methods<'a>(
    input: &'a [u8],
    constant_pool: &[ConstantPoolInfo],
) -> IResult<&'a [u8], Vec<MethodInfo>> {
    let (input, methods_count) = be_u16(input)?;

    let (input, methods) = count(parse_method(constant_pool), methods_count as _)(input)?;

    Ok((input, methods))
}

fn parse_method(pool: &[ConstantPoolInfo]) -> impl FnMut(&[u8]) -> IResult<&[u8], MethodInfo> + '_ {
    move |input| {
        let (input, access_flags) = be_u16(input)?;
        let (input, name_index) = be_u16(input)?;
        let (input, descriptor_index) = be_u16(input)?;
        let (input, attributes) = parse_attributes(input, pool)?;

        // TODO: unwrap
        let descriptor = Class::resolve_utf8_constant(pool, descriptor_index).unwrap();
        let (_, descriptor) = parse_method_descriptor(&descriptor).unwrap();
        Ok((
            input,
            MethodInfo {
                // SAFETY: allow extra bits
                access_flags: unsafe { MethodAccessFlag::from_bits_unchecked(access_flags) },
                // TODO: unwrap
                name: Class::resolve_utf8_constant(pool, name_index).unwrap(),
                descriptor,
                attributes,
            },
        ))
    }
}

fn parse_exception_table(input: &[u8]) -> IResult<&[u8], ExceptionTableItem> {
    let (input, start_pc) = be_u16(input)?;
    let (input, end_pc) = be_u16(input)?;
    let (input, handler_pc) = be_u16(input)?;
    let (input, catch_type) = be_u16(input)?;

    Ok((
        input,
        ExceptionTableItem {
            start_pc,
            end_pc,
            handler_pc,
            catch_type,
        },
    ))
}
