use crate::{err, generate::*, Error, Operation};
use genco::quote_in;
use heck::ToSnakeCase;
use once_cell::sync::OnceCell;
use openapiv3::{PathItem, ReferenceOr};
use std::{collections::HashMap, sync::Mutex};

static RESOURCES: OnceCell<Mutex<HashMap<String, Resource>>> = OnceCell::new();

#[derive(Clone)]
pub struct Resource {
    pub path: String,
    pub operations: Vec<String>,
}

impl Resource {
    pub fn all() -> Vec<Resource> {
        RESOURCES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }

    pub fn add(resource: Resource) {
        let mut resources = RESOURCES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
        if resources.contains_key(&resource.path) {
            panic!("Resource {} already exists", resource.path);
        }
        resources.insert(resource.path.clone(), resource);
    }

    pub fn add_operation(path: &str, operation: &str) -> Result<(), Error> {
        let mut resources = RESOURCES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap();
        if let Some(resource) = resources.get_mut(path) {
            if resource.operations.contains(&operation.to_string()) {
                return err!(
                    "Operation {} already exists for resource {}",
                    operation,
                    path
                );
            }
            resource.operations.push(operation.to_string());
        } else {
            let resource = Resource {
                path: path.to_string(),
                operations: vec![operation.to_string()],
            };
            resources.insert(path.to_string(), resource);
        }
        Ok(())
    }

    fn get(path: &str) -> Option<Resource> {
        RESOURCES
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap()
            .get(path)
            .cloned()
    }

    pub fn unversioned_path(&self) -> String {
        self.path.replace("/v1/", "")
    }

    pub fn write(&self, dir: &str) -> Result<(), Error> {
        let path = format!("{dir}/resources/{}.rs", self.unversioned_path());
        println!("Writing resource to {}", path);
        let mut tokens = Tokens::new();
        quote_in! { tokens =>
            use super::super::Error;
            use super::super::config::ENDPOINT;
            use hyper::{Client, Uri};
            use hyper_tls::HttpsConnector;
        }
        tokens.line();
        for op in &self.operations {
            let operation = Operation::get(op).unwrap();
            let name = operation.name.to_snake_case();
            let method = operation.method.as_str();
            let path = operation.path.clone();
            let description = operation.description.clone();
            quote_in! { tokens =>
                #[doc = $(quoted(description))]
                pub async fn $name() -> Result<(), Error> {
                    let uri = format!("{ENDPOINT}{}", $(quoted(path))).parse::<Uri>().unwrap();

                    let https = HttpsConnector::new();
                    let client = Client::builder().build::<_, hyper::Body>(https);

                    let req = hyper::Request::builder()
                        .method(hyper::Method::$method)
                        .uri(uri)
                        .header("user-agent", "cycle/1.0.0")
                        .body(hyper::Body::empty())
                        .unwrap();

                    let resp = client.request(req).await?;

                    println!("Response: {:?}", resp);
                    Ok(())
                }
            }
            tokens.line();
        }
        write_tokens(&path, tokens)?;
        Ok(())
    }
}
