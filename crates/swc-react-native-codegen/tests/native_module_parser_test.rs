use swc_common::{sync::Lrc, FileName, SourceMap};
use swc_ecma_ast::Module;
use swc_ecma_parser::{parse_file_as_module, FlowSyntax, Syntax, TsSyntax};
use swc_react_native_codegen::{
    codegen_schema::{
        ModuleSchema, NativeModulePropertyShape, NativeModuleSchema, NativeModuleTypeAnnotation,
        SchemaType,
    },
    parse_codegen_schema,
};

fn parse_schema(filename: &str, code: &str) -> anyhow::Result<SchemaType> {
    let syntax = if filename.ends_with(".ts") {
        Syntax::Typescript(TsSyntax {
            tsx: false,
            ..Default::default()
        })
    } else {
        Syntax::Flow(FlowSyntax {
            all: true,
            enums: true,
            ..Default::default()
        })
    };

    let cm: Lrc<SourceMap> = Default::default();
    let fm = cm.new_source_file(FileName::Custom(filename.into()).into(), code.to_string());
    let module: Module = parse_file_as_module(&fm, syntax, Default::default(), None, &mut vec![])
        .expect("failed to parse fixture");

    parse_codegen_schema(filename, &module)
}

fn native_module<'a>(schema: &'a SchemaType, haste_module_name: &str) -> &'a NativeModuleSchema {
    match schema.modules.get(haste_module_name) {
        Some(ModuleSchema::NativeModule(module)) => module,
        other => panic!("expected NativeModule schema, got {other:?}"),
    }
}

fn method<'a>(module: &'a NativeModuleSchema, method_name: &str) -> &'a NativeModulePropertyShape {
    module
        .spec
        .methods
        .iter()
        .find(|method| method.name == method_name)
        .expect("method not found")
}

fn strip_nullable(annotation: &NativeModuleTypeAnnotation) -> &NativeModuleTypeAnnotation {
    match annotation {
        NativeModuleTypeAnnotation::NullableTypeAnnotation { type_annotation } => {
            strip_nullable(type_annotation)
        }
        other => other,
    }
}

#[test]
fn parses_typescript_native_module_schema() {
    let code = r#"
import type {TurboModule, CodegenTypes} from 'react-native';
import {TurboModuleRegistry} from 'react-native';

export type Payload = {
  value: string;
  count?: number;
};

export enum Quality {
  Low = 1,
  High = 2,
}

export interface Spec extends TurboModule {
  readonly readBuffer: (id: string) => ArrayBuffer;
  readonly writeBuffer: (arg: ArrayBuffer | null) => Promise<ArrayBuffer>;
  readonly getPayload: () => Payload;
  readonly choose: (quality: Quality) => void;
  readonly onReady: CodegenTypes.EventEmitter<Payload>;
}

export default TurboModuleRegistry.getEnforcing<Spec>('SampleTurboModule');
"#;

    let schema = parse_schema("NativeSampleTurboModule.ts", code).unwrap();
    let module = native_module(&schema, "NativeSampleTurboModule");

    assert_eq!(module.module_name, "SampleTurboModule");
    assert!(module.excluded_platforms.is_none());
    assert!(module.alias_map.contains_key("Payload"));
    assert!(module.enum_map.contains_key("Quality"));
    assert_eq!(module.spec.methods.len(), 4);
    assert_eq!(module.spec.event_emitters.len(), 1);

    let read_buffer = method(module, "readBuffer");
    match strip_nullable(&read_buffer.type_annotation) {
        NativeModuleTypeAnnotation::FunctionTypeAnnotation {
            return_type_annotation,
            ..
        } => assert!(matches!(
            strip_nullable(return_type_annotation),
            NativeModuleTypeAnnotation::ArrayBufferTypeAnnotation
        )),
        other => panic!("expected function type, got {other:?}"),
    }

    let write_buffer = method(module, "writeBuffer");
    match strip_nullable(&write_buffer.type_annotation) {
        NativeModuleTypeAnnotation::FunctionTypeAnnotation {
            params,
            return_type_annotation,
        } => {
            assert!(matches!(
                params.first().map(|param| &param.type_annotation),
                Some(NativeModuleTypeAnnotation::NullableTypeAnnotation { .. })
            ));
            match strip_nullable(return_type_annotation) {
                NativeModuleTypeAnnotation::PromiseTypeAnnotation { element_type } => {
                    assert!(matches!(
                        strip_nullable(element_type),
                        NativeModuleTypeAnnotation::ArrayBufferTypeAnnotation
                    ));
                }
                other => panic!("expected Promise type, got {other:?}"),
            }
        }
        other => panic!("expected function type, got {other:?}"),
    }
}

#[test]
fn parses_flow_native_module_schema() {
    let code = r#"
import type {TurboModule} from 'react-native/Libraries/TurboModule/RCTExport';
import * as TurboModuleRegistry from 'react-native/Libraries/TurboModule/TurboModuleRegistry';

export interface Spec extends TurboModule {
  +readBuffer: (id: string) => ArrayBuffer;
}

export default TurboModuleRegistry.get<Spec>('FlowSampleTurboModule');
"#;

    let schema = parse_schema("NativeFlowSampleTurboModule.js", code).unwrap();
    let module = native_module(&schema, "NativeFlowSampleTurboModule");
    let read_buffer = method(module, "readBuffer");

    assert_eq!(module.module_name, "FlowSampleTurboModule");
    match strip_nullable(&read_buffer.type_annotation) {
        NativeModuleTypeAnnotation::FunctionTypeAnnotation {
            return_type_annotation,
            ..
        } => assert!(matches!(
            strip_nullable(return_type_annotation),
            NativeModuleTypeAnnotation::ArrayBufferTypeAnnotation
        )),
        other => panic!("expected function type, got {other:?}"),
    }
}

#[test]
fn rejects_array_buffer_object_properties() {
    let code = r#"
import type {TurboModule} from 'react-native';
import {TurboModuleRegistry} from 'react-native';

export type Payload = {
  buffer: ArrayBuffer;
};

export interface Spec extends TurboModule {
  readonly getPayload: () => Payload;
}

export default TurboModuleRegistry.getEnforcing<Spec>('SampleTurboModule');
"#;

    let error = parse_schema("NativeSampleTurboModule.ts", code)
        .expect_err("ArrayBuffer object property should be rejected")
        .to_string();

    assert!(error.contains("cannot have type"));
    assert!(error.contains("ArrayBufferTypeAnnotation"));
}

fn parse_alias_fixture(
    filename: &str,
    aliases: &str,
    type_name: &str,
) -> anyhow::Result<SchemaType> {
    let modifier = if filename.ends_with(".ts") {
        "readonly "
    } else {
        "+"
    };
    parse_schema(
        filename,
        &format!(
            "import type {{TurboModule, CodegenTypes}} from 'react-native';
             import {{TurboModuleRegistry}} from 'react-native';
             {aliases}
             export interface Spec extends TurboModule {{
               {modifier}getValue: (arg: {type_name}) => {type_name};
             }}
             export default TurboModuleRegistry.getEnforcing<Spec>('SampleTurboModule');"
        ),
    )
}

fn assert_alias_method_type(schema: &SchemaType, expected: &str) {
    let module = native_module(schema, "NativeSampleTurboModule");
    match &method(module, "getValue").type_annotation {
        NativeModuleTypeAnnotation::FunctionTypeAnnotation {
            params,
            return_type_annotation,
        } => {
            for annotation in [&params[0].type_annotation, &**return_type_annotation] {
                assert_eq!(
                    format!("{annotation:?}"),
                    format!("{expected}TypeAnnotation")
                );
            }
        }
        other => panic!("expected function type, got {other:?}"),
    }
}

#[test]
fn resolves_namespaced_native_module_aliases() {
    // RN 0.88's NAMESPACED_NATIVE_MODULE_WITH_LOCAL_TYPE_ALIASES fixture.
    for filename in ["NativeSampleTurboModule.ts", "NativeSampleTurboModule.js"] {
        for (type_name, expected) in [("Double", "Double"), ("MyFloat", "Float")] {
            let schema = parse_alias_fixture(
                filename,
                "type Double = CodegenTypes.Double; type MyFloat = CodegenTypes.Float;",
                type_name,
            )
            .unwrap();
            let module = native_module(&schema, "NativeSampleTurboModule");
            assert!(module.alias_map.is_empty());
            assert!(module.enum_map.is_empty());
            assert_alias_method_type(&schema, expected);
        }
    }
}

#[test]
fn distinguishes_local_aliases_from_namespace_members() {
    for filename in ["NativeSampleTurboModule.ts", "NativeSampleTurboModule.js"] {
        for (type_name, expected) in [("Double", "Float"), ("CodegenTypes.Double", "Double")] {
            let schema =
                parse_alias_fixture(filename, "type Double = CodegenTypes.Float;", type_name)
                    .unwrap();
            assert_alias_method_type(&schema, expected);
        }
    }
}

#[test]
fn rejects_qualified_references_to_local_types() {
    for filename in ["NativeSampleTurboModule.ts", "NativeSampleTurboModule.js"] {
        for declaration in [
            "type Payload = {value: string};",
            "interface Payload {value: string}",
            "enum Payload {Value = 'value'}",
        ] {
            let error = parse_alias_fixture(filename, declaration, "CodegenTypes.Payload")
                .expect_err("namespace members must not resolve to local declarations")
                .to_string();
            assert!(error.contains("Unsupported NativeModule type reference 'Payload'"));
        }
    }
}

#[test]
fn stops_resolving_cyclic_native_module_aliases() {
    for filename in ["NativeSampleTurboModule.ts", "NativeSampleTurboModule.js"] {
        for aliases in [
            "type Value = Value;",
            "type Value = Other; type Other = Value;",
        ] {
            let error = parse_alias_fixture(filename, aliases, "Value")
                .expect_err("cyclic aliases must not produce unresolved schema references")
                .to_string();
            assert!(error.contains("Unsupported NativeModule type reference 'Value'"));
        }
        let schema = parse_alias_fixture(filename, "type Double = Double;", "Double").unwrap();
        assert_alias_method_type(&schema, "Double");
    }
}

#[test]
fn preserves_recursive_native_module_object_aliases() {
    for filename in ["NativeSampleTurboModule.ts", "NativeSampleTurboModule.js"] {
        let schema =
            parse_alias_fixture(filename, "type Payload = {next?: Payload};", "Payload").unwrap();
        let module = native_module(&schema, "NativeSampleTurboModule");
        match module.alias_map.get("Payload").unwrap() {
            NativeModuleTypeAnnotation::ObjectTypeAnnotation { properties, .. } => {
                assert!(properties[0].optional);
                assert!(matches!(
                    &properties[0].type_annotation,
                    NativeModuleTypeAnnotation::TypeAliasTypeAnnotation { name } if name == "Payload"
                ));
            }
            other => panic!("expected object type, got {other:?}"),
        }
    }
}

#[test]
fn preserves_legacy_native_module_alias_chains() {
    for filename in ["NativeSampleTurboModule.ts", "NativeSampleTurboModule.js"] {
        for (target, expected) in [
            ("string", "String"),
            ("Int32", "Int32"),
            ("CodegenTypes.Float", "Float"),
            ("CodegenTypes.Double", "Double"),
        ] {
            let aliases =
                format!("type First = {target}; type Second = First; type Third = Second;");
            let schema = parse_alias_fixture(filename, &aliases, "Third").unwrap();
            assert_alias_method_type(&schema, expected);
            assert!(native_module(&schema, "NativeSampleTurboModule")
                .alias_map
                .is_empty());
        }

        let schema = parse_alias_fixture(
            filename,
            "type First = string | null; type Second = First;",
            "Second",
        )
        .unwrap();
        let module = native_module(&schema, "NativeSampleTurboModule");
        match &method(module, "getValue").type_annotation {
            NativeModuleTypeAnnotation::FunctionTypeAnnotation {
                params,
                return_type_annotation,
            } => {
                for annotation in [&params[0].type_annotation, &**return_type_annotation] {
                    assert!(matches!(
                        annotation,
                        NativeModuleTypeAnnotation::NullableTypeAnnotation { type_annotation }
                            if matches!(**type_annotation, NativeModuleTypeAnnotation::StringTypeAnnotation)
                    ));
                }
            }
            other => panic!("expected function type, got {other:?}"),
        }
    }
}

#[test]
fn preserves_legacy_native_module_object_alias_chains() {
    for filename in ["NativeSampleTurboModule.ts", "NativeSampleTurboModule.js"] {
        for target in [
            "Payload",
            "Readonly<Payload>",
            "ReadonlyArray<Payload>",
            "Array<Payload>",
        ] {
            let schema = parse_alias_fixture(
                filename,
                &format!(
                    "type Payload = {{value: string}}; type First = {target}; type Second = First;"
                ),
                "Second",
            )
            .unwrap();
            let module = native_module(&schema, "NativeSampleTurboModule");
            assert_eq!(module.alias_map.len(), 1);
            assert!(module.alias_map.contains_key("Payload"));
            match &method(module, "getValue").type_annotation {
                NativeModuleTypeAnnotation::FunctionTypeAnnotation {
                    params,
                    return_type_annotation,
                } => {
                    for mut annotation in [&params[0].type_annotation, &**return_type_annotation] {
                        if target.contains("Array") {
                            let NativeModuleTypeAnnotation::ArrayTypeAnnotation { element_type } =
                                annotation
                            else {
                                panic!("expected array type, got {annotation:?}");
                            };
                            annotation = element_type;
                        }
                        assert!(matches!(annotation,
                            NativeModuleTypeAnnotation::TypeAliasTypeAnnotation { name } if name == "Payload"
                        ));
                    }
                }
                other => panic!("expected function type, got {other:?}"),
            }
        }
    }
}
