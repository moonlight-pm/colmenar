use crate::prelude::*;
use openapiv3::{ReferenceOr, Schema, SchemaKind, Type};

pub struct Array {}

impl Array {
    pub fn discover(name: &str, schema: &Schema) -> Result<Tokens, Error> {
        Ok(match &schema.schema_kind {
            SchemaKind::Type(Type::Array(array)) => match array.items.as_ref().unwrap() {
                ReferenceOr::Reference { reference, .. } => {
                    let ty = reference.split("/").last().unwrap().to_string();
                    quote!(Vec<$ty>)
                }
                ReferenceOr::Item(item) => match &item.schema_kind {
                    SchemaKind::Type(Type::String(string)) => {
                        if string.enumeration.is_empty() {
                            quote!(Vec<String>)
                        } else {
                            Model::discover(name, item)?;
                            quote!(Vec<$name>)
                        }
                    }
                    SchemaKind::Type(Type::Integer(integer)) => {
                        if integer.enumeration.is_empty() {
                            quote!(Vec<i64>)
                        } else {
                            Model::discover(name, item)?;
                            quote!(Vec<$name>)
                        }
                    }
                    SchemaKind::Type(Type::Object(_)) => {
                        Model::discover(name, item)?;
                        quote!(Vec<$name>)
                    }
                    _ => return err!("Unhandled array type for {name}: {:?}", item.schema_kind,),
                },
            },
            _ => return err!("Passed wrong type to Array::discover"),
        })
    }
}
