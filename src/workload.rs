use crate::{err, generate, Error};
use openapiv3::OpenAPI;
use std::{fs::File, io::Read, path::Path};

#[derive(Debug)]
pub struct Workload {
    pub input: String,
    pub output: String,
    pub extension: String,
    pub api: OpenAPI,
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
            input,
            output,
            extension,
            api: schema,
        })
    }

    pub fn generate(&self) -> Result<(), Error> {
        generate(self)
    }
}
