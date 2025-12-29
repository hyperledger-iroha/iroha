//! Utility to extract and generate a summary of public items in `mod model`.
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use syn::{Item, UseTree};

// Parse the content of a file and extract names of public items inside the
// `mod model` module if it exists.
fn parse_model_module(src: &str) -> Vec<String> {
    let bytes = src.as_bytes();
    let mut depth = 0usize;
    let mut i = 0;
    let mut mod_idx = None;
    while i + 9 <= bytes.len() {
        match bytes[i] {
            b'{' => {
                depth += 1;
                i += 1;
            }
            b'}' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            b'm' if depth == 0 && src[i..].starts_with("mod model") => {
                mod_idx = Some(i);
                break;
            }
            _ => i += 1,
        }
    }
    let Some(mod_idx) = mod_idx else {
        return Vec::new();
    };
    let brace_idx = match src[mod_idx..].find('{') {
        Some(idx) => mod_idx + idx + 1,
        None => return Vec::new(),
    };
    let mut depth = 1usize;
    let mut i = brace_idx;
    while i < bytes.len() && depth > 0 {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    let block = &src[brace_idx..i.saturating_sub(1)];
    let mut names = Vec::new();
    let mut rest = block;
    while let Some(pos) = rest.find("pub ") {
        let after_pub = &rest[pos + 4..];
        let kw_opt = ["struct", "enum", "union", "type", "const", "fn", "trait"]
            .iter()
            .find_map(|kw| after_pub.strip_prefix(kw).map(|s| (*kw, s)));
        if let Some((_kw, mut s)) = kw_opt {
            s = s.trim_start();
            let ident: String = s
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if !ident.is_empty() {
                names.push(ident);
            }
        }
        rest = &after_pub[1..];
    }
    names
}

const MODULES: &[&str] = &[
    "account.rs",
    "asset.rs",
    "block/mod.rs",
    "domain.rs",
    "events/mod.rs",
    "executor.rs",
    "id.rs",
    "ipfs.rs",
    "isi/mod.rs",
    "level.rs",
    "metadata.rs",
    "name.rs",
    "nft.rs",
    "parameter/mod.rs",
    "peer.rs",
    "permission.rs",
    "query/mod.rs",
    "role.rs",
    "transaction/mod.rs",
    "trigger.rs",
    "validation_fail.rs",
];

fn collect_model_items(src_dir: &Path) -> std::io::Result<BTreeMap<String, Vec<String>>> {
    let mut map = BTreeMap::new();
    for module in MODULES {
        let path = src_dir.join(module);
        if !path.exists() {
            continue;
        }
        // Skip symlinked modules to avoid following self-referential links
        // that could otherwise lead to infinite recursion when the script is
        // pointed at a directory with symlink loops.
        if path.symlink_metadata()?.file_type().is_symlink() {
            continue;
        }
        let src = fs::read_to_string(&path)?;
        if !src.contains("#[model]") {
            continue;
        }
        let names = parse_model_module(&src);
        if !names.is_empty() {
            let module_name = module
                .strip_suffix("/mod.rs")
                .or_else(|| module.strip_suffix(".rs"))
                .unwrap_or(module);
            map.insert(module_name.to_string(), names);
        }
    }
    Ok(map)
}

fn collect_lib_exports(lib_rs: &Path) -> std::io::Result<BTreeMap<String, ()>> {
    let src = fs::read_to_string(lib_rs)?;
    let file = syn::parse_file(&src).expect("failed to parse lib.rs");
    let mut set = BTreeMap::new();
    for item in file.items {
        if let Item::Use(u) = item {
            collect_use_tree(&u.tree, &mut set);
        }
    }
    Ok(set)
}

fn collect_use_tree(tree: &UseTree, set: &mut BTreeMap<String, ()>) {
    match tree {
        UseTree::Name(n) => {
            set.insert(n.ident.to_string(), ());
        }
        UseTree::Rename(r) => {
            set.insert(r.rename.to_string(), ());
        }
        UseTree::Path(p) => collect_use_tree(&p.tree, set),
        UseTree::Group(g) => g.items.iter().for_each(|t| collect_use_tree(t, set)),
        UseTree::Glob(_) => {}
    }
}

fn generate_api(
    out_dir: &Path,
    modules: &BTreeMap<String, Vec<String>>,
    transparent: bool,
) -> std::io::Result<()> {
    let file_name = if transparent {
        "transparent_api.rs"
    } else {
        "non_transparent_api.rs"
    };
    let mut contents = String::from("// @generated\n");
    contents.push_str(&format!(
        "pub const TRANSPARENT_API: bool = {};\n",
        transparent
    ));
    for (module, items) in modules {
        contents.push_str(&format!(
            "pub use crate::{}::{{{}}};\n",
            module,
            items.join(", ")
        ));
    }
    fs::write(out_dir.join(file_name), contents)
}

fn main() -> std::io::Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crate_dir = manifest_dir
        .join("../../crates/iroha_data_model")
        .canonicalize()?;
    let src_dir = crate_dir.join("src");
    let mut modules = collect_model_items(&src_dir)?;
    let exports = collect_lib_exports(&src_dir.join("lib.rs"))?;
    for items in modules.values_mut() {
        items.retain(|item| !exports.contains_key(item));
    }
    modules.retain(|_, v| !v.is_empty());
    generate_api(&crate_dir, &modules, true)?;
    generate_api(&crate_dir, &modules, false)?;
    println!("Generated API files in {}", crate_dir.display());
    Ok(())
}
