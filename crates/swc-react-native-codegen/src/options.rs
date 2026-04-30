/// Configuration for the codegen transform.
#[derive(Debug, Default, Clone)]
pub struct CodegenOptions {
    /// Filename of the file being transformed (used for error reporting and
    /// as the source key for parsing the generated view config).
    pub filename: String,
}
