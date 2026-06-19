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
    NativeModule(NativeModuleSchema),
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

#[derive(Debug, Clone)]
pub struct NativeModuleSchema {
    pub alias_map: BTreeMap<String, NativeModuleTypeAnnotation>,
    pub enum_map: BTreeMap<String, NativeModuleEnumDeclarationWithMembers>,
    pub spec: NativeModuleSpec,
    pub module_name: String,
    pub excluded_platforms: Option<Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct NativeModuleSpec {
    pub event_emitters: Vec<NativeModuleEventEmitterShape>,
    pub methods: Vec<NativeModulePropertyShape>,
}

pub type NativeModuleEventEmitterShape = NamedShape<NativeModuleTypeAnnotation>;
pub type NativeModulePropertyShape = NamedShape<NativeModuleTypeAnnotation>;

#[derive(Debug, Clone)]
pub enum NativeModuleTypeAnnotation {
    NullableTypeAnnotation {
        type_annotation: Box<NativeModuleTypeAnnotation>,
    },
    FunctionTypeAnnotation {
        params: Vec<NamedShape<NativeModuleTypeAnnotation>>,
        return_type_annotation: Box<NativeModuleTypeAnnotation>,
    },
    EventEmitterTypeAnnotation {
        type_annotation: Box<NativeModuleTypeAnnotation>,
    },
    PromiseTypeAnnotation {
        element_type: Box<NativeModuleTypeAnnotation>,
    },
    ArrayTypeAnnotation {
        element_type: Box<NativeModuleTypeAnnotation>,
    },
    ObjectTypeAnnotation {
        properties: Vec<NamedShape<NativeModuleTypeAnnotation>>,
        base_types: Option<Vec<String>>,
    },
    UnionTypeAnnotation {
        types: Vec<NativeModuleTypeAnnotation>,
    },
    GenericObjectTypeAnnotation {
        dictionary_value_type: Option<Box<NativeModuleTypeAnnotation>>,
    },
    TypeAliasTypeAnnotation {
        name: String,
    },
    EnumDeclaration {
        name: String,
        member_type: NativeModuleEnumMemberType,
    },
    ReservedTypeAnnotation {
        name: String,
    },
    AnyTypeAnnotation,
    ArrayBufferTypeAnnotation,
    BooleanTypeAnnotation,
    BooleanLiteralTypeAnnotation {
        value: bool,
    },
    DoubleTypeAnnotation,
    FloatTypeAnnotation,
    Int32TypeAnnotation,
    MixedTypeAnnotation,
    NumberTypeAnnotation,
    NumberLiteralTypeAnnotation {
        value: f64,
    },
    StringTypeAnnotation,
    StringLiteralTypeAnnotation {
        value: String,
    },
    VoidTypeAnnotation,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeModuleEnumMemberType {
    NumberTypeAnnotation,
    StringTypeAnnotation,
}

#[derive(Debug, Clone)]
pub struct NativeModuleEnumDeclarationWithMembers {
    pub name: String,
    pub member_type: NativeModuleEnumMemberType,
    pub members: Vec<NativeModuleEnumMember>,
}

#[derive(Debug, Clone)]
pub struct NativeModuleEnumMember {
    pub name: String,
    pub value: NativeModuleEnumMemberValue,
}

#[derive(Debug, Clone)]
pub enum NativeModuleEnumMemberValue {
    NumberLiteralTypeAnnotation { value: f64 },
    StringLiteralTypeAnnotation { value: String },
}
