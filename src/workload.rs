use openapiv3::OpenAPI;

#[derive(Debug)]
pub struct Workload {
    pub input: String,
    pub output: String,
    pub extension: String,
    pub api: OpenAPI,
}
