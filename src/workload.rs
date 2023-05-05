use crate::{err, generate::*, Error, Model, Operation, Resource};
use genco::quote_in;
use heck::ToSnakeCase;
use openapiv3::{OpenAPI, ReferenceOr};
use std::{fs::File, io::Read, path::Path, process::Command};

pub struct Workload {
    schema: OpenAPI,
    output: String,
}

impl Workload {
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
        // if !Path::new(&output).exists() {
        //     match std::fs::create_dir(&output) {
        //         Ok(_) => {
        //             std::fs::create_dir(&format!("{output}/models")).unwrap();
        //             std::fs::create_dir(&format!("{output}/operations")).unwrap();
        //         }
        //         Err(e) => {
        //             return err!("Error: could not create directory {}: {}", output, e);
        //         }
        //     }
        // }
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
        self.write_config()?;
        // for (name, schema) in self.schema.components.as_ref().unwrap().schemas.iter() {
        //     let schema = schema.as_item().unwrap();
        //     Model::discover(name, schema)?;
        // }
        // self.write_models()?;
        for (path, schema) in self.schema.paths.iter() {
            Operation::discover_all_from_path(path, schema)?;
            break;
        }
        self.write_resources()?;
        self.format()?;
        Ok(())
    }

    pub fn write_models(&self) -> Result<(), Error> {
        let models = Model::all();
        write_tokens(
            &format!("{}/models/mod.rs", self.output),
            quote!(
                $(for model in &models => pub mod $(&model.path);)
                $['\n']
                $(for model in &models => pub use $(&model.path)::$(&model.name);)
            ),
        )?;
        // write_tokens(
        //     &format!("{}/mod.rs", self.output),
        //     quote!(
        //         pub mod models;
        //     ),
        // )?;
        for model in models {
            model.write(&self.output)?;
        }
        Ok(())
    }

    pub fn write_resources(&self) -> Result<(), Error> {
        let path = format!("{dir}/api.rs", dir = self.output);
        let mut tokens = Tokens::new();
        let mut resource_tokens = Tokens::new();
        quote_in! { tokens =>
            use super::Error;
            use super::config::ENDPOINT;
            use hyper::{Client, Uri, client::HttpConnector};
            use hyper_tls::HttpsConnector;
        }
        tokens.line();
        for resource in Resource::all() {
            for op in &resource.operations {
                let operation = Operation::get(op).unwrap();
                let name = operation.name.to_snake_case();
                let method = operation.method.as_str();
                let path = operation.path.clone();
                let description = operation.description.clone();
                quote_in! { resource_tokens =>
                    #[doc = $(quoted(description))]
                    pub async fn $name(&self) -> Result<String, Error> {
                        $(match method {
                            "GET" => self.get($(quoted(path))).await,
                            _ => unimplemented!(),
                        })
                    }
                }
                resource_tokens.line();
            }
        }
        quote_in!(tokens =>
            pub struct Api {
                token: String,
                client: Client<HttpsConnector<HttpConnector>>,
            }
            $['\n']
            impl Api {
                pub fn new<S: AsRef<str>>(token: S) -> Self {
                    let https = HttpsConnector::new();
                    let client = Client::builder().build::<_, hyper::Body>(https);
                    Self {
                        token: token.as_ref().to_string(),
                        client,
                    }
                }
                $['\n']
                pub async fn request(&self, method: hyper::Method, path: &str) -> Result<String, Error> {
                    let uri = format!("{ENDPOINT}{path}").parse::<Uri>().unwrap();

                    let request = hyper::Request::builder()
                        .method(method)
                        .uri(uri)
                        .header("user-agent", "cycle/1.0.0")
                        .header("authorization", format!("Bearer {}", self.token))
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
                    Ok(String::from_utf8_lossy(&body).to_string())
                }
                $['\n']
                pub async fn get(&self, path: &str) -> Result<String, Error> {
                    self.request(hyper::Method::GET, path).await
                }
                $['\n']
                $resource_tokens
            }
        );
        write_tokens(&path, tokens)?;
        // write_tokens(
        //     &format!("{}/resources/mod.rs", self.output),
        //     quote!(
        //         $(for resource in &resources => pub mod $(resource.unversioned_path());)
        //         $['\n']
        //         $(for resource in &resources => pub use $(resource.unversioned_path())::*;)
        //     ),
        // )?;
        // for resource in resources {
        //     resource.write(&self.output)?;
        // }
        Ok(())
    }

    pub fn write_config(&self) -> Result<(), Error> {
        // println!("{:#?}", self.schema.servers);
        write_tokens(
            &format!("{}/config.rs", self.output),
            quote!(
                pub const VERSION: &str = $(quoted(&self.schema.info.version));
                pub const ENDPOINT: &str = $(quoted(&self.schema.servers[0].url));
            ),
        )?;
        Ok(())
    }

    fn format(&self) -> Result<(), Error> {
        println!("Formatting...");
        let output = Command::new("bash")
            .args([
                "-c",
                &format!(
                    "find {} -type f | xargs rustfmt --edition 2021",
                    self.output
                ),
            ])
            .output()?;
        if !output.status.success() {
            return err!(
                "Error: could not format generated code: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        println!("{}", String::from_utf8_lossy(&output.stdout));
        Ok(())
    }
}
