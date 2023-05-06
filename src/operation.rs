use crate::{err, generate::*, Error, Model, Resource};
use heck::{ToSnakeCase, ToUpperCamelCase};
use hyper::Method;
use once_cell::sync::OnceCell;
use openapiv3::{PathItem, ReferenceOr, StatusCode::Code};
use std::{collections::HashMap, sync::Mutex};

static OPERATIONS: OnceCell<Mutex<HashMap<String, Operation>>> = OnceCell::new();

#[derive(Clone)]
pub struct Operation {
    pub name: String,
    pub path: String,
    pub method: Method,
    pub description: String,
    pub response: Tokens,
}

impl Operation {
    pub fn all() -> Vec<Operation> {
        OPERATIONS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    fn add(operation: Operation) -> Result<(), Error> {
        let mut operations = OPERATIONS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
        if operations.contains_key(&operation.name) {
            panic!("Operation {} already exists", operation.name);
        }
        Resource::add_operation(&operation.path, &operation.name)?;
        operations.insert(operation.name.clone(), operation);
        Ok(())
    }

    pub fn get(name: &str) -> Option<Operation> {
        OPERATIONS
            .get_or_init(|| Mutex::new(HashMap::new()))
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
        let mut response_name = String::from("()");
        for response in schema.responses.responses.iter() {
            match response {
                (status, response) => {
                    if *status != Code(200) {
                        continue;
                    }
                    response_name = format!("{name}_response").to_upper_camel_case();
                    let schema = match response {
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
                            Model::discover(&response_name, schema)?;
                        }
                    };
                    // let tokens = quote_in! { *; {
                    //     #[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
                    //     pub struct #name {
                    //         #schema
                    //     }
                    // }};
                    // write_tokens(&tokens, &format!("src/operation/{}.rs", name))?;
                }
            }
        }
        Operation::add(Operation {
            name,
            path: path.to_string(),
            method,
            description: schema.description.unwrap_or_default(),
            response: match response_name.as_str() {
                "()" => quote!(()),
                name => {
                    let module = import("super::model", name);
                    quote!($module)
                }
            },
        })?;
        Ok(())
    }
}

// responses:
//         '200':
//           description: Returns a collection of environment resources.
//           content:
//             application/json:
//               schema:
//                 title: EnvironmentListResponse
//                 type: object
//                 required:
//                   - data
//                 properties:
//                   data:
//                     type: array
//                     items:
//                       $ref: '#/components/schemas/Environment'
//                   includes:
//                     type: object
//                     properties:
//                       creators:
//                         $ref: '#/components/schemas/CreatorInclude'
