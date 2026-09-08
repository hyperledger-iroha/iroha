//! Partial editor facts use the same locked type and callable imports as strict linking.
use super::*;
use crate::{
    resolved::BindingId,
    semantic::{SemanticContext, TypedHirNode},
};

/// Successfully resolved facts; no recovered program can reach code generation.
pub(crate) struct EditorModuleFacts {
    /// Call signatures resolved through the graph's explicit export tables.
    pub(crate) signatures: BTreeMap<String, FunctionSignature>,
    /// Inferred types keyed by the original resolver binding identities.
    pub(crate) bindings: BTreeMap<BindingId, Type>,
    /// Successfully typed source expressions, including those preceding a body error.
    pub(crate) nodes: Vec<TypedHirNode>,
}

impl ModuleBuildGraph {
    /// Apply the normal graph bounds and canonical source names before assigning editor identities.
    pub(crate) fn editor_request(
        request: &SourceLinkRequest,
    ) -> Result<SourceLinkRequest, SourceGraphError> {
        let names = validate_source_link_request(request)?;
        let mut request = request.clone();
        canonicalize_source_link_request(&mut request, names);
        Ok(request)
    }
}

impl TypedLinker {
    /// Retain local facts after a body error while preserving locked import authority.
    pub(crate) fn analyze_editor_graph(
        &self,
        mut request: LinkRequest,
    ) -> Result<BTreeMap<SourceId, EditorModuleFacts>, LinkError> {
        validate_linker_options(self.options)?;
        if request.root.ast().unit.kind != SourceUnitKind::Seiyaku {
            return Err(LinkError::RootMustBeSeiyaku {
                source: request.root.source_name,
            });
        }
        validate_program_symbols(&request.root)?;
        let packages = resolve_packages(self.options, &mut request.packages)?;
        let indexes = packages
            .iter()
            .enumerate()
            .map(|(index, package)| (package.identity.clone(), index))
            .collect();
        let imports = resolve_imports("root", &request.imports, &indexes)?;
        let analyze =
            |module: &ModuleUnit, imports: &BTreeMap<String, usize>, package: Option<&str>| {
                let semantic = SemanticContext::with_capabilities(
                    self.options.zk_enabled,
                    self.options.test_builtins_enabled,
                );
                if let Some(package) = package {
                    semantic.set_package_identity(package.to_owned());
                }
                let types = external_types(imports, &packages);
                let signatures = semantic
                    .resolve_resolved_function_signatures_with_types(&module.program, &types)
                    .unwrap_or_default();
                let (_, bindings, nodes) = semantic.analyze_editor(
                    &module.program,
                    external_signatures(imports, &packages),
                    types,
                );
                (
                    module.program.source_file().id(),
                    EditorModuleFacts {
                        signatures,
                        bindings,
                        nodes,
                    },
                )
            };
        let mut facts = BTreeMap::from([analyze(&request.root, &imports, None)]);
        for package in &packages {
            for module in &package.modules {
                let (id, module_facts) =
                    analyze(module.source, &package.imports, Some(&package.identity));
                facts.insert(id, module_facts);
            }
        }
        Ok(facts)
    }
}
