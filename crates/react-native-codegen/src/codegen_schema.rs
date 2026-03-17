// Corresponds to `CodegenSchema.js` in react-native/packages/react-native-codegen/src/CodegenSchema.js

use std::collections::BTreeMap;

/// Top-level schema type.
#[derive(Debug, Clone)]
pub struct SchemaType {
    pub modules: BTreeMap<String, ModuleSchema>,
}

#[derive(Debug, Clone)]
pub enum ModuleSchema {
    Component(ComponentModule),
}

#[derive(Debug, Clone)]
pub struct ComponentModule {
    pub components: BTreeMap<String, ComponentShape>,
}

#[derive(Debug, Clone)]
pub struct ComponentShape {
    pub extends_props: Vec<ExtendsPropsShape>,
    pub events: Vec<EventTypeShape>,
    pub props: Vec<NamedShape<PropTypeAnnotation>>,
    pub commands: Vec<NamedShape<CommandTypeAnnotation>>,
    pub paper_component_name: Option<String>,
    pub paper_component_name_deprecated: Option<String>,
    pub interface_only: Option<bool>,
    pub excluded_platforms: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct ExtendsPropsShape {
    pub type_: ExtendsPropsType,
    pub known_type_name: String,
}

#[derive(Debug, Clone)]
pub enum ExtendsPropsType {
    ReactNativeBuiltInType,
}

#[derive(Debug, Clone)]
pub struct EventTypeShape {
    pub name: String,
    pub bubbling_type: BubblingType,
    pub optional: bool,
    pub paper_top_level_name_deprecated: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BubblingType {
    Direct,
    Bubble,
}

#[derive(Debug, Clone)]
pub struct NamedShape<T> {
    pub name: String,
    pub optional: bool,
    pub type_annotation: T,
}

#[derive(Debug, Clone)]
pub enum PropTypeAnnotation {
    BooleanTypeAnnotation,
    StringTypeAnnotation,
    Int32TypeAnnotation,
    DoubleTypeAnnotation,
    FloatTypeAnnotation,
    ObjectTypeAnnotation,
    StringEnumTypeAnnotation,
    Int32EnumTypeAnnotation,
    MixedTypeAnnotation,
    ReservedPropTypeAnnotation {
        name: String,
    },
    ArrayTypeAnnotation {
        element_type: Box<PropTypeAnnotation>,
    },
}

#[derive(Debug, Clone)]
pub struct CommandTypeAnnotation {
    pub params: Vec<CommandParam>,
}

#[derive(Debug, Clone)]
pub struct CommandParam {
    pub name: String,
}

/// Options extracted from the second argument of `codegenNativeComponent()`.
#[derive(Debug, Clone, Default)]
pub struct OptionsShape {
    pub interface_only: Option<bool>,
    pub paper_component_name: Option<String>,
    pub paper_component_name_deprecated: Option<String>,
    pub excluded_platforms: Option<Vec<String>>,
}
