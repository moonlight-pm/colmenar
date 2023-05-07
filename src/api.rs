use crate::prelude::*;
use openapiv3::OpenAPI;
use std::{fs::File, io::Read, path::Path};

pub struct Api {
    schema: OpenAPI,
    output: String,
}

impl Api {
    pub fn new(input: &str, output: &str) -> Result<Self, Error> {
        if !std::path::Path::new(&input).exists() {
            return err!("Error: file does not exist: {}", input);
        }
        let mut file = match File::open(&input) {
            Ok(f) => f,
            Err(e) => {
                return err!("Error: could not open file {}: {}", &input, e);
            }
        };
        let mut source = String::new();
        if let Err(e) = file.read_to_string(&mut source) {
            return err!("Error: could not read file {}: {}", &input, e);
        }
        let extension = Path::new(&input)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();
        let schema = match extension.as_str() {
            "json" => match serde_json::from_str(&source) {
                Ok(e) => e,
                Err(e) => {
                    return err!("Error: could not parse JSON in {}: {}", input, e);
                }
            },
            "yml" | "yaml" => match serde_yaml::from_str(&source) {
                Ok(e) => e,
                Err(e) => {
                    return err!("Error: could not parse YAML in {}: {}", input, e);
                }
            },
            _ => {
                return err!("Error: unsupported file type for {}: {}", input, extension);
            }
        };
        Ok(Self {
            schema,
            output: output.to_string(),
        })
    }

    pub fn generate(&self) -> Result<(), Error> {
        for (name, schema) in self.schema.components.as_ref().unwrap().parameters.iter() {
            let data = schema.as_item().unwrap().clone().parameter_data();
            Parameter::discover(name, data)?;
        }
        for (name, schema) in self.schema.components.as_ref().unwrap().schemas.iter() {
            let schema = schema.as_item().unwrap();
            Model::discover(name, schema)?;
        }
        for (path, schema) in self.schema.paths.iter() {
            if path.starts_with("/v1/environments") {
                Operation::discover_all_from_path(path, schema)?;
            }
        }
        self.write()?;
        Ok(())
    }

    pub fn write(&self) -> Result<(), Error> {
        write_tokens(
            &format!("{}/mod.rs", self.output),
            quote!(
                pub mod api;
                pub mod model;
                $['\n']
                pub use api::Api;
                pub use model::*;
                $['\n']
                pub type Error = Box<dyn std::error::Error + Send + Sync>;
            ),
        )?;
        write_tokens(
            &format!("{}/model.rs", self.output),
            quote!(
                $(for model in Model::all() =>
                    $(model.tokens()?)
                    $['\n']
                )
            ),
        )?;
        write_tokens(
            &format!("{}/api.rs", self.output),
            quote!(
                use super::Error;
                use hyper::{Client, Uri, client::HttpConnector, Method};
                use hyper_tls::HttpsConnector;
                use serde_json::Value;
                use serde::Serialize;
                $['\n']
                pub fn resolve<S: Serialize>(object: &mut Value, name: &str, value: Option<S>) {
                    if let Some(value) = value {
                        let value = serde_json::to_value(value).unwrap();
                        object[name] = if let serde_json::Value::Array(value) = value {
                            serde_json::Value::String(
                                value
                                    .iter()
                                    .map(|v| v.as_str().unwrap())
                                    .collect::<Vec<_>>()
                                    .join(","),
                            )
                        } else {
                            value
                        };
                    }
                }
                $['\n']
                pub struct Api {
                    version: String,
                    endpoint: String,
                    token: String,
                    hub: String,
                    client: Client<HttpsConnector<HttpConnector>>,
                }
                $['\n']
                impl Api {
                    pub fn new<S: AsRef<str>>(token: S, hub: S) -> Self {
                        let https = HttpsConnector::new();
                        let client = Client::builder().build::<_, hyper::Body>(https);
                        Self {
                            version: $(quoted(&self.schema.info.version)).to_string(),
                            endpoint: $(quoted(&self.schema.servers[0].url)).to_string(),
                            token: token.as_ref().to_string(),
                            hub: hub.as_ref().to_string(),
                            client,
                        }
                    }
                    $['\n']
                    pub async fn request<S: AsRef<str>>(&self, method: Method, path: S, body: Option<Value>) -> Result<Option<Value>, Error> {
                        let path = path.as_ref();
                        let uri = format!("{}{path}", self.endpoint).parse::<Uri>().unwrap();
                        println!("Request: {} {}", method, uri);
                        let request = hyper::Request::builder()
                            .method(method)
                            .uri(uri)
                            .header("user-agent", "cycle/1.0.0")
                            .header("authorization", format!("Bearer {}", self.token))
                            .header("x-hub-id", &self.hub)
                            .body(match body {
                                Some(body) => hyper::Body::from(serde_json::to_string(&body)?),
                                None => hyper::Body::empty(),
                            })
                            .unwrap();
                        let response = match self.client.request(request).await {
                            Ok(resp) => resp,
                            Err(e) => {
                                println!("Error: {e:#?}");
                                return Err(Error::from(e));
                            }
                        };
                        // println!("Response: {:?}", response);
                        let body = hyper::body::to_bytes(response.into_body()).await?;
                        // println!("Body: {:?}", String::from_utf8_lossy(&body));
                        Ok(match serde_json::from_slice(&body) {
                            Ok(v) => Some(v),
                            Err(e) => None,
                        })
                    }
                    $['\n']
                    $(for operation in Operation::all() =>
                        $(operation.tokens()?)
                        $['\n']
                    )
                }
            ),
        )?;
        format(&self.output)?;
        Ok(())
    }
}
