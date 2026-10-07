// Port of plugin-oxc/src/imports.rs and program.rs's import index.
use rustc_hash::{FxHashMap, FxHashSet};
use std::path::{Component, Path, PathBuf};
use swc_common::DUMMY_SP;
use swc_ecma_ast::*;
use swc_ecma_visit::{VisitMut, VisitMutWith};

#[derive(Clone)]
pub struct ImportInfo {
    pub source: String,
    pub specifier: ImportSpecifier,
}

pub fn build_imports_index(module: &Module) -> FxHashMap<Id, ImportInfo> {
    let mut out = FxHashMap::default();
    for item in &module.body {
        let ModuleItem::ModuleDecl(ModuleDecl::Import(import)) = item else {
            continue;
        };
        if import.type_only {
            continue;
        }
        for specifier in &import.specifiers {
            if matches!(specifier, ImportSpecifier::Named(specifier) if specifier.is_type_only) {
                continue;
            }
            out.insert(
                specifier.local().to_id(),
                ImportInfo {
                    source: import.src.value.to_string_lossy().into_owned(),
                    specifier: specifier.clone(),
                },
            );
        }
    }
    out
}

pub fn split_forwarded_imports(
    closure: &mut Vec<Ident>,
    imports: &FxHashMap<Id, ImportInfo>,
    rebound: &FxHashSet<Id>,
    filename: &str,
    modules: &[String],
    paths: &[String],
) -> Vec<ImportInfo> {
    let mut forwarded = Vec::new();
    closure.retain(|id| {
        let Some(info) = imports.get(&id.to_id()) else {
            return true;
        };
        if rebound.contains(&id.to_id()) || matches!(info.specifier, ImportSpecifier::Namespace(_))
        {
            return true;
        }
        let allowed = if info.source.starts_with('.') {
            can_forward_relative_import(filename, paths)
        } else {
            can_forward_module_import(&info.source, modules)
        };
        if allowed {
            forwarded.push(info.clone());
        }
        !allowed
    });
    forwarded
}

pub fn can_forward_module_import(name: &str, modules: &[String]) -> bool {
    modules.iter().any(|module| {
        name.strip_prefix(module)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
    })
}

pub fn can_forward_relative_import(filename: &str, paths: &[String]) -> bool {
    if filename.is_empty() {
        return false;
    }
    let segments: Vec<_> = filename.split('/').collect();
    let segments = segments
        .iter()
        .rposition(|s| *s == "node_modules")
        .map_or(segments.as_slice(), |index| &segments[index + 1..]);
    paths.iter().any(|path| {
        let allowed: Vec<_> = path.split('/').collect();
        !allowed.is_empty()
            && segments
                .windows(allowed.len())
                .any(|window| window == allowed.as_slice())
    })
}

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if out.file_name().is_some_and(|name| name != "..") {
                    out.pop();
                } else if !path.is_absolute() {
                    out.push("..");
                }
            }
            component => out.push(component.as_os_str()),
        }
    }
    out
}

pub fn pathdiff(from: &Path, to: &Path) -> Option<PathBuf> {
    if from.is_absolute() != to.is_absolute() {
        return None;
    }
    let from = normalize_path(from);
    let to = normalize_path(to);
    let from: Vec<_> = from.components().collect();
    let to: Vec<_> = to.components().collect();
    let common = from.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut out = PathBuf::new();
    for _ in common..from.len() {
        out.push("..");
    }
    for component in &to[common..] {
        out.push(component.as_os_str());
    }
    if out.as_os_str().is_empty() {
        out.push(".");
    }
    Some(out)
}

pub fn create_import_path(filename: &str, original: &str, package_dir: &Path) -> Option<String> {
    let resolved = normalize_path(&Path::new(filename).parent()?.join(original));
    let relative = pathdiff(&package_dir.join(".worklets"), &resolved)?;
    let mut out = relative.to_string_lossy().replace('\\', "/");
    if !out.starts_with('.') && !out.starts_with('/') {
        out.insert_str(0, "./");
    }
    Some(out)
}

pub fn update_relative_requires(
    body: &mut FunctionBody,
    filename: &str,
    paths: &[String],
    package_dir: &Path,
) {
    if !can_forward_relative_import(filename, paths) {
        return;
    }
    struct Rewriter<'a> {
        filename: &'a str,
        package_dir: &'a Path,
    }
    impl VisitMut for Rewriter<'_> {
        fn visit_mut_call_expr(&mut self, call: &mut CallExpr) {
            call.visit_mut_children_with(self);
            if !matches!(&call.callee, Callee::Expr(expr) if matches!(&**expr, Expr::Ident(id) if id.sym == "require"))
            {
                return;
            }
            let Some(ExprOrSpread { spread: None, expr }) = call.args.first_mut() else {
                return;
            };
            let Expr::Lit(Lit::Str(value)) = &mut **expr else {
                return;
            };
            let original = value.value.to_string_lossy();
            if original.starts_with('.') {
                if let Some(rebased) =
                    create_import_path(self.filename, &original, self.package_dir)
                {
                    *value = Str {
                        span: DUMMY_SP,
                        value: rebased.into(),
                        raw: None,
                    };
                }
            }
        }
    }
    body.visit_mut_with(&mut Rewriter {
        filename,
        package_dir,
    });
}

pub fn resolve_package_dir(filename: &str, cwd: Option<&str>) -> Option<PathBuf> {
    let cwd = cwd
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())?;
    let file = normalize_path(&cwd.join(filename));
    for ancestor in file.parent()?.ancestors().chain(cwd.ancestors()) {
        if ancestor
            .file_name()
            .is_some_and(|name| name == "react-native-worklets")
            && ancestor.join("package.json").is_file()
        {
            return Some(ancestor.to_path_buf());
        }
        let candidate = ancestor.join("node_modules/react-native-worklets");
        if candidate.join("package.json").is_file() {
            return Some(candidate);
        }
    }
    None
}
