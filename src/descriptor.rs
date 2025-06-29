use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::take_until,
    character::complete::{char, one_of},
    combinator::{eof, map},
    multi::many0,
    sequence::delimited,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldDescriptor(pub(crate) FieldType);

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct MethodDescriptor {
    pub(crate) parameters: Vec<FieldType>,
    pub(crate) return_type: ReturnType,
}

pub type ReturnType = Option<FieldType>;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum FieldType {
    Byte,
    Char,
    Double,
    Float,
    Int,
    Long,
    Object(String),
    Short,
    Boolean,
    Array(Box<FieldType>),
}

impl FieldType {
    pub fn is_long(&self) -> bool {
        matches!(self, FieldType::Long | FieldType::Double)
    }

    pub fn is_primitive(&self) -> bool {
        matches!(
            self,
            FieldType::Byte
                | FieldType::Char
                | FieldType::Double
                | FieldType::Float
                | FieldType::Int
                | FieldType::Long
                | FieldType::Short
                | FieldType::Boolean
        )
    }

    pub fn to_descriptor(&self) -> String {
        match self {
            FieldType::Byte => "B".to_string(),
            FieldType::Char => "C".to_string(),
            FieldType::Double => "D".to_string(),
            FieldType::Float => "F".to_string(),
            FieldType::Int => "I".to_string(),
            FieldType::Long => "J".to_string(),
            FieldType::Short => "S".to_string(),
            FieldType::Boolean => "Z".to_string(),
            FieldType::Object(class_name) => format!("L{class_name};"),
            FieldType::Array(element_type) => format!("[{}", element_type.to_descriptor()),
        }
    }
    pub fn get_field_type_size(&self) -> usize {
        match self {
            FieldType::Byte => 1,
            FieldType::Char => 2,
            FieldType::Double => 8,
            FieldType::Float => 4,
            FieldType::Int => 4,
            FieldType::Long => 8,
            FieldType::Object(_) => 4,
            FieldType::Short => 2,
            FieldType::Boolean => 1,
            FieldType::Array(_) => 4,
        }
    }
}

pub fn parse_field_descriptor(input: &str) -> IResult<&str, FieldDescriptor> {
    let (input, field_type) = parse_field_type(input)?;
    eof(input)?;
    Ok((input, FieldDescriptor(field_type)))
}

pub fn parse_method_descriptor(input: &str) -> IResult<&str, MethodDescriptor> {
    let (input, parameters) =
        delimited(char('('), many0(parse_field_type), char(')')).parse(input)?;

    let (input, return_type) = parse_return_type_descriptor(input)?;

    eof(input)?;
    Ok((
        input,
        MethodDescriptor {
            parameters,
            return_type,
        },
    ))
}

pub fn parse_return_type_descriptor(input: &str) -> IResult<&str, ReturnType> {
    alt((map(parse_field_type, Some), parse_void_type)).parse(input)
}

fn parse_field_type(input: &str) -> IResult<&str, FieldType> {
    alt((parse_base_type, parse_object_type, parse_array_type)).parse(input)
}

fn parse_base_type(input: &str) -> IResult<&str, FieldType> {
    let (input, ch) = one_of("BCDFIJSZ")(input)?;
    let field_type = match ch {
        'B' => FieldType::Byte,
        'C' => FieldType::Char,
        'D' => FieldType::Double,
        'F' => FieldType::Float,
        'I' => FieldType::Int,
        'J' => FieldType::Long,
        'S' => FieldType::Short,
        'Z' => FieldType::Boolean,
        _ => {
            todo!("error")
        }
    };
    Ok((input, field_type))
}

fn parse_object_type(input: &str) -> IResult<&str, FieldType> {
    let (input, _) = char('L')(input)?;

    let (input, class_name) = take_until(";")(input)?;

    let (input, _) = char(';')(input)?;

    Ok((input, FieldType::Object(class_name.to_string())))
}

fn parse_array_type(input: &str) -> IResult<&str, FieldType> {
    let (input, _) = char('[')(input)?;

    let (input, field_type) = parse_field_type(input)?;

    Ok((input, FieldType::Array(Box::new(field_type))))
}

fn parse_void_type(input: &str) -> IResult<&str, Option<FieldType>> {
    let (input, _) = char('V')(input)?;
    Ok((input, None))
}
