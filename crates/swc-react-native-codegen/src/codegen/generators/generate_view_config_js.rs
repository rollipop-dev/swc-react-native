// Corresponds to `GenerateViewConfigJs.js` in
// react-native/packages/react-native-codegen/src/generators/components/GenerateViewConfigJs.js

use std::collections::BTreeSet;

use crate::codegen::codegen_schema::*;

/// Generate JavaScript view config code from a schema.
/// Returns the generated JS code as a string.
// Corresponds to `generate` in GenerateViewConfigJs.js
pub fn generate(_library_name: &str, schema: &SchemaType) -> String {
    let mut imports: BTreeSet<String> = BTreeSet::new();
    let mut component_configs = Vec::new();

    for module in schema.modules.values() {
        match module {
            ModuleSchema::Component(component_module) => {
                for (component_name, component) in &component_module.components {
                    if component.paper_component_name_deprecated.is_some() {
                        imports.insert("const {UIManager} = require(\"react-native\")".to_string());
                    }

                    let config = build_component_config(component_name, component, &mut imports);
                    component_configs.push(config);
                }
            }
            ModuleSchema::NativeModule(_) => {}
        }
    }

    let imports_str = imports.into_iter().collect::<Vec<_>>().join("\n");

    format!("{}\n{}", imports_str, component_configs.join("\n\n"))
}

fn build_component_config(
    component_name: &str,
    component: &ComponentShape,
    imports: &mut BTreeSet<String>,
) -> String {
    // Add NativeComponentRegistry import
    for ext in &component.extends_props {
        match &ext.type_ {
            ExtendsPropsType::ReactNativeBuiltInType => {
                if ext.known_type_name == "ReactNativeCoreViewProps" {
                    imports.insert(
                        "const NativeComponentRegistry = require('react-native/Libraries/NativeComponent/NativeComponentRegistry');".to_string(),
                    );
                }
            }
        }
    }

    let paper_component_name = component
        .paper_component_name
        .as_deref()
        .unwrap_or(component_name);

    // Build view config object
    let view_config = build_view_config(paper_component_name, component, imports);

    // Build the component template
    let mut output = String::new();

    output.push_str(&format!(
        "let nativeComponentName = '{paper_component_name}';\n",
    ));

    // Handle deprecated component name
    if let Some(deprecated) = &component.paper_component_name_deprecated {
        output.push_str(&format!(
            "if (UIManager.hasViewManagerConfig('{component_name}')) {{\n  nativeComponentName = '{component_name}';\n}} else if (UIManager.hasViewManagerConfig('{deprecated}')) {{\n  nativeComponentName = '{deprecated}';\n}} else {{\n  throw new Error('Failed to find native component for either \"{component_name}\" or \"{deprecated}\"');\n}}\n",
        ));
    }

    output.push_str(&format!(
        "export const __INTERNAL_VIEW_CONFIG = {view_config};\n",
    ));
    output.push_str(
        "export default NativeComponentRegistry.get(nativeComponentName, () => __INTERNAL_VIEW_CONFIG);",
    );

    // Build commands export
    if let Some(commands_export) = build_commands(component, imports) {
        output.push('\n');
        output.push_str(&commands_export);
    }

    output
}

fn build_view_config(
    component_name: &str,
    component: &ComponentShape,
    imports: &mut BTreeSet<String>,
) -> String {
    let mut properties = Vec::new();

    // uiViewClassName
    properties.push(format!("  uiViewClassName: \"{component_name}\""));

    // bubblingEventTypes
    let bubbling_events: Vec<&EventTypeShape> = component
        .events
        .iter()
        .filter(|e| e.bubbling_type == BubblingType::Bubble)
        .collect();

    if !bubbling_events.is_empty() {
        let events_str = bubbling_events
            .iter()
            .map(|event| {
                let event_name = normalize_input_event_name(
                    event
                        .paper_top_level_name_deprecated
                        .as_deref()
                        .unwrap_or(&event.name),
                );
                format!(
                    "    {}: {{\n      phasedRegistrationNames: {{\n        captured: \"{}Capture\",\n        bubbled: \"{}\"\n      }}\n    }}",
                    event_name,
                    event.name,
                    event.name,
                )
            })
            .collect::<Vec<_>>()
            .join(",\n");
        properties.push(format!("  bubblingEventTypes: {{\n{events_str}\n  }}"));
    }

    // directEventTypes
    let direct_events: Vec<&EventTypeShape> = component
        .events
        .iter()
        .filter(|e| e.bubbling_type == BubblingType::Direct)
        .collect();

    if !direct_events.is_empty() {
        let events_str = direct_events
            .iter()
            .map(|event| {
                let event_name = normalize_input_event_name(
                    event
                        .paper_top_level_name_deprecated
                        .as_deref()
                        .unwrap_or(&event.name),
                );
                format!(
                    "    {}: {{\n      registrationName: \"{}\"\n    }}",
                    event_name, event.name,
                )
            })
            .collect::<Vec<_>>()
            .join(",\n");
        properties.push(format!("  directEventTypes: {{\n{events_str}\n  }}"));
    }

    // validAttributes
    let valid_attrs = build_valid_attributes(component, imports);
    properties.push(format!("  validAttributes: {valid_attrs}"));

    format!("{{\n{}\n}}", properties.join(",\n"))
}

fn build_valid_attributes(component: &ComponentShape, imports: &mut BTreeSet<String>) -> String {
    let mut attrs = Vec::new();

    // Props
    for prop in &component.props {
        let value = get_react_diff_process_value(&prop.type_annotation);
        attrs.push(format!("    {}: {}", prop.name, value));
    }

    // Events
    if !component.events.is_empty() {
        imports.insert(
            "const {ConditionallyIgnoredEventHandlers} = require('react-native/Libraries/NativeComponent/ViewConfigIgnore');".to_string(),
        );

        let event_attrs = component
            .events
            .iter()
            .map(|e| format!("      {}: true", e.name))
            .collect::<Vec<_>>()
            .join(",\n");

        attrs.push(format!(
            "    ...ConditionallyIgnoredEventHandlers({{\n{event_attrs}\n    }})",
        ));
    }

    if attrs.is_empty() {
        "{}".to_string()
    } else {
        format!("{{\n{}\n  }}", attrs.join(",\n"))
    }
}

// Corresponds to `getReactDiffProcessValue` in GenerateViewConfigJs.js
fn get_react_diff_process_value(type_annotation: &PropTypeAnnotation) -> String {
    match type_annotation {
        PropTypeAnnotation::BooleanTypeAnnotation
        | PropTypeAnnotation::StringTypeAnnotation
        | PropTypeAnnotation::Int32TypeAnnotation
        | PropTypeAnnotation::DoubleTypeAnnotation
        | PropTypeAnnotation::FloatTypeAnnotation
        | PropTypeAnnotation::ObjectTypeAnnotation
        | PropTypeAnnotation::StringEnumTypeAnnotation
        | PropTypeAnnotation::Int32EnumTypeAnnotation
        | PropTypeAnnotation::MixedTypeAnnotation => "true".to_string(),

        PropTypeAnnotation::ReservedPropTypeAnnotation { name } => match name.as_str() {
            "ColorPrimitive" => {
                "require('react-native/Libraries/Components/View/ReactNativeStyleAttributes').colorAttribute".to_string()
            }
            "ImageSourcePrimitive" => {
                "{ process: ((req) => 'default' in req ? req.default : req)(require('react-native/Libraries/Image/resolveAssetSource')) }".to_string()
            }
            "PointPrimitive" => {
                "{ diff: ((req) => 'default' in req ? req.default : req)(require('react-native/Libraries/Utilities/differ/pointsDiffer')) }".to_string()
            }
            "EdgeInsetsPrimitive" => {
                "{ diff: ((req) => 'default' in req ? req.default : req)(require('react-native/Libraries/Utilities/differ/insetsDiffer')) }".to_string()
            }
            "DimensionPrimitive" => "true".to_string(),
            _ => "true".to_string(),
        },

        PropTypeAnnotation::ArrayTypeAnnotation { element_type } => match &**element_type {
            PropTypeAnnotation::ReservedPropTypeAnnotation { name } if name == "ColorPrimitive" => {
                "{ process: ((req) => 'default' in req ? req.default : req)(require('react-native/Libraries/StyleSheet/processColorArray')) }".to_string()
            }
            _ => "true".to_string(),
        },
    }
}

// Corresponds to `normalizeInputEventName` in GenerateViewConfigJs.js
fn normalize_input_event_name(name: &str) -> String {
    if let Some(stripped) = name.strip_prefix("on") {
        format!("top{stripped}")
    } else if !name.starts_with("top") {
        let mut chars = name.chars();
        match chars.next() {
            Some(c) => format!("top{}{}", c.to_uppercase(), chars.as_str()),
            None => "top".to_string(),
        }
    } else {
        name.to_string()
    }
}

fn build_commands(component: &ComponentShape, imports: &mut BTreeSet<String>) -> Option<String> {
    if component.commands.is_empty() {
        return None;
    }

    imports.insert(
        "const {dispatchCommand} = require(\"react-native/Libraries/ReactNative/RendererProxy\");"
            .to_string(),
    );

    let methods = component
        .commands
        .iter()
        .map(|cmd| {
            let param_names: Vec<&str> = cmd
                .type_annotation
                .params
                .iter()
                .map(|p| p.name.as_str())
                .collect();
            let all_params = std::iter::once("ref")
                .chain(param_names.iter().copied())
                .collect::<Vec<_>>()
                .join(", ");
            let args_array = if param_names.is_empty() {
                "[]".to_string()
            } else {
                format!("[{}]", param_names.join(", "))
            };
            format!(
                "  {}({}) {{\n    dispatchCommand(ref, \"{}\", {});\n  }}",
                cmd.name, all_params, cmd.name, args_array
            )
        })
        .collect::<Vec<_>>()
        .join(",\n");

    Some(format!("export const Commands = {{\n{methods}\n}};"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_input_event_name() {
        assert_eq!(
            normalize_input_event_name("onDirectEventDefinedInlineNull"),
            "topDirectEventDefinedInlineNull"
        );
        assert_eq!(
            normalize_input_event_name("onBubblingEventDefinedInlineNull"),
            "topBubblingEventDefinedInlineNull"
        );
        assert_eq!(normalize_input_event_name("topAlready"), "topAlready");
    }
}
