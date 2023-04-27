use crate::{err, generate::*, Enumeration, Error, Property};
use heck::{ToSnakeCase, ToUpperCamelCase};
use indexmap::IndexMap;
use once_cell::sync::OnceCell;
use openapiv3::{ReferenceOr, Schema, SchemaKind, Type};
use std::{collections::HashMap, sync::Mutex};

static MODELS: OnceCell<Mutex<HashMap<String, Model>>> = OnceCell::new();

#[derive(Clone)]
pub struct Model {
    pub path: String,
    pub name: String,
    pub ty: Option<Tokens>,
    pub description: Option<String>,
    pub properties: Vec<Property>,
    pub enumeration: Option<Enumeration>,
}

impl Model {
    pub fn all() -> Vec<Model> {
        MODELS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    fn add(model: Model) {
        MODELS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .insert(model.name.clone(), model);
    }

    fn get(name: &str) -> Option<Model> {
        MODELS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .get(name)
            .cloned()
    }

    pub fn discover(name: &str, schema: &Schema) -> Result<(), Error> {
        let path = name.to_snake_case();
        let mut model = Self {
            path: path.clone(),
            name: name.to_string(),
            ty: None,
            description: None,
            properties: Vec::new(),
            enumeration: None,
        };

        model.description = schema.schema_data.description.clone();
        match &schema.schema_kind {
            SchemaKind::Type(Type::String(_)) => {
                model.enumeration = Enumeration::discover(schema);
                if model.enumeration.is_none() {
                    model.ty = Some(quote!(String));
                }
            }
            SchemaKind::Type(Type::Integer(_)) => {
                model.enumeration = Enumeration::discover(schema);
                if model.enumeration.is_none() {
                    model.ty = Some(quote!(i64));
                }
            }
            SchemaKind::Type(Type::Object(object)) => {
                model.discover_properties(&object.required, &object.properties)?;
            }
            SchemaKind::AllOf { all_of } => {
                for schema in all_of.iter() {
                    match schema {
                        ReferenceOr::Reference { reference, .. } => {
                            let reference = reference
                                .as_str()
                                .split('/')
                                .last()
                                .unwrap()
                                .to_upper_camel_case();
                            let reference = Model::get(&reference).unwrap();
                            model.properties.extend(reference.properties.clone());
                        }
                        ReferenceOr::Item(item) => match &item.schema_kind {
                            SchemaKind::Type(Type::Object(object)) => {
                                model.discover_properties(&object.required, &object.properties)?;
                            }
                            SchemaKind::Any(schema) => {
                                model.discover_properties(&schema.required, &schema.properties)?;
                            }
                            _ => return err!("Unhandled type: {:?} {name:?}", item.schema_kind,),
                        },
                    }
                }
            }
            _ => return err!("Unhandled kind {:?}", schema.schema_kind),
        };
        Model::add(model);
        Ok(())
    }

    pub fn discover_properties(
        &mut self,
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
                    let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                    property.ty = quote!($ty);
                }
                ReferenceOr::Item(item) => {
                    property.description = item.schema_data.description.clone();
                    let ty = format!("{}_{name}", self.name).to_upper_camel_case();
                    match &item.schema_kind {
                        SchemaKind::Type(Type::String(string)) => {
                            if string.enumeration.is_empty() {
                                property.ty = quote!(String)
                            } else {
                                Model::discover(&ty, item)?;
                                let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                                property.ty = quote!($ty)
                            }
                        }
                        SchemaKind::Type(Type::Boolean {}) => property.ty = quote!(bool),
                        SchemaKind::Type(Type::Integer(integer)) => {
                            if integer.enumeration.is_empty() {
                                property.ty = quote!(i64)
                            } else {
                                Model::discover(&ty, item)?;
                                let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                                property.ty = quote!($ty)
                            }
                        }
                        SchemaKind::Type(Type::Object(_)) => {
                            Model::discover(&ty, item)?;
                            let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                            property.ty = quote!($ty)
                        }
                        _ => {
                            return err!("Unhandled property type: {:?} {name:?}", item.schema_kind,)
                        }
                    }
                }
            };
            self.properties.push(property);
        }
        Ok(())
    }

    pub fn add_property(&mut self, property: Property) {
        self.properties.push(property);
    }

    pub fn write(&self, dir: &str) -> Result<(), Error> {
        let path = format!("{dir}/models/{}.rs", self.path);
        let mut tokens = Tokens::new();
        if let Some(description) = &self.description {
            tokens.append(quote!(
                #[doc=$(quoted(description))]
            ));
        }
        tokens.append(match &self.ty {
            Some(ty) => quote!(
                pub type $(&self.name) = $ty;
            ),
            None =>  {
                match &self.enumeration {
                    Some(enumeration) => {
                        match enumeration {
                            Enumeration::String(values) => {
                                let values = values.into_iter().map(|s| {
                                    if s.chars().next().unwrap().is_digit(10) {
                                        format!("{}{}", &self.name, s)
                                    } else {
                                        s.to_string()
                                    }
                                }).collect::<Vec<_>>();
                                quote!(
                                    pub enum $(&self.name) {
                                        $(for value in values => $value,)
                                    }
                                )
                            }
                            Enumeration::Integer(values) => {
                                quote!(
                                    pub enum $(&self.name) {
                                        $(for value in values =>
                                            $(&self.name)$(*value) = $(*value),
                                        )
                                    }
                                )
                            }
                        }
                    }
                    None => {
                        quote!(
                            pub struct $(&self.name) {
                                $(for property in &self.properties =>
                                    $(property.description.as_ref().map(|description| quote!(#[doc=$(quoted(description))])))
                                    pub $(&property.name): $(if property.required { $(&property.ty) } else { Option<$(&property.ty)> }),
                                )
                            }
                        )
                    }
                }
            }
        });
        write_tokens(&path, tokens)?;
        Ok(())
    }
}
