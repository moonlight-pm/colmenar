use crate::{constants::KEYWORDS, err, generate::*, Enumeration, Error, Property};
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
                            let reference =
                                reference.split('/').last().unwrap().to_upper_camel_case();
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
                        SchemaKind::Type(Type::Array(array)) => {
                            match array.items.as_ref().unwrap() {
                                ReferenceOr::Reference { reference, .. } => {
                                    let ty = reference.split("/").last().unwrap().to_string();
                                    let ty =
                                        rust::import(format!("super::{}", ty.to_snake_case()), ty);
                                    property.ty = quote!(Vec<$ty>);
                                }
                                ReferenceOr::Item(item) => match &item.schema_kind {
                                    // SchemaKind::Type(Type::String(_)) => {
                                    //     property.ty = quote!(Vec<String>);
                                    // }
                                    // SchemaKind::Type(Type::Integer(_)) => {
                                    //     property.ty = quote!(Vec<i64>);
                                    // }
                                    SchemaKind::Type(Type::Object(_)) => {
                                        Model::discover(&ty, item)?;
                                        let ty = rust::import(
                                            format!("super::{}", ty.to_snake_case()),
                                            ty,
                                        );
                                        property.ty = quote!(Vec<$ty>);
                                    }
                                    _ => {
                                        return err!(
                                            "Unhandled array type: {:?} {name:?}",
                                            item.schema_kind,
                                        )
                                    }
                                },
                            }
                        }
                        SchemaKind::AllOf { .. } => {
                            Model::discover(&ty, item)?;
                            let ty = rust::import(format!("super::{}", ty.to_snake_case()), ty);
                            property.ty = quote!($ty);
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

    pub fn write(&self, dir: &str) -> Result<(), Error> {
        let import_serialize = rust::import("serde", "Serialize");
        let import_deserialize = rust::import("serde", "Deserialize");
        // let import_display = rust::import("std::fmt", "Display");
        // let import_formatter = rust::import("std::fmt", "Formatter");
        // let import_fromstr = rust::import("std::str", "FromStr");
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
                                let range = 0..values.len();
                                let variants = values.into_iter().map(|s| {
                                    if s.chars().next().unwrap().is_digit(10) {
                                        format!("{}{}", &self.name, s).to_upper_camel_case()
                                    } else {
                                        s.to_upper_camel_case()
                                    }
                                }).collect::<Vec<_>>();
                                quote!(
                                    #[derive(Debug, Clone, PartialEq, Eq, Hash, $import_serialize, $import_deserialize)]
                                    pub enum $(&self.name) {
                                        $(for v in range =>
                                            #[serde(rename = $(quoted(&values[v])))]
                                            $(&variants[v]),
                                        )
                                    }
                                    // $['\n']
                                    // impl $import_fromstr for $(&self.name) {
                                    //     type Err = String;
                                    //     $['\n']
                                    //     fn from_str(s: &str) -> Result<Self, Self::Err> {
                                    //         match s {
                                    //             $(for v in range.clone() =>
                                    //                 $(quoted(&values[v])) => Ok(Self::$(&variants[v])),
                                    //             )
                                    //             _ => Err(format!("Invalid variant: {}", s)),
                                    //         }
                                    //     }
                                    // }
                                    // $['\n']
                                    // impl $import_display for $(&self.name) {
                                    //     fn fmt(&self, f: &mut $import_formatter<'_>) -> std::fmt::Result {
                                    //         let variant = match self {
                                    //             $(for v in range =>
                                    //                 Self::$(&variants[v]) => $(quoted(&values[v])),
                                    //             )
                                    //         };
                                    //         write!(f, "{}", variant)
                                    //     }
                                    // }
                                )
                            }
                            Enumeration::Integer(values) => {
                                quote!(
                                    #[derive(Debug, Clone, PartialEq, Eq, Hash, $import_serialize, $import_deserialize)]
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
                            #[derive(Debug, Clone, PartialEq, Eq, Hash, $import_serialize, $import_deserialize)]
                            pub struct $(&self.name) {
                                $(for property in &self.properties =>
                                    $(property.description.as_ref().map(|description| quote!(#[doc=$(quoted(description))])))
                                    $(if KEYWORDS.contains(&property.name.as_str()) {
                                        #[serde(rename = $(quoted(&property.name)))]
                                        pub $(&format!("r#{}", property.name))
                                    } else {
                                        pub $(&property.name)
                                    }):
                                    $(if property.required { $(&property.ty) } else { Option<$(&property.ty)> }),
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
