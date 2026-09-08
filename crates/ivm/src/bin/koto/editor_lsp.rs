//! LSP projection of the compiler-owned semantic editor snapshot.
use super::*;
use ivm::kotodama::{
    editor::EditorSnapshot,
    source::{SourceId, SourceRange},
};

pub(super) struct Workspace {
    snapshot: EditorSnapshot,
    uris: BTreeMap<SourceId, String>,
    manifest: Option<ivm::kotodama::driver::ProjectManifestSource>,
    open_uris: HashSet<String>,
    versions: HashMap<String, i64>,
    rename_error: Option<String>,
}
impl Workspace {
    pub(super) fn new(
        documents: &HashMap<String, String>,
        project: Option<&LoadedSourceProject>,
        uri: &str,
        zk: bool,
    ) -> Self {
        if let Some(project) = project
            && let Some((graph, source_uris, _, manifest)) =
                lsp_project_with_open_overlays(project, documents)
            && source_uris.values().any(|candidate| candidate == uri)
        {
            let snapshot = EditorSnapshot::project(&graph, zk);
            let uris = snapshot
                .sources()
                .filter_map(|source| {
                    let key = ProjectSourceKey {
                        package_identity: source.package_identity().map(ToOwned::to_owned),
                        source_name: source.name().to_owned(),
                    };
                    source_uris.get(&key).map(|uri| (source.id(), uri.clone()))
                })
                .collect();
            return Self {
                snapshot,
                uris,
                manifest,
                open_uris: documents.keys().cloned().collect(),
                versions: HashMap::new(),
                rename_error: None,
            };
        }
        let snapshot =
            EditorSnapshot::single(uri, documents.get(uri).map_or("", String::as_str), zk);
        Self {
            snapshot,
            uris: BTreeMap::from([(SourceId(0), uri.to_owned())]),
            manifest: None,
            open_uris: documents.keys().cloned().collect(),
            versions: HashMap::new(),
            rename_error: project
                .filter(|project| {
                    project
                        .source_paths
                        .values()
                        .any(|path| lsp_file_uri_path(uri).as_ref() == Some(path))
                })
                .map(|_| "Rename requires a valid current project manifest.".into()),
        }
    }
    pub(super) fn with_versions(mut self, versions: &HashMap<String, i64>) -> Self {
        self.versions = versions
            .iter()
            .filter(|(uri, _)| self.open_uris.contains(*uri))
            .map(|(uri, version)| (uri.clone(), *version))
            .collect();
        self
    }
    fn manifest_uri(&self) -> Option<String> {
        let path = self.manifest.as_ref()?.path();
        self.open_uris
            .iter()
            .find(|uri| lsp_file_uri_path(uri).as_deref() == Some(path))
            .cloned()
            .or_else(|| lsp_path_file_uri(path))
    }
    fn rename_plan(
        &self,
        source: SourceId,
        offset: u32,
        name: &str,
    ) -> Result<ivm::kotodama::editor::EditorRename, String> {
        if let Some(error) = &self.rename_error {
            return Err(error.clone());
        }
        let plan = self.snapshot.rename(source, offset, name)?;
        if self.manifest.is_none()
            && plan.sources.iter().any(|range| {
                self.snapshot
                    .source(range.source)
                    .is_some_and(|file| file.package_identity().is_some())
            })
        {
            return Err("Package rename requires an owned local export manifest; external locked dependencies are immutable.".into());
        }
        if !plan.exports.is_empty() {
            let manifest = self.manifest.as_ref().ok_or("Export rename requires an owned local export manifest; external locked dependencies are immutable.")?;
            for export in &plan.exports {
                manifest
                    .export_range(&export.package, &export.old_name)
                    .ok_or("Rename export is absent from the exact manifest snapshot.")?;
            }
        }
        // Open buffers are immutable/versioned in this workspace; unopened inputs must still
        // match the captured graph before returning any source or metadata edit.
        for (id, uri) in &self.uris {
            if self.open_uris.contains(uri) {
                continue;
            }
            let path = lsp_file_uri_path(uri).ok_or("Rename source is no longer available.")?;
            if read_source_file(&path).map_err(|error| error.to_string())?
                != self
                    .snapshot
                    .source(*id)
                    .ok_or("Rename source is unavailable.")?
                    .text()
            {
                return Err(
                    "Rename source changed on disk; reload the project before renaming.".into(),
                );
            }
        }
        if let Some(manifest) = &self.manifest {
            let uri = self
                .manifest_uri()
                .ok_or("Rename manifest URI is unavailable.")?;
            if !self.open_uris.contains(&uri)
                && read_source_file(manifest.path()).map_err(|error| error.to_string())?
                    != manifest.text()
            {
                return Err(
                    "Rename manifest changed on disk; reload the project before renaming.".into(),
                );
            }
        }
        Ok(plan)
    }
    fn location(&self, source: SourceRange) -> Option<norito::json::Value> {
        Some(json_object(vec![
            (
                "uri",
                norito::json::Value::from(self.uris.get(&source.source)?.as_str()),
            ),
            (
                "range",
                lsp_text_range(self.snapshot.source(source.source)?.text(), source.range),
            ),
        ]))
    }
    fn position(&self, message: &norito::json::Value) -> Option<(SourceId, u32)> {
        let uri = message.pointer("/params/textDocument/uri")?.as_str()?;
        let source = *self
            .uris
            .iter()
            .find(|(_, candidate)| candidate.as_str() == uri)?
            .0;
        let line = message.pointer("/params/position/line")?.as_u64()?;
        let character = message.pointer("/params/position/character")?.as_u64()?;
        Some((
            source,
            utf16_offset(self.snapshot.source(source)?.text(), line, character)?,
        ))
    }
    pub(super) fn response(
        &self,
        method: &str,
        message: &norito::json::Value,
    ) -> Result<norito::json::Value, String> {
        let Some((source, offset)) = self.position(message) else {
            return Ok(norito::json::Value::Null);
        };
        Ok(match method {
            "textDocument/completion" => {
                let items = self
                    .snapshot
                    .completions(source, offset)
                    .into_iter()
                    .map(|completion| {
                        json_object(vec![
                            ("label", completion.label.into()),
                            ("kind", completion.kind.into()),
                            ("detail", completion.detail.into()),
                            ("insertText", completion.insert_text.into()),
                            (
                                "insertTextFormat",
                                norito::json::Value::from(if completion.snippet {
                                    2_u64
                                } else {
                                    1_u64
                                }),
                            ),
                            (
                                "documentation",
                                json_object(vec![
                                    ("kind", "markdown".into()),
                                    ("value", completion.documentation.into()),
                                ]),
                            ),
                        ])
                    })
                    .collect();
                json_object(vec![
                    ("isIncomplete", (!self.snapshot.is_complete()).into()),
                    ("items", norito::json::Value::Array(items)),
                ])
            }
            "textDocument/definition" => self
                .snapshot
                .definition(source, offset)
                .and_then(|definition| self.location(definition.source))
                .unwrap_or(norito::json::Value::Null),
            "textDocument/references" => norito::json::Value::Array(
                self.snapshot
                    .references(
                        source,
                        offset,
                        message
                            .pointer("/params/context/includeDeclaration")
                            .and_then(norito::json::Value::as_bool)
                            .unwrap_or(false),
                    )
                    .into_iter()
                    .filter_map(|source| self.location(source))
                    .collect(),
            ),
            "textDocument/hover" => self
                .snapshot
                .hover(source, offset)
                .map(|(detail, documentation)| {
                    json_object(vec![(
                        "contents",
                        json_object(vec![
                            ("kind", "markdown".into()),
                            (
                                "value",
                                format!("```kotodama\n{detail}\n```\n{documentation}").into(),
                            ),
                        ]),
                    )])
                })
                .unwrap_or(norito::json::Value::Null),
            "textDocument/signatureHelp" => self
                .snapshot
                .signature_help(source, offset)
                .map(|(signature, active)| {
                    let active = active.min(signature.parameters.len().saturating_sub(1));
                    json_object(vec![
                        (
                            "signatures",
                            norito::json::Value::Array(vec![json_object(vec![
                                ("label", signature.label().into()),
                                ("documentation", signature.documentation.clone().into()),
                                (
                                    "parameters",
                                    norito::json::Value::Array(
                                        signature
                                            .parameters
                                            .iter()
                                            .map(|parameter| {
                                                json_object(vec![(
                                                    "label",
                                                    format!(
                                                        "{} {}{}",
                                                        parameter.ty,
                                                        if parameter.named { "" } else { "_ " },
                                                        parameter.name
                                                    )
                                                    .into(),
                                                )])
                                            })
                                            .collect(),
                                    ),
                                ),
                            ])]),
                        ),
                        ("activeSignature", 0_u64.into()),
                        ("activeParameter", (active as u64).into()),
                    ])
                })
                .unwrap_or(norito::json::Value::Null),
            "textDocument/prepareRename" => {
                let definition = self
                    .snapshot
                    .definition(source, offset)
                    .ok_or("No resolved declaration at this position.")?;
                self.rename_plan(source, offset, &definition.name)?;
                let range = self
                    .snapshot
                    .references(source, offset, true)
                    .into_iter()
                    .find(|range| {
                        range.source == source
                            && range.range.start <= offset
                            && offset < range.range.end
                    })
                    .ok_or("No exact identifier range at this position.")?;
                json_object(vec![
                    (
                        "range",
                        lsp_text_range(
                            self.snapshot.source(source).expect("source").text(),
                            range.range,
                        ),
                    ),
                    ("placeholder", definition.name.clone().into()),
                ])
            }
            "textDocument/rename" => {
                let name = message
                    .pointer("/params/newName")
                    .and_then(norito::json::Value::as_str)
                    .ok_or("Rename requires a newName.")?;
                let plan = self.rename_plan(source, offset, name)?;
                let mut changes = BTreeMap::<String, Vec<norito::json::Value>>::new();
                for range in plan.sources {
                    let uri = self
                        .uris
                        .get(&range.source)
                        .ok_or("Rename source is not in the exact project graph.")?;
                    let file = self
                        .snapshot
                        .source(range.source)
                        .ok_or("Rename source is unavailable.")?;
                    changes
                        .entry(uri.clone())
                        .or_default()
                        .push(json_object(vec![
                            ("range", lsp_text_range(file.text(), range.range)),
                            ("newText", name.into()),
                        ]));
                }
                for export in plan.exports {
                    let manifest = self
                        .manifest
                        .as_ref()
                        .ok_or("Owned export manifest is unavailable.")?;
                    let range = manifest
                        .export_range(&export.package, &export.old_name)
                        .ok_or("Exact export token is unavailable.")?;
                    changes
                        .entry(
                            self.manifest_uri()
                                .ok_or("Export manifest URI is unavailable.")?,
                        )
                        .or_default()
                        .push(json_object(vec![
                            ("range", lsp_text_range(manifest.text(), range)),
                            (
                                "newText",
                                norito::json::to_string(&export.new_name)
                                    .map_err(|error| error.to_string())?
                                    .into(),
                            ),
                        ]));
                }
                json_object(vec![(
                    "documentChanges",
                    norito::json::Value::Array(
                        changes
                            .into_iter()
                            .map(|(uri, edits)| {
                                let version = self
                                    .versions
                                    .get(&uri)
                                    .copied()
                                    .map(norito::json::Value::from)
                                    .unwrap_or(norito::json::Value::Null);
                                json_object(vec![
                                    (
                                        "textDocument",
                                        json_object(vec![
                                            ("uri", uri.into()),
                                            ("version", version),
                                        ]),
                                    ),
                                    ("edits", norito::json::Value::Array(edits)),
                                ])
                            })
                            .collect(),
                    ),
                )])
            }
            _ => norito::json::Value::Null,
        })
    }
}
fn utf16_offset(source: &str, line: u64, character: u64) -> Option<u32> {
    let line = usize::try_from(line).ok()?;
    let character = usize::try_from(character).ok()?;
    let start = if line == 0 {
        0
    } else {
        source.match_indices('\n').nth(line - 1)?.0 + 1
    };
    let text = source[start..].split('\n').next()?;
    let mut utf16 = 0;
    for (byte, ch) in text.char_indices() {
        if utf16 == character {
            return u32::try_from(start + byte).ok();
        }
        utf16 += ch.len_utf16();
        if utf16 > character {
            return None;
        }
    }
    (utf16 == character)
        .then(|| u32::try_from(start + text.len()).ok())
        .flatten()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn positions_preserve_japanese_and_reject_half_surrogates() {
        let text = "金庫😀x\n次";
        assert_eq!(utf16_offset(text, 0, 2), Some(6));
        assert_eq!(utf16_offset(text, 0, 3), None);
        assert_eq!(utf16_offset(text, 0, 4), Some(10));
        assert_eq!(utf16_offset(text, 1, 1), Some(text.len() as u32));
    }
    #[test]
    fn semantic_responses_share_identity_ranges_and_named_argument_labels() {
        let uri = "file:///editor.ko";
        let text = "module Editor { fn target(int _ first, int second) -> int { first + second } fn run() -> int { target(1, second: 2) } }";
        let documents = HashMap::from([(uri.to_owned(), text.to_owned())]);
        let workspace = Workspace::new(&documents, None, uri, false);
        let position = text.find("second: 2").unwrap();
        let request = norito::json!({"params": {"textDocument": {"uri": uri}, "position": {"line": 0, "character": position}, "context": {"includeDeclaration": true}, "newName": "increment"}});
        let definition = workspace
            .response("textDocument/definition", &request)
            .unwrap();
        assert_eq!(
            definition
                .pointer("/uri")
                .and_then(norito::json::Value::as_str),
            Some(uri)
        );
        assert_eq!(
            definition
                .pointer("/range/start/character")
                .and_then(norito::json::Value::as_u64),
            Some(text.find("second)").unwrap() as u64)
        );
        let references = workspace
            .response("textDocument/references", &request)
            .unwrap();
        assert_eq!(references.as_array().unwrap().len(), 3);
        let renamed = workspace.response("textDocument/rename", &request).unwrap();
        let edits = renamed
            .pointer("/documentChanges/0/edits")
            .unwrap()
            .as_array()
            .unwrap();
        assert_eq!(edits.len(), 3);
        assert!(edits.iter().all(|edit| {
            edit.pointer("/newText")
                .and_then(norito::json::Value::as_str)
                == Some("increment")
        }));
        let active_position = position + 9;
        let active = norito::json!({"params": {"textDocument": {"uri": uri}, "position": {"line": 0, "character": active_position}}});
        let signature = workspace
            .response("textDocument/signatureHelp", &active)
            .unwrap();
        assert_eq!(
            signature
                .pointer("/activeParameter")
                .and_then(norito::json::Value::as_u64),
            Some(1)
        );
        assert_eq!(
            signature
                .pointer("/signatures/0/label")
                .and_then(norito::json::Value::as_str),
            Some("target(int _ first, int second) -> int")
        );
    }
    #[test]
    fn owned_export_rename_updates_only_exact_versioned_source_and_manifest_tokens() {
        let root = std::env::temp_dir().join(format!(
            "kotodama-export-rename-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let app = root.join("金庫😀.ko");
        let module = root.join("values.ko");
        let manifest = root.join("kotodama.project.json");
        let app_text = "seiyaku App { /* 金庫😀 */ view fn run() -> int { value::value() } }";
        let module_text =
            "module Values { fn value() -> int { 7 } fn other() -> string { \"value\" } }";
        let manifest_text = r#"{
            "version": 1, "root": "金庫😀.ko",
            "imports": [{"alias": "value", "package": "test/value@1"}],
            "packages": [{"identity": "test/value@1", "modules": ["values.ko"],
                "exports": ["v\u0061lue"], "imports": []}]
        }"#;
        std::fs::write(&app, app_text).unwrap();
        std::fs::write(&module, module_text).unwrap();
        std::fs::write(&manifest, manifest_text).unwrap();
        let project = load_source_project_manifest(&manifest).unwrap();
        let app_uri = lsp_path_file_uri(&app.canonicalize().unwrap()).unwrap();
        let module_uri = lsp_path_file_uri(&module.canonicalize().unwrap()).unwrap();
        let manifest_uri = lsp_path_file_uri(&manifest.canonicalize().unwrap()).unwrap();
        let manifest_overlay = manifest_text.replace("\"exports\":", "\"exports\" :");
        let documents = HashMap::from([
            (app_uri.clone(), app_text.to_owned()),
            (manifest_uri.clone(), manifest_overlay.clone()),
        ]);
        let versions = HashMap::from([(app_uri.clone(), 7), (manifest_uri.clone(), 4)]);
        let workspace =
            Workspace::new(&documents, Some(&project), &app_uri, false).with_versions(&versions);
        let offset = app_text.rfind("value()").unwrap();
        let character = app_text[..offset].encode_utf16().count();
        let request = norito::json!({"params": {"textDocument": {"uri": (app_uri.clone())},
            "position": {"line": 0, "character": character}, "newName": "renamed"}});
        workspace
            .response("textDocument/prepareRename", &request)
            .unwrap();
        let response = workspace.response("textDocument/rename", &request).unwrap();
        let edits = response.get("documentChanges").unwrap().as_array().unwrap();
        assert_eq!(edits.len(), 3);
        let mut rewritten = BTreeMap::from([
            (app_uri.clone(), app_text.to_owned()),
            (module_uri.clone(), module_text.to_owned()),
            (manifest_uri.clone(), manifest_overlay.clone()),
        ]);
        for document in edits {
            let uri = document
                .pointer("/textDocument/uri")
                .unwrap()
                .as_str()
                .unwrap();
            let version = document.pointer("/textDocument/version").unwrap();
            assert_eq!(version.as_i64(), versions.get(uri).copied());
            let text = rewritten.get_mut(uri).unwrap();
            let mut replacements = document
                .get("edits")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|edit| {
                    let start = edit.pointer("/range/start").unwrap();
                    let end = edit.pointer("/range/end").unwrap();
                    let start = utf16_offset(
                        text,
                        start.get("line").unwrap().as_u64().unwrap(),
                        start.get("character").unwrap().as_u64().unwrap(),
                    )
                    .unwrap() as usize;
                    let end = utf16_offset(
                        text,
                        end.get("line").unwrap().as_u64().unwrap(),
                        end.get("character").unwrap().as_u64().unwrap(),
                    )
                    .unwrap() as usize;
                    (
                        start,
                        end,
                        edit.get("newText").unwrap().as_str().unwrap().to_owned(),
                    )
                })
                .collect::<Vec<_>>();
            if uri == manifest_uri {
                assert_eq!(replacements.len(), 1);
                let (start, end, replacement) = &replacements[0];
                assert_eq!(&text[*start..*end], "\"v\\u0061lue\"");
                assert_eq!(replacement, "\"renamed\"");
            }
            replacements.sort_by_key(|(start, _, _)| std::cmp::Reverse(*start));
            for (start, end, replacement) in replacements {
                text.replace_range(start..end, &replacement);
            }
        }
        assert!(rewritten[&app_uri].contains("value::renamed()"));
        assert!(rewritten[&module_uri].contains("\"value\""));
        assert!(rewritten[&manifest_uri].contains("\"alias\": \"value\""));
        assert!(rewritten[&manifest_uri].contains("test/value@1"));
        let mut external = project.clone();
        external.manifest = None;
        let external = Workspace::new(&documents, Some(&external), &app_uri, false);
        assert!(
            external
                .response("textDocument/rename", &request)
                .unwrap_err()
                .contains("immutable")
        );
        let mut invalid = documents.clone();
        invalid.insert(manifest_uri.clone(), "{".into());
        assert!(
            Workspace::new(&invalid, Some(&project), &app_uri, false)
                .response("textDocument/rename", &request)
                .is_err()
        );
        std::fs::write(&module, format!("{module_text} // newer disk version")).unwrap();
        assert!(
            workspace
                .response("textDocument/rename", &request)
                .unwrap_err()
                .contains("changed on disk")
        );
        std::fs::write(&module, module_text).unwrap();
        let closed = Workspace::new(
            &HashMap::from([(app_uri.clone(), app_text.to_owned())]),
            Some(&project),
            &app_uri,
            false,
        );
        std::fs::write(&manifest, format!("{manifest_text}\n")).unwrap();
        assert!(
            closed
                .response("textDocument/rename", &request)
                .unwrap_err()
                .contains("manifest changed")
        );
        std::fs::write(&app, &rewritten[&app_uri]).unwrap();
        std::fs::write(&module, &rewritten[&module_uri]).unwrap();
        let updated = ivm::kotodama::driver::load_source_project_manifest_with_text(
            &manifest,
            &rewritten[&manifest_uri],
        )
        .unwrap();
        assert!(EditorSnapshot::project(&updated.graph, false).is_complete());
        std::fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn incomplete_documents_do_not_publish_rename_edits() {
        let uri = "file:///incomplete.ko";
        let text = "module Incomplete { fn run(int _ value) -> int { value + } }";
        let documents = HashMap::from([(uri.to_owned(), text.to_owned())]);
        let workspace = Workspace::new(&documents, None, uri, false);
        let position = text.find("value +").unwrap();
        let request = norito::json!({"params": {"textDocument": {"uri": uri}, "position": {"line": 0, "character": position}, "newName": "amount"}});
        assert!(workspace.response("textDocument/rename", &request).is_err());
    }
}
