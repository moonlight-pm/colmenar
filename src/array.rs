use crate::{generate::*, Model};
use heck::ToSnakeCase;
use openapiv3::{ReferenceOr, Schema, SchemaKind, Type};

use crate::{err, Error};

pub struct Array {}

impl Array {
    pub fn discover(name: &str, schema: &Schema) -> Result<Tokens, Error> {
        Ok(match &schema.schema_kind {
            SchemaKind::Type(Type::Array(array)) => match array.items.as_ref().unwrap() {
                ReferenceOr::Reference { reference, .. } => {
                    let ty = reference.split("/").last().unwrap().to_string();
                    let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                    quote!(Vec<$ty>)
                }
                ReferenceOr::Item(item) => match &item.schema_kind {
                    SchemaKind::Type(Type::String(_)) => {
                        quote!(Vec<String>)
                    }
                    SchemaKind::Type(Type::Integer(_)) => {
                        quote!(Vec<i64>)
                    }
                    SchemaKind::Type(Type::Object(_)) => {
                        Model::discover(name, item)?;
                        let ty = rust::import(format!("super::{}", name.to_snake_case()), name);
                        quote!(Vec<$ty>)
                    }
                    _ => return err!("Unhandled array type for: {:?}", item.schema_kind,),
                },
            },
            _ => return err!("Passed wrong type to Array::discover"),
        })
    }
}
