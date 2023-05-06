use crate::prelude::*;
use indexmap::IndexMap;
use openapiv3::{ReferenceOr, Schema, SchemaKind, Type};

#[derive(Clone)]
pub struct Property {
    pub name: String,
    pub ty: Tokens,
    pub description: Option<String>,
    pub required: bool,
    pub nullable: bool,
}

impl Property {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ty: quote!(String),
            description: None,
            required: false,
            nullable: false,
        }
    }

    pub fn discover(
        model: &mut Model,
        required: &Vec<String>,
        indexmap: &IndexMap<String, ReferenceOr<Box<Schema>>>,
    ) -> Result<(), Error> {
        for (name, schema) in indexmap.iter() {
            let mut property = Property::new(name);
            property.required = required.contains(name);
            match schema {
                ReferenceOr::Reference {
                    reference,
                    description,
                    ..
                } => {
                    property.description = description.clone();
                    let ty = reference.split("/").last().unwrap().to_string();
                    // let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                    property.ty = quote!($ty);
                }
                ReferenceOr::Item(item) => {
                    property.nullable = item.schema_data.nullable;
                    property.description = item.schema_data.description.clone();
                    let ty = format!("{}_{name}", model.name).to_upper_camel_case();
                    match &item.schema_kind {
                        SchemaKind::Type(Type::String(string)) => {
                            if string.enumeration.is_empty() {
                                property.ty = quote!(String)
                            } else {
                                Model::discover(&ty, item)?;
                                // let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                                property.ty = quote!($ty)
                            }
                        }
                        SchemaKind::Type(Type::Boolean {}) => property.ty = quote!(bool),
                        SchemaKind::Type(Type::Integer(integer)) => {
                            if integer.enumeration.is_empty() {
                                property.ty = quote!(i64)
                            } else {
                                Model::discover(&ty, item)?;
                                // let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                                property.ty = quote!($ty)
                            }
                        }
                        SchemaKind::Type(Type::Number(number)) => {
                            if number.enumeration.is_empty() {
                                property.ty = quote!(f64)
                            } else {
                                Model::discover(&ty, item)?;
                                // let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                                property.ty = quote!($ty)
                            }
                        }
                        SchemaKind::Type(Type::Object(_)) => {
                            Model::discover(&ty, item)?;
                            // let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                            property.ty = quote!($ty)
                        }
                        SchemaKind::Type(Type::Array(_)) => {
                            property.ty = Array::discover(&ty, item)?;
                        }
                        SchemaKind::Any(_) => {
                            Model::discover(&ty, item)?;
                            // let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                            property.ty = quote!($ty)
                        }
                        SchemaKind::AllOf { .. } => {
                            Model::discover(&ty, item)?;
                            // let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                            property.ty = quote!($ty);
                        }
                        SchemaKind::OneOf { .. } => {
                            let ty = format!("{}_{name}", model.name).to_upper_camel_case();
                            Model::discover(&ty, item)?;
                            // let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                            property.ty = quote!($ty);
                        }
                        _ => {
                            return err!(
                                "Unhandled property type for '{}.{name}: {:?}",
                                model.name,
                                item.schema_kind,
                            )
                        }
                    }
                }
            };
            model.properties.push(property);
        }
        Ok(())
    }
}
