// Corresponds to `SchemaValidator.js` in react-native/packages/react-native-codegen/src/SchemaValidator.js

use std::collections::HashMap;

use anyhow::{bail, Result};

use crate::codegen::codegen_schema::{ModuleSchema, SchemaType};

pub fn validate(schema: &SchemaType) -> Result<()> {
    let errors = get_errors(schema);
    if errors.is_empty() {
        Ok(())
    } else {
        bail!("Errors found validating schema:\n{}", errors.join("\n"));
    }
}

fn get_errors(schema: &SchemaType) -> Vec<String> {
    let mut errors = Vec::new();
    // Track component names to detect duplicates across modules
    let mut component_modules: HashMap<String, Vec<String>> = HashMap::new();

    for (module_name, module) in &schema.modules {
        match module {
            ModuleSchema::Component(component_module) => {
                for component_name in component_module.components.keys() {
                    component_modules
                        .entry(component_name.clone())
                        .or_default()
                        .push(module_name.clone());
                }
            }
            ModuleSchema::NativeModule(_) => {}
        }
    }

    for (component_name, modules) in &component_modules {
        if modules.len() > 1 {
            errors.push(format!(
                "Duplicate component name '{}' found in modules: {}",
                component_name,
                modules.join(", ")
            ));
        }
    }

    errors.sort();
    errors
}
