mod common;

use swc_common::{sync::Lrc, FileName, SourceMap};
use swc_ecma_parser::{parse_file_as_module, FlowSyntax, Syntax, TsSyntax};
use swc_ecma_visit::VisitMutWith;
use swc_react_native_codegen::{CodegenOptions, CodegenVisitor};

fn fixture(typescript: bool, aliases: &str, annotation: &str) -> (String, String) {
    let extension = if typescript { "ts" } else { "js" };
    let props = if typescript {
        format!("interface NativeProps extends ViewProps {{ value?: {annotation}; }}")
    } else {
        format!("type NativeProps = Readonly<{{ ...ViewProps, value?: {annotation} }}>;")
    };
    (
        format!("AliasesNativeComponent.{extension}"),
        format!(
            "{aliases}\n{props}\n\
             export default codegenNativeComponent<NativeProps>('Aliases');"
        ),
    )
}

fn assert_alias_processors(typescript: bool) {
    let array = if typescript {
        "ReadonlyArray"
    } else {
        "$ReadOnlyArray"
    };
    let nullable = if typescript {
        "Tint | null | undefined"
    } else {
        "?Tint"
    };
    let aliases = format!(
        "type Tint = ColorValue; type ChainedTint = Tint;\n\
         type NullableTint = {nullable}; type Palette = {array}<Tint>;\n\
         type Image = ImageSource; type Point = PointValue; type Insets = EdgeInsetsValue;"
    );
    // These aliases and wrappers are accepted by the RN 0.84 Babel codegen parser.
    for (annotation, expected) in [
        ("Tint".to_string(), ".colorAttribute"),
        ("ChainedTint".to_string(), ".colorAttribute"),
        ("NullableTint".to_string(), ".colorAttribute"),
        ("WithDefault<Tint, null>".to_string(), ".colorAttribute"),
        ("Palette".to_string(), "StyleSheet/processColorArray"),
        (
            format!("{array}<ChainedTint>"),
            "StyleSheet/processColorArray",
        ),
        (
            format!("WithDefault<{array}<ColorValue>, null>"),
            "StyleSheet/processColorArray",
        ),
        (
            "WithDefault<Palette, null>".to_string(),
            "StyleSheet/processColorArray",
        ),
        ("Image".to_string(), "Image/resolveAssetSource"),
        ("Point".to_string(), "Utilities/differ/pointsDiffer"),
        ("Insets".to_string(), "Utilities/differ/insetsDiffer"),
    ] {
        let (filename, source) = fixture(typescript, &aliases, &annotation);
        let output = common::transform_fixture(&filename, &source).unwrap();
        assert!(
            output.contains(expected),
            "Missing {expected} for {annotation}: {output}"
        );
    }
}

#[test]
fn flow_prop_aliases_preserve_processors() {
    assert_alias_processors(false);
}

#[test]
fn typescript_prop_aliases_preserve_processors() {
    assert_alias_processors(true);
}

#[test]
fn typescript_readonly_wrappers_preserve_processors_and_events() {
    for (annotation, expected) in [
        ("Readonly<ColorValue>", ".colorAttribute"),
        (
            "Readonly<ReadonlyArray<ColorValue>>",
            "StyleSheet/processColorArray",
        ),
        ("WithDefault<Readonly<ColorValue>, null>", ".colorAttribute"),
    ] {
        let (filename, source) = fixture(true, "", annotation);
        let output = common::transform_fixture(&filename, &source).unwrap();
        assert!(output.contains(expected), "{annotation}: {output}");
    }

    let source = "type Changed = Readonly<DirectEventHandler<null, 'paperChanged'>>;\n\
        interface NativeProps extends ViewProps { onChange?: Changed | null; }\n\
        export default codegenNativeComponent<NativeProps>('Events');";
    let output = common::transform_fixture("EventsNativeComponent.ts", source).unwrap();
    assert!(output.contains("directEventTypes:"), "{output}");
    assert!(output.contains("topPaperChanged:"), "{output}");
    assert!(
        output.contains("registrationName: \"onChange\""),
        "{output}"
    );
}

#[test]
fn string_enum_values_are_not_interpreted_as_reserved_types() {
    for typescript in [false, true] {
        for literal in [
            "ColorValue",
            "ImageSource",
            "PointValue",
            "DirectEventHandler",
        ] {
            let annotation = format!("WithDefault<'{literal}' | 'none', '{literal}'>");
            let (filename, source) = fixture(typescript, "", &annotation);
            let output = common::transform_fixture(&filename, &source).unwrap();
            assert!(output.contains("value: true"), "{annotation}: {output}");
            for unexpected in [
                ".colorAttribute",
                "resolveAssetSource",
                "pointsDiffer",
                "ConditionallyIgnoredEventHandlers",
            ] {
                assert!(!output.contains(unexpected), "{annotation}: {output}");
            }
        }
        // An enum literal at the top level must not become an event either.
        let (filename, source) = fixture(typescript, "", "'DirectEventHandler' | 'none'");
        let output = common::transform_fixture(&filename, &source).unwrap();
        assert!(output.contains("value: true"), "{output}");
        assert!(!output.contains("directEventTypes:"), "{output}");
    }
}

#[test]
fn cyclic_prop_alias_returns_error_without_changing_module() {
    for typescript in [false, true] {
        for annotation in ["First", "ReadonlyArray<First>"] {
            let (filename, source) = fixture(
                typescript,
                "type First = Second; type Second = First;",
                annotation,
            );
            let syntax = if typescript {
                Syntax::Typescript(TsSyntax::default())
            } else {
                Syntax::Flow(FlowSyntax {
                    all: true,
                    ..Default::default()
                })
            };
            let cm: Lrc<SourceMap> = Default::default();
            let fm = cm.new_source_file(FileName::Custom(filename.clone()).into(), source);
            let mut module =
                parse_file_as_module(&fm, syntax, Default::default(), None, &mut vec![]).unwrap();
            let original = module.clone();
            let mut visitor = CodegenVisitor::new(cm, CodegenOptions { filename });
            module.visit_mut_with(&mut visitor);
            assert!(
                visitor.into_result().is_err(),
                "Cycle accepted for {annotation}"
            );
            assert_eq!(
                module, original,
                "Failed transform changed input for {annotation}"
            );
        }
    }
}
