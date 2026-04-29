// Internal port of `@react-native/codegen`. Items here mirror the upstream
// schema and helpers; some variants and fields are kept for upstream parity
// even when not yet exercised by the visitor.
#![allow(dead_code)]
// Enum variant names mirror the upstream Codegen schema (e.g. `Int32TypeAnnotation`).
#![allow(clippy::enum_variant_names)]

pub mod codegen_schema;
pub mod generators;
pub mod parsers;
pub mod schema_validator;

use anyhow::Result;
use swc_ecma_ast::Module;

use crate::codegen::codegen_schema::SchemaType;

/// Parse source code and generate a view config string.
/// Corresponds to `generateViewConfig` in babel-plugin-codegen/index.js
pub fn generate_view_config(
    filename: &str,
    module: &Module,
    command_type_name: Option<&str>,
) -> Result<String> {
    let schema = parse_file(filename, module, command_type_name)?;
    schema_validator::validate(&schema)?;

    let library_name = extract_library_name(filename);
    Ok(generators::generate_view_config_js::generate(
        &library_name,
        &schema,
    ))
}

/// Parse a module to produce a SchemaType.
fn parse_file(
    filename: &str,
    module: &Module,
    command_type_name: Option<&str>,
) -> Result<SchemaType> {
    if filename.ends_with(".js") {
        parsers::flow::build_schema(module, command_type_name)
    } else if filename.ends_with(".ts") || filename.ends_with(".tsx") {
        parsers::typescript::build_schema(module, command_type_name)
    } else {
        anyhow::bail!("Unable to parse file '{filename}'. Unsupported filename extension.",);
    }
}

/// Extract library name from filename.
/// "ModuleNativeComponent.js" → "Module"
fn extract_library_name(filename: &str) -> String {
    let basename = std::path::Path::new(filename)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    // Strip NativeComponent.(js|ts) suffix
    if let Some(stripped) = basename.strip_suffix("NativeComponent.js") {
        stripped.to_string()
    } else if let Some(stripped) = basename.strip_suffix("NativeComponent.ts") {
        stripped.to_string()
    } else if let Some(stripped) = basename.strip_suffix("NativeComponent.tsx") {
        stripped.to_string()
    } else {
        // Fallback: strip extension
        basename
            .rsplit_once('.')
            .map(|(name, _)| name.to_string())
            .unwrap_or(basename)
    }
}
