use crate::{err, generate::*, Error, Model};
use openapiv3::OpenAPI;
use std::{fs::File, io::Read, path::Path, process::Command};

pub struct Workload {
    output: String,
    api: OpenAPI,
}

impl Workload {
    pub fn new<S: AsRef<str>>(input: S, output: S) -> Result<Self, Error> {
        let input = input.as_ref().to_string();
        let output = output.as_ref().to_string();
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
        if !Path::new(&output).exists() {
            match std::fs::create_dir(&output) {
                Ok(_) => {
                    std::fs::create_dir(&format!("{output}/models")).unwrap();
                }
                Err(e) => {
                    return err!("Error: could not create directory {}: {}", output, e);
                }
            }
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
            output,
            api: schema,
        })
    }

    pub fn generate(&self) -> Result<(), Error> {
        for (name, schema) in self.api.components.as_ref().unwrap().schemas.iter() {
            let schema = schema.as_item().unwrap();
            eprintln!("{name}: {schema:#?}");
            Model::discover(name, schema)?;
            // generate_model(workload, name, schema)?;
            if name == "AccountState" {
                break;
            }
        }
        self.write_models()?;
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
        write_tokens(
            &format!("{}/mod.rs", self.output),
            quote!(
                pub mod models;
            ),
        )?;
        for model in models {
            model.write(&self.output)?;
        }
        Ok(())
    }

    fn format(&self) -> Result<(), Error> {
        Command::new("bash")
            .args(["-c", &format!("rustfmt {}/**/*.rs", self.output)])
            .status()?;
        Ok(())
    }
}
