// Port of plugin-oxc/src/naming.rs.
use std::path::Path;
use swc_ecma_ast::Ident;

pub fn make_worklet_name(name: Option<&str>, filename: &str, number: u32) -> (String, String) {
    let base = Path::new(filename)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknownFile");
    let parts: Vec<_> = filename.split('/').collect();
    let source = parts
        .iter()
        .position(|s| *s == "node_modules")
        .and_then(|index| parts.get(index + 1))
        .map_or_else(|| base.to_string(), |lib| format!("{lib}_{base}"));
    let suffix = format!("{source}{number}");
    let react_name = name.filter(|s| !s.is_empty()).map_or_else(
        || to_identifier(&suffix),
        |name| {
            Ident::verify_symbol(name)
                .err()
                .unwrap_or_else(|| name.to_string())
        },
    );
    let worklet_name = name.filter(|s| !s.is_empty()).map_or_else(
        || to_identifier(&suffix),
        |name| to_identifier(&format!("{name}_{suffix}")),
    );
    (worklet_name, react_name)
}

pub fn to_identifier(input: &str) -> String {
    let mapped: String = input
        .chars()
        .map(|c| if Ident::is_valid_continue(c) { c } else { '-' })
        .collect();
    let trimmed = mapped.trim_start_matches(|c: char| c == '-' || c.is_ascii_digit());
    let mut out = String::with_capacity(trimmed.len());
    let mut upper = false;
    for c in trimmed.chars() {
        if c == '-' {
            upper = true;
        } else if upper {
            out.extend(c.to_uppercase());
            upper = false;
        } else {
            out.push(c);
        }
    }
    if out.chars().next().is_none_or(|c| !Ident::is_valid_start(c)) {
        out.insert(0, '_');
    }
    out
}
