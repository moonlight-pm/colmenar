use crate::{err, generate::*, Error, Model, Operation, Resource};
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
        for (name, schema) in self.schema.components.as_ref().unwrap().schemas.iter() {
            let schema = schema.as_item().unwrap();
            Model::discover(name, schema)?;
        }
        for (path, schema) in self.schema.paths.iter() {
            if path == "/v1/environments" {
                Operation::discover_all_from_path(path, schema)?;
                break;
            }
        }
        self.write()?;
        Ok(())
    }

    pub fn write(&self) -> Result<(), Error> {
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
                use hyper::{Client, Uri, client::HttpConnector};
                use hyper_tls::HttpsConnector;
                use serde_json::Value;
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
                    pub async fn request(&self, method: hyper::Method, path: &str) -> Result<Value, Error> {
                        let uri = format!("{}{path}", self.endpoint).parse::<Uri>().unwrap();

                        let request = hyper::Request::builder()
                            .method(method)
                            .uri(uri)
                            .header("user-agent", "cycle/1.0.0")
                            .header("authorization", format!("Bearer {}", self.token))
                            .header("x-hub-id", &self.hub)
                            .body(hyper::Body::empty())
                            .unwrap();

                        let response = match self.client.request(request).await {
                            Ok(resp) => resp,
                            Err(e) => {
                                println!("Error: {e:#?}");
                                return Err(Error::from(e));
                            }
                        };

                        println!("Response: {:?}", response);
                        let body = hyper::body::to_bytes(response.into_body()).await?;
                        Ok(serde_json::from_slice(&body)?)
                    }
                    $['\n']
                    pub async fn get(&self, path: &str) -> Result<Value, Error> {
                        self.request(hyper::Method::GET, path).await
                    }
                    $['\n']
                    $(for resource in Resource::all() =>
                        $(resource.tokens()?)
                    )
                }
            ),
        )?;
        format(&self.output)?;
        Ok(())
    }
}
