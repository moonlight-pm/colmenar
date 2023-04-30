use crate::{err, generate::*, Error, Model, Operation};
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
        if !Path::new(&output).exists() {
            match std::fs::create_dir(&output) {
                Ok(_) => {
                    std::fs::create_dir(&format!("{output}/models")).unwrap();
                    std::fs::create_dir(&format!("{output}/operations")).unwrap();
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
            schema,
            output: output.to_string(),
        })
    }

    pub fn generate(&self) -> Result<(), Error> {
        // for (name, schema) in self.schema.components.as_ref().unwrap().schemas.iter() {
        //     let schema = schema.as_item().unwrap();
        //     Model::discover(name, schema)?;
        // }
        // self.write_models()?;
        for (path, schema) in self.schema.paths.iter() {
            Operation::discover_all_from_path(path, schema)?;
            break;
        }
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
