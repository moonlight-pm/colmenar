use crate::prelude::*;
use heck::{ToSnakeCase, ToUpperCamelCase};
use hyper::Method;
use once_cell::sync::OnceCell;
use openapiv3::{PathItem, ReferenceOr, StatusCode::Code};
use std::{collections::BTreeMap, sync::Mutex};

static OPERATIONS: OnceCell<Mutex<BTreeMap<String, Operation>>> = OnceCell::new();

#[derive(Clone)]
pub struct Operation {
    pub name: String,
    pub path: String,
    pub method: Method,
    pub description: String,
    pub parameters: Vec<Parameter>,
    pub query: Vec<Parameter>,
    pub request: Option<Tokens>,
    pub response: Option<Tokens>,
}

impl Operation {
    pub fn all() -> Vec<Operation> {
        OPERATIONS
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    fn add(operation: Operation) -> Result<(), Error> {
        let mut operations = OPERATIONS
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .unwrap();
        if operations.contains_key(&operation.name) {
            panic!("Operation {} already exists", operation.name);
        }
        operations.insert(operation.name.clone(), operation);
        Ok(())
    }

    pub fn get(name: &str) -> Option<Operation> {
        OPERATIONS
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .unwrap()
            .get(name)
            .cloned()
    }

    pub fn discover_all_from_path(path: &str, schema: &ReferenceOr<PathItem>) -> Result<(), Error> {
        match schema {
            ReferenceOr::Reference { reference, .. } => {
                return err!("References not implemented: {}", reference);
            }
            ReferenceOr::Item(item) => {
                if let Some(op) = &item.get {
                    Self::discover(path, Method::GET, op.clone())?;
                }
                if let Some(op) = &item.put {
                    Self::discover(path, Method::PUT, op.clone())?;
                }
                if let Some(op) = &item.post {
                    Self::discover(path, Method::POST, op.clone())?;
                }
                if let Some(op) = &item.delete {
                    Self::discover(path, Method::DELETE, op.clone())?;
                }
                if let Some(op) = &item.options {
                    Self::discover(path, Method::OPTIONS, op.clone())?;
                }
                if let Some(op) = &item.head {
                    Self::discover(path, Method::HEAD, op.clone())?;
                }
                if let Some(op) = &item.patch {
                    Self::discover(path, Method::PATCH, op.clone())?;
                }
                if let Some(op) = &item.trace {
                    Self::discover(path, Method::TRACE, op.clone())?;
                }
            }
        }
        Ok(())
    }

    pub fn discover(path: &str, method: Method, schema: openapiv3::Operation) -> Result<(), Error> {
        let name = match schema.operation_id {
            Some(name) => name.to_snake_case(),
            None => {
                return err!("Operation is missing operationId: {}", path);
            }
        };
        let mut parameters = Vec::new();
        let mut query = Vec::new();
        for item in schema.parameters {
            match item {
                ReferenceOr::Reference { reference, .. } => {
                    let reference = reference.split('/').last().unwrap();
                    let parameter = Parameter::get(&reference).unwrap();
                    query.push(parameter);
                }
                ReferenceOr::Item(item) => match item {
                    openapiv3::Parameter::Path { parameter_data, .. } => {
                        let type_name =
                            format!("{name}_{}", parameter_data.name).to_upper_camel_case();
                        parameters.push(Parameter::discover(&type_name, parameter_data)?);
                    }
                    openapiv3::Parameter::Query { parameter_data, .. } => {
                        let type_name =
                            format!("{name}_{}", parameter_data.name).to_upper_camel_case();
                        query.push(Parameter::discover(&type_name, parameter_data)?);
                    }
                    _ => {
                        return err!("Unsupported parameter type: {item:?}");
                    }
                },
            }
        }
        let mut request = None;
        if let Some(item) = schema.request_body.as_ref() {
            match item {
                ReferenceOr::Reference { .. } => {
                    unimplemented!();
                }
                ReferenceOr::Item(item) => {
                    if item.content.get("application/json").is_none() {
                        return err!("Request is missing application/json content type: {path}");
                    }
                    let schema = match item
                        .content
                        .get("application/json")
                        .unwrap()
                        .schema
                        .as_ref()
                        .unwrap()
                    {
                        ReferenceOr::Reference { reference, .. } => {
                            return err!("References not implemented: {}", reference);
                        }
                        ReferenceOr::Item(item) => item,
                    };
                    let name = format!("{name}_request").to_upper_camel_case();
                    let module = import("super", &name);
                    Model::discover(&name, schema)?;
                    request = Some(quote!($module));
                }
            }
        }
        let mut response = None;
        for item in schema.responses.responses.iter() {
            match item {
                (status, item) => {
                    if *status != Code(200) {
                        continue;
                    }
                    match item {
                        ReferenceOr::Reference { reference, .. } => {
                            return err!("References not implemented: {}", reference);
                        }
                        ReferenceOr::Item(item) => {
                            if item.content.get("application/json").is_none() {
                                return err!(
                                    "Response is missing application/json content type: {path}"
                                );
                            }
                            let schema = match item
                                .content
                                .get("application/json")
                                .unwrap()
                                .schema
                                .as_ref()
                                .unwrap()
                            {
                                ReferenceOr::Reference { reference, .. } => {
                                    return err!("References not implemented: {}", reference);
                                }
                                ReferenceOr::Item(item) => item,
                            };
                            let name = format!("{name}_response").to_upper_camel_case();
                            let module = import("super", &name);
                            Model::discover(&name, schema)?;
                            response = Some(quote!($module));
                        }
                    };
                }
            }
        }
        Operation::add(Operation {
            name,
            path: path.to_string(),
            method,
            description: schema.description.unwrap_or_default(),
            parameters,
            query,
            request,
            response,
        })?;
        Ok(())
    }

    pub fn tokens(&self) -> Result<Tokens, Error> {
        // let response_type = self.response.clone().unwrap_or(quote!(Value));
        Ok(quote!(
            #[doc = $(quoted(&self.description))]
            pub async fn $(self.name.to_snake_case())(&self
                $(for parameter in &self.parameters {
                    , $(&parameter.name):
                    $(if parameter.required {
                        $(&parameter.ty)
                    } else {
                        Option<$(&parameter.ty)>
                    })
                })
                $(for parameter in &self.query {
                    , $(&parameter.name):
                    $(if parameter.required {
                        $(&parameter.ty)
                    } else {
                        Option<$(&parameter.ty)>
                    })
                })
                $(if self.request.is_some() {
                    , body: $(self.request.as_ref().unwrap())
                })
            ) -> Result<$(self.response.as_ref().unwrap_or(&quote!(()))), Error> {
                let
                $(if !self.parameters.is_empty() || !self.query.is_empty() {
                   mut
                })
                path = String::from($(quoted(&self.path)));
                $(for parameter in &self.parameters {
                    path = path.replace($(quoted(quote!({$(&parameter.original_name)}))), &$(&parameter.name));
                })
                $(if !self.query.is_empty() {
                    let mut query = Value::Object(serde_json::Map::new());
                    $(for parameter in &self.query {
                        $(if parameter.required {
                            resolve(&mut query, $(quoted(&parameter.original_name)), Some($(&parameter.name)));
                        } else {
                            resolve(&mut query, $(quoted(&parameter.original_name)), $(&parameter.name));
                        })
                    })
                    let query = serde_urlencoded::to_string(&query)?;
                    if !query.is_empty() {
                        path = format!("{path}?{query}");
                    }
                })
                $(if self.response.is_some() {
                    let response =
                })
                $(match self.method {
                    Method::GET => {
                        self.request::<_, Value, $(if self.response.is_some() { _ } else { Value })>(Method::GET, path, None).await?;
                    },
                    Method::PUT => {
                        self.request::<_, _, $(if self.response.is_some() { _ } else { Value })>(Method::PUT, path, Some(body)).await?;
                    }
                    Method::POST => {
                        self.request::<_, _, $(if self.response.is_some() { _ } else { Value })>(Method::POST, path, Some(body)).await?;
                    }
                    Method::DELETE => {
                        self.request::<_, Value, $(if self.response.is_some() { _ } else { Value })>(Method::DELETE, path, None).await?;
                    }
                    Method::PATCH => {
                        self.request::<_, _, $(if self.response.is_some() { _ } else { Value })>(Method::PATCH, path, Some(body)).await?;
                    }
                    _ => {
                        None;
                    },
                })
                $(if self.response.is_some() {
                    Ok(response.unwrap())
                } else {
                    Ok(())
                })
            }
        ))
    }
}
