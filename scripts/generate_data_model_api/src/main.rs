//! Deterministically render and verify the curated `iroha_data_model` root API facade.
//!
//! The public API contract lives in `public_api.rs`. This binary resolves every
//! declared module through `iroha_data_model/src/lib.rs`, verifies every export
//! against that module's parsed public surface, and then renders the transparent
//! and non-transparent facade files. Write mode accepts only an explicit external
//! staging root; check mode compares the repository outputs without writing.

use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    process,
};

use syn::{Item, ItemMod, UseTree, Visibility};

const API_CONTRACT: &str = include_str!("../public_api.rs");
const MAX_LINE_WIDTH: usize = 100;
const OUTPUTS: [(&str, bool); 2] = [
    (
        "crates/iroha_data_model/non_transparent_api.rs",
        false,
    ),
    ("crates/iroha_data_model/transparent_api.rs", true),
];
const USAGE: &str = "Usage:\n  generate_data_model_api --check\n  generate_data_model_api --write --output-root <absolute-external-directory>";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Check,
    Write,
    Help,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Options {
    mode: Mode,
    output_root: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ApiModule {
    name: String,
    items: Vec<String>,
}

fn parse_options_from(arguments: &[String]) -> Result<Options, String> {
    if matches!(arguments, [argument] if argument == "--help" || argument == "-h") {
        return Ok(Options {
            mode: Mode::Help,
            output_root: None,
        });
    }

    let mut mode = None;
    let mut output_root = None;
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--check" | "--write" => {
                let requested = if argument == "--check" {
                    Mode::Check
                } else {
                    Mode::Write
                };
                if mode.replace(requested).is_some() {
                    return Err(format!("select exactly one mode\n{USAGE}"));
                }
            }
            "--output-root" => {
                if output_root.is_some() {
                    return Err(format!("--output-root was supplied more than once\n{USAGE}"));
                }
                let value = arguments
                    .next()
                    .ok_or_else(|| format!("--output-root requires a path\n{USAGE}"))?;
                if value.is_empty() || value.starts_with('-') {
                    return Err(format!(
                        "--output-root requires a non-empty directory path\n{USAGE}"
                    ));
                }
                output_root = Some(PathBuf::from(value));
            }
            _ => return Err(format!("unknown argument `{argument}`\n{USAGE}")),
        }
    }

    let mode = mode.ok_or_else(|| format!("a mode is required\n{USAGE}"))?;
    match (mode, output_root) {
        (Mode::Check, None) => Ok(Options {
            mode,
            output_root: None,
        }),
        (Mode::Check, Some(_)) => Err(format!(
            "--check reads only repository outputs and forbids --output-root\n{USAGE}"
        )),
        (Mode::Write, Some(output_root)) => Ok(Options {
            mode,
            output_root: Some(output_root),
        }),
        (Mode::Write, None) => Err(format!(
            "--write requires an explicit --output-root\n{USAGE}"
        )),
        (Mode::Help, _) => unreachable!("help is handled before option parsing"),
    }
}

fn parse_options() -> Result<Options, String> {
    parse_options_from(&env::args().skip(1).collect::<Vec<_>>())
}

fn require_regular_file(path: &Path, label: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("inspect {label} {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "{label} must be a non-symbolic regular file: {}",
            path.display()
        ));
    }
    Ok(())
}

fn repository_root() -> Result<PathBuf, String> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| "generator manifest must be two levels below the repository root".to_owned())?;
    let canonical = fs::canonicalize(root)
        .map_err(|error| format!("canonicalize repository root {}: {error}", root.display()))?;
    require_regular_file(&canonical.join("Cargo.toml"), "repository manifest")?;
    require_regular_file(
        &canonical.join("crates/iroha_data_model/src/lib.rs"),
        "data-model crate root",
    )?;
    Ok(canonical)
}

fn contract_module(tree: &UseTree) -> Result<ApiModule, String> {
    let UseTree::Path(crate_path) = tree else {
        return Err("contract exports must begin with `crate::`".to_owned());
    };
    if crate_path.ident != "crate" {
        return Err("contract exports must begin with `crate::`".to_owned());
    }
    let UseTree::Path(module_path) = crate_path.tree.as_ref() else {
        return Err("contract exports must name exactly one crate module".to_owned());
    };
    let module = module_path.ident.to_string();
    let mut items = Vec::new();
    match module_path.tree.as_ref() {
        UseTree::Name(name) => items.push(name.ident.to_string()),
        UseTree::Group(group) => {
            if group.items.is_empty() {
                return Err(format!("contract module `{module}` has no exports"));
            }
            for tree in &group.items {
                let UseTree::Name(name) = tree else {
                    return Err(format!(
                        "contract module `{module}` accepts only unqualified item names"
                    ));
                };
                items.push(name.ident.to_string());
            }
        }
        _ => {
            return Err(format!(
                "contract module `{module}` accepts only one item or a flat item group"
            ));
        }
    }
    let unique: BTreeSet<_> = items.iter().collect();
    if unique.len() != items.len() {
        return Err(format!("contract module `{module}` contains duplicate exports"));
    }
    Ok(ApiModule {
        name: module,
        items,
    })
}

fn parse_contract(source: &str) -> Result<Vec<ApiModule>, String> {
    let file = syn::parse_file(source)
        .map_err(|error| format!("parse public API contract: {error}"))?;
    if file.shebang.is_some() || !file.attrs.is_empty() {
        return Err("public API contract must contain only public use declarations".to_owned());
    }

    let mut modules = Vec::new();
    let mut module_names = BTreeSet::new();
    let mut item_owners = BTreeMap::new();
    for item in file.items {
        let Item::Use(item_use) = item else {
            return Err("public API contract must contain only public use declarations".to_owned());
        };
        if !item_use.attrs.is_empty()
            || !matches!(item_use.vis, Visibility::Public(_))
            || item_use.leading_colon.is_some()
        {
            return Err(
                "public API contract declarations must be unconditional `pub use crate::...` items"
                    .to_owned(),
            );
        }
        let module = contract_module(&item_use.tree)?;
        if !module_names.insert(module.name.clone()) {
            return Err(format!(
                "contract module `{}` is declared more than once",
                module.name
            ));
        }
        for item in &module.items {
            if let Some(previous) = item_owners.insert(item.clone(), module.name.clone()) {
                return Err(format!(
                    "contract export `{item}` is owned by both `{previous}` and `{}`",
                    module.name
                ));
            }
        }
        modules.push(module);
    }
    if modules.is_empty() {
        return Err("public API contract must declare at least one module".to_owned());
    }
    Ok(modules)
}

fn render_contract(modules: &[ApiModule]) -> String {
    let mut output = String::new();
    for module in modules {
        if module.items.len() == 1 {
            output.push_str("pub use crate::");
            output.push_str(&module.name);
            output.push_str("::");
            output.push_str(&module.items[0]);
            output.push_str(";\n");
            continue;
        }

        let single_line = format!(
            "pub use crate::{}::{{{}}};",
            module.name,
            module.items.join(", ")
        );
        if single_line.len() <= MAX_LINE_WIDTH {
            output.push_str(&single_line);
            output.push('\n');
            continue;
        }

        output.push_str("pub use crate::");
        output.push_str(&module.name);
        output.push_str("::{\n");
        let mut line = String::from("    ");
        for item in &module.items {
            let token = format!("{item},");
            let separator = if line.len() == 4 { "" } else { " " };
            if line.len() + separator.len() + token.len() > MAX_LINE_WIDTH {
                output.push_str(&line);
                output.push('\n');
                line.clear();
                line.push_str("    ");
            } else {
                line.push_str(separator);
            }
            line.push_str(&token);
        }
        output.push_str(&line);
        output.push_str("\n};\n");
    }
    output
}

fn validated_contract() -> Result<Vec<ApiModule>, String> {
    let modules = parse_contract(API_CONTRACT)?;
    let canonical = render_contract(&modules);
    if API_CONTRACT != canonical {
        return Err(
            "public_api.rs is not in the generator's canonical deterministic format".to_owned(),
        );
    }
    Ok(modules)
}

fn external_module_source(
    source_root: &Path,
    declaration: &ItemMod,
) -> Result<PathBuf, String> {
    let module = declaration.ident.to_string();
    if declaration.content.is_some() {
        return Err(format!(
            "contract module `{module}` must be an external source module"
        ));
    }
    if declaration
        .attrs
        .iter()
        .any(|attribute| attribute.path().is_ident("path"))
    {
        return Err(format!(
            "contract module `{module}` must not override its source with #[path]"
        ));
    }

    let candidates = [
        source_root.join(format!("{module}.rs")),
        source_root.join(&module).join("mod.rs"),
    ];
    let existing: Vec<_> = candidates
        .into_iter()
        .filter(|candidate| candidate.exists())
        .collect();
    let [path] = existing.as_slice() else {
        return Err(format!(
            "contract module `{module}` must resolve to exactly one of {} or {}; found {}",
            source_root.join(format!("{module}.rs")).display(),
            source_root.join(&module).join("mod.rs").display(),
            existing.len()
        ));
    };
    require_regular_file(path, &format!("contract module `{module}` source"))?;
    Ok(path.clone())
}

fn public_item_name(item: &Item) -> Option<String> {
    let (visibility, identifier) = match item {
        Item::Const(item) => (&item.vis, &item.ident),
        Item::Enum(item) => (&item.vis, &item.ident),
        Item::Fn(item) => (&item.vis, &item.sig.ident),
        Item::Mod(item) => (&item.vis, &item.ident),
        Item::Static(item) => (&item.vis, &item.ident),
        Item::Struct(item) => (&item.vis, &item.ident),
        Item::Trait(item) => (&item.vis, &item.ident),
        Item::TraitAlias(item) => (&item.vis, &item.ident),
        Item::Type(item) => (&item.vis, &item.ident),
        Item::Union(item) => (&item.vis, &item.ident),
        _ => return None,
    };
    matches!(visibility, Visibility::Public(_)).then(|| identifier.to_string())
}

fn inline_module_items<'a>(
    items: &'a [Item],
    path: &[String],
) -> Result<&'a [Item], String> {
    let path = path.strip_prefix(&["self".to_owned()]).unwrap_or(path);
    if path.is_empty() {
        return Err("public glob re-export must name an inline module".to_owned());
    }

    let mut current = items;
    for segment in path {
        let matches: Vec<_> = current
            .iter()
            .filter_map(|item| match item {
                Item::Mod(module) if module.ident == segment => Some(module),
                _ => None,
            })
            .collect();
        let [module] = matches.as_slice() else {
            return Err(format!(
                "public glob re-export `{}` must resolve to exactly one inline module",
                path.join("::")
            ));
        };
        let Some((_, nested)) = &module.content else {
            return Err(format!(
                "public glob re-export `{}` resolves outside the inspected module source",
                path.join("::")
            ));
        };
        current = nested;
    }
    Ok(current)
}

fn collect_use_exports(
    tree: &UseTree,
    prefix: &mut Vec<String>,
    module_items: &[Item],
    exports: &mut BTreeSet<String>,
) -> Result<(), String> {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            let result = collect_use_exports(&path.tree, prefix, module_items, exports);
            prefix.pop();
            result
        }
        UseTree::Name(name) => {
            exports.insert(name.ident.to_string());
            Ok(())
        }
        UseTree::Rename(rename) => {
            exports.insert(rename.rename.to_string());
            Ok(())
        }
        UseTree::Group(group) => {
            for item in &group.items {
                collect_use_exports(item, prefix, module_items, exports)?;
            }
            Ok(())
        }
        UseTree::Glob(_) => {
            let nested = inline_module_items(module_items, prefix)?;
            let public_names: Vec<_> = nested.iter().filter_map(public_item_name).collect();
            if public_names.is_empty() {
                return Err(format!(
                    "public glob re-export `{}` exposes no direct public items",
                    prefix.join("::")
                ));
            }
            exports.extend(public_names);
            Ok(())
        }
    }
}

fn module_public_surface(source: &str, label: &str) -> Result<BTreeSet<String>, String> {
    let file = syn::parse_file(source).map_err(|error| format!("parse {label}: {error}"))?;
    let mut exports: BTreeSet<_> = file.items.iter().filter_map(public_item_name).collect();
    for item in &file.items {
        let Item::Use(item_use) = item else {
            continue;
        };
        if matches!(item_use.vis, Visibility::Public(_)) {
            collect_use_exports(
                &item_use.tree,
                &mut Vec::new(),
                &file.items,
                &mut exports,
            )?;
        }
    }
    Ok(exports)
}

fn collect_explicit_use_names(tree: &UseTree, names: &mut BTreeSet<String>) {
    match tree {
        UseTree::Path(path) => collect_explicit_use_names(&path.tree, names),
        UseTree::Name(name) => {
            names.insert(name.ident.to_string());
        }
        UseTree::Rename(rename) => {
            names.insert(rename.rename.to_string());
        }
        UseTree::Group(group) => {
            for tree in &group.items {
                collect_explicit_use_names(tree, names);
            }
        }
        UseTree::Glob(_) => {}
    }
}

fn validate_source_contract(repository: &Path, modules: &[ApiModule]) -> Result<(), String> {
    let source_root = repository.join("crates/iroha_data_model/src");
    let lib_path = source_root.join("lib.rs");
    let lib_source = fs::read_to_string(&lib_path)
        .map_err(|error| format!("read {}: {error}", lib_path.display()))?;
    let lib = syn::parse_file(&lib_source)
        .map_err(|error| format!("parse {}: {error}", lib_path.display()))?;

    let mut declarations: BTreeMap<String, &ItemMod> = BTreeMap::new();
    let mut existing_root_names = BTreeSet::new();
    for item in &lib.items {
        if let Some(name) = public_item_name(item) {
            existing_root_names.insert(name);
        }
        match item {
            Item::Mod(module) if matches!(module.vis, Visibility::Public(_)) => {
                let name = module.ident.to_string();
                if declarations.insert(name.clone(), module).is_some() {
                    return Err(format!(
                        "data-model crate root declares public module `{name}` more than once"
                    ));
                }
            }
            Item::Use(item_use) => {
                collect_explicit_use_names(&item_use.tree, &mut existing_root_names);
            }
            _ => {}
        }
    }

    for module in modules {
        let declaration = declarations.get(&module.name).ok_or_else(|| {
            format!(
                "contract refers to unknown public data-model module `{}`",
                module.name
            )
        })?;
        let source_path = external_module_source(&source_root, declaration)?;
        let source = fs::read_to_string(&source_path)
            .map_err(|error| format!("read {}: {error}", source_path.display()))?;
        let surface = module_public_surface(&source, &source_path.display().to_string())?;
        for item in &module.items {
            if existing_root_names.contains(item) {
                return Err(format!(
                    "contract export `{item}` from `{}` already exists at the crate root",
                    module.name
                ));
            }
            if !surface.contains(item) {
                return Err(format!(
                    "contract export `{}::{item}` is missing from the module's public surface",
                    module.name
                ));
            }
        }
    }
    Ok(())
}

fn render_api(modules: &[ApiModule], transparent: bool) -> Vec<u8> {
    let mut output = String::from(
        "// @generated\n/// Indicates whether this crate exposes a transparent API.\n",
    );
    output.push_str(&format!(
        "pub const TRANSPARENT_API: bool = {transparent};\n"
    ));
    output.push_str(&render_contract(modules));
    output.into_bytes()
}

fn expected_files(modules: &[ApiModule]) -> BTreeMap<PathBuf, Vec<u8>> {
    OUTPUTS
        .iter()
        .map(|(path, transparent)| {
            (
                PathBuf::from(path),
                render_api(modules, *transparent),
            )
        })
        .collect()
}

fn check_outputs(repository: &Path, expected: &BTreeMap<PathBuf, Vec<u8>>) -> Result<(), String> {
    for (relative, expected_bytes) in expected {
        let path = repository.join(relative);
        require_regular_file(&path, "generated API output")?;
        let actual = fs::read(&path)
            .map_err(|error| format!("read generated API output {}: {error}", path.display()))?;
        if actual != *expected_bytes {
            return Err(format!(
                "generated API output is stale: {}",
                path.display()
            ));
        }
    }
    Ok(())
}

#[derive(Debug, Default, PartialEq, Eq)]
struct TreeInventory {
    directories: BTreeSet<PathBuf>,
    files: BTreeSet<PathBuf>,
}

fn collect_tree_inventory(root: &Path) -> Result<TreeInventory, String> {
    fn visit(root: &Path, directory: &Path, inventory: &mut TreeInventory) -> Result<(), String> {
        let mut entries: Vec<_> = fs::read_dir(directory)
            .map_err(|error| format!("read staging directory {}: {error}", directory.display()))?
            .collect::<Result<_, _>>()
            .map_err(|error| format!("read staging directory {}: {error}", directory.display()))?;
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let relative = path
                .strip_prefix(root)
                .map_err(|_| format!("staging entry escaped output root: {}", path.display()))?
                .to_path_buf();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("inspect staging entry {}: {error}", path.display()))?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "staging tree must not contain symlinks: {}",
                    path.display()
                ));
            }
            if metadata.is_dir() {
                inventory.directories.insert(relative);
                visit(root, &path, inventory)?;
            } else if metadata.is_file() {
                inventory.files.insert(relative);
            } else {
                return Err(format!(
                    "staging tree must contain only directories and regular files: {}",
                    path.display()
                ));
            }
        }
        Ok(())
    }

    let mut inventory = TreeInventory::default();
    visit(root, root, &mut inventory)?;
    Ok(inventory)
}

fn expected_inventory(expected: &BTreeMap<PathBuf, Vec<u8>>) -> TreeInventory {
    let files = expected.keys().cloned().collect();
    let mut directories = BTreeSet::new();
    for path in expected.keys() {
        let mut parent = path.parent();
        while let Some(directory) = parent {
            if directory.as_os_str().is_empty() {
                break;
            }
            directories.insert(directory.to_path_buf());
            parent = directory.parent();
        }
    }
    TreeInventory { directories, files }
}

fn validate_external_output_root(root: &Path, repository: &Path) -> Result<PathBuf, String> {
    if !root.is_absolute() {
        return Err(format!(
            "--output-root must be absolute: {}",
            root.display()
        ));
    }
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| format!("inspect output root {}: {error}", root.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "--output-root must be an existing non-symbolic directory: {}",
            root.display()
        ));
    }
    let canonical = fs::canonicalize(root)
        .map_err(|error| format!("canonicalize output root {}: {error}", root.display()))?;
    if canonical != root {
        return Err(format!(
            "--output-root must be a canonical path without symbolic ancestors: {}",
            root.display()
        ));
    }
    if canonical.parent().is_none() {
        return Err("--output-root must not be the filesystem root".to_owned());
    }
    if canonical.starts_with(repository) {
        return Err(format!(
            "--output-root must be outside the repository: {}",
            canonical.display()
        ));
    }
    Ok(canonical)
}

fn write_outputs(
    root: &Path,
    repository: &Path,
    expected: &BTreeMap<PathBuf, Vec<u8>>,
) -> Result<(), String> {
    let root = validate_external_output_root(root, repository)?;
    let complete_inventory = expected_inventory(expected);
    let initial_inventory = collect_tree_inventory(&root)?;
    if initial_inventory != TreeInventory::default() && initial_inventory != complete_inventory {
        return Err(format!(
            "--output-root must be fresh or contain exactly the two declared outputs: {}",
            root.display()
        ));
    }

    for (relative, bytes) in expected {
        let destination = root.join(relative);
        let parent = destination.parent().ok_or_else(|| {
            format!("generated output has no parent: {}", destination.display())
        })?;
        fs::create_dir_all(parent)
            .map_err(|error| format!("create staging directory {}: {error}", parent.display()))?;
        fs::write(&destination, bytes)
            .map_err(|error| format!("write staged output {}: {error}", destination.display()))?;
    }

    let final_inventory = collect_tree_inventory(&root)?;
    if final_inventory != complete_inventory {
        return Err(format!(
            "staged output inventory is not the exact declared two-file set: {}",
            root.display()
        ));
    }
    check_outputs(&root, expected)
}

fn run() -> Result<(), String> {
    let options = parse_options()?;
    if options.mode == Mode::Help {
        println!("{USAGE}");
        return Ok(());
    }

    let repository = repository_root()?;
    let modules = validated_contract()?;
    validate_source_contract(&repository, &modules)?;
    let expected = expected_files(&modules);
    match options.mode {
        Mode::Check => check_outputs(&repository, &expected),
        Mode::Write => write_outputs(
            options
                .output_root
                .as_deref()
                .expect("write mode requires output root"),
            &repository,
            &expected,
        ),
        Mode::Help => unreachable!("help returned before generation"),
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("ERROR: {error}");
        process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn command_requires_one_explicit_safe_mode() {
        assert_eq!(
            parse_options_from(&arguments(&["--check"])),
            Ok(Options {
                mode: Mode::Check,
                output_root: None,
            })
        );
        assert_eq!(
            parse_options_from(&arguments(&["--write", "--output-root", "/stage"])),
            Ok(Options {
                mode: Mode::Write,
                output_root: Some(PathBuf::from("/stage")),
            })
        );
        assert_eq!(
            parse_options_from(&arguments(&["--help"])),
            Ok(Options {
                mode: Mode::Help,
                output_root: None,
            })
        );

        for rejected in [
            arguments(&[]),
            arguments(&["--write"]),
            arguments(&["--check", "--output-root", "/stage"]),
            arguments(&["--check", "--write", "--output-root", "/stage"]),
            arguments(&["--write", "--output-root"]),
            arguments(&["--write", "--output-root", "--check"]),
            arguments(&["--write", "--output-root", "/one", "--output-root", "/two"]),
            arguments(&["--unknown"]),
        ] {
            assert!(parse_options_from(&rejected).is_err(), "accepted {rejected:?}");
        }
    }

    #[test]
    fn contract_parser_is_strict_and_rejects_ambiguous_exports() {
        let valid = parse_contract(
            "pub use crate::account::{Account, AccountId};\n\
             pub use crate::rwa::Rwa;\n",
        )
        .expect("valid contract");
        assert_eq!(
            valid,
            vec![
                ApiModule {
                    name: "account".to_owned(),
                    items: vec!["Account".to_owned(), "AccountId".to_owned()],
                },
                ApiModule {
                    name: "rwa".to_owned(),
                    items: vec!["Rwa".to_owned()],
                },
            ]
        );

        for rejected in [
            "",
            "use crate::account::Account;",
            "pub use other::account::Account;",
            "pub use crate::account::*;",
            "pub use crate::account::{Account, Account};",
            "pub use crate::account::Account; pub use crate::account::AccountId;",
            "pub use crate::account::Account; pub use crate::other::Account;",
            "pub struct Account;",
        ] {
            assert!(parse_contract(rejected).is_err(), "accepted `{rejected}`");
        }
    }

    #[test]
    fn public_surface_resolves_model_globs_and_direct_rwa_items() {
        let source = r#"
            pub use self::{model::*, nested::Other};
            mod model {
                pub struct Account;
                pub type AccountId = u8;
                fn hidden() {}
            }
            mod nested { pub struct Other; }
            pub struct Rwa;
            struct Private;
        "#;
        let surface = module_public_surface(source, "test module").expect("public surface");
        assert!(surface.contains("Account"));
        assert!(surface.contains("AccountId"));
        assert!(surface.contains("Other"));
        assert!(surface.contains("Rwa"));
        assert!(!surface.contains("hidden"));
        assert!(!surface.contains("Private"));
    }

    #[test]
    fn checked_in_contract_is_canonical_and_contains_rwa() {
        let modules = validated_contract().expect("canonical contract");
        let rwa = modules
            .iter()
            .find(|module| module.name == "rwa")
            .expect("RWA API module");
        assert_eq!(
            rwa.items,
            ["NewRwa", "Rwa", "RwaControlPolicy", "RwaId", "RwaParentRef"]
        );
        assert_eq!(render_contract(&modules), API_CONTRACT);
    }

    #[test]
    fn renderer_is_deterministic_and_matches_the_two_declared_paths() {
        let modules = validated_contract().expect("canonical contract");
        let first = expected_files(&modules);
        let second = expected_files(&modules);
        assert_eq!(first, second);
        assert_eq!(
            first.keys().cloned().collect::<BTreeSet<_>>(),
            OUTPUTS
                .iter()
                .map(|(path, _)| PathBuf::from(path))
                .collect()
        );
        assert!(
            first[Path::new("crates/iroha_data_model/transparent_api.rs")]
                .starts_with(b"// @generated\n/// Indicates whether this crate exposes a transparent API.\npub const TRANSPARENT_API: bool = true;\n")
        );
    }

    #[test]
    fn expected_inventory_is_closed_over_declared_output_parents() {
        let modules = validated_contract().expect("canonical contract");
        let inventory = expected_inventory(&expected_files(&modules));
        assert_eq!(
            inventory.files,
            BTreeSet::from([
                PathBuf::from("crates/iroha_data_model/non_transparent_api.rs"),
                PathBuf::from("crates/iroha_data_model/transparent_api.rs"),
            ])
        );
        assert_eq!(
            inventory.directories,
            BTreeSet::from([
                PathBuf::from("crates"),
                PathBuf::from("crates/iroha_data_model"),
            ])
        );
    }
}
