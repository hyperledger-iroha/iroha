//! Incomplete source keeps receiver facts from its exact locked package graph.
use super::*;
use crate::linker::{SourceModuleUnit, SourcePackageUnit};

fn request(member: &str, ty: &str) -> SourceLinkRequest {
    SourceLinkRequest {
        root: SourceModuleUnit {
            source_name: "app.ko".into(),
            source: format!("誓約 App {{ view fn read(rows::{ty} row) -> int {{ row.{member} }} }}"),
        },
        imports: vec![ImportBinding {
            alias: "rows".into(),
            package: "local/rows@1".into(),
        }],
        packages: vec![SourcePackageUnit {
            identity: "local/rows@1".into(),
            modules: vec![SourceModuleUnit {
                source_name: "model.ko".into(),
                source: "module Rows { struct Row { int amount; string memo; } struct Hidden { int secret; } }".into(),
            }],
            exports: BTreeSet::from(["Row".into()]),
            imports: vec![],
        }],
    }
}

fn assert_receiver_fields(request: &SourceLinkRequest, package: Option<&str>, source_name: &str) {
    let snapshot = EditorSnapshot::project(request, false);
    assert!(!snapshot.is_complete());
    let source = snapshot
        .sources()
        .find(|source| source.package_identity() == package && source.name() == source_name)
        .expect("source in the locked graph");
    let original_text = source.text().to_owned();
    let member_start = original_text.find("row.").expect("receiver") + 4;
    let member_len = original_text[member_start..]
        .bytes()
        .take_while(u8::is_ascii_alphanumeric)
        .count();
    let offset = (member_start + member_len) as u32;
    let candidates = snapshot.completions(source.id(), offset);
    assert_eq!(
        candidates
            .iter()
            .map(|candidate| candidate.label.as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["amount", "memo"]),
        "completion must use the imported receiver, not globals or hidden exports"
    );
    assert_eq!(
        candidates
            .iter()
            .find(|candidate| candidate.label == "amount")
            .unwrap()
            .detail,
        "int"
    );
    assert_eq!(
        candidates
            .iter()
            .find(|candidate| candidate.label == "memo")
            .unwrap()
            .detail,
        "string"
    );
    assert!(snapshot.rename(source.id(), offset - 2, "other").is_err());
    assert!(
        !snapshot.is_complete(),
        "query must not publish the recovery projection"
    );
    assert_eq!(snapshot.source(source.id()).unwrap().text(), original_text);
    assert!(
        ModuleBuildGraph::default()
            .link(request.clone(), LinkerOptions::default())
            .is_err(),
        "completion must not broaden compiler acceptance"
    );
}

#[test]
fn incomplete_root_receivers_retain_locked_struct_fields() {
    for member in ["", "am"] {
        assert_receiver_fields(&request(member, "Row"), None, "app.ko");
    }
}

#[test]
fn incomplete_graph_uses_the_linkers_canonical_source_names() {
    let mut request = request("", "Row");
    request.root.source_name = "src\\.\\app.ko".into();
    request.packages[0].modules[0].source_name = "src/./model.ko".into();
    assert_receiver_fields(&request, None, "src/app.ko");
}

#[test]
fn incomplete_package_receivers_use_their_own_imports_and_source_identity() {
    for member in ["", "am"] {
        let mut request = request(member, "Row");
        request.root.source = "seiyaku App { view fn read() -> int { 7 } }".into();
        request.root.source_name = "model.ko".into();
        request.imports = vec![ImportBinding {
            alias: "adapter".into(),
            package: "local/adapter@1".into(),
        }];
        request.packages.push(SourcePackageUnit {
            identity: "local/adapter@1".into(),
            modules: vec![SourceModuleUnit {
                source_name: "model.ko".into(),
                source: format!(
                    "module Adapter {{ fn inspect(rows::Row row) -> int {{ row.{member} }} }}"
                ),
            }],
            exports: BTreeSet::from(["inspect".into()]),
            imports: vec![ImportBinding {
                alias: "rows".into(),
                package: "local/rows@1".into(),
            }],
        });
        assert_receiver_fields(&request, Some("local/adapter@1"), "model.ko");
    }
}

#[test]
fn incomplete_receivers_do_not_reveal_unexported_package_types() {
    let request = request("", "Hidden");
    let snapshot = EditorSnapshot::project(&request, false);
    let source = snapshot
        .sources()
        .find(|source| source.package_identity().is_none())
        .unwrap();
    let offset = source.text().find("row.").unwrap() as u32 + 4;
    assert!(snapshot.completions(source.id(), offset).is_empty());
    assert!(!snapshot.is_complete());
}
