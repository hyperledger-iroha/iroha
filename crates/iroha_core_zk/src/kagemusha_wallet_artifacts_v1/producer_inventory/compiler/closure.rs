//! Full unsigned catalog assembly after exact source dependency closure.

use super::*;

fn require_receipt(finality: &FinalityV1, expected: [BlobV1; 2]) -> Result<(), CompilationErrorV1> {
    use iroha_kagemusha_proof::finality::native::{ArtifactId, Composition, NodeId};
    let id = ArtifactId::Wrapper(NodeId::Composition(Composition::Receipt));
    let mut selected = None;
    for record in &finality.originals {
        if record
            .matches_identity(&id)
            .map_err(|_| CompilationErrorV1::Closure)?
        {
            if selected.replace(record).is_some() {
                return Err(CompilationErrorV1::Closure);
            }
        }
    }
    let record = selected.ok_or(CompilationErrorV1::Closure)?;
    if expected
        .iter()
        .enumerate()
        .any(|(i, blob)| record.lengths[i] != blob.bytes || record.sha256[i] != blob.sha256)
    {
        return Err(CompilationErrorV1::Closure);
    }
    Ok(())
}

fn intern(originals: &mut Vec<OriginalV1>, item: OriginalV1) -> Result<u32, CompilationErrorV1> {
    let i = if let Some(i) = originals.iter().position(|old| *old == item) {
        i
    } else {
        if originals.len() >= ARTIFACT_MAX_COUNT_V1 {
            return Err(CompilationErrorV1::Closure);
        }
        originals.push(item);
        originals.len() - 1
    };
    u32::try_from(i).map_err(|_| CompilationErrorV1::Closure)
}

impl OfflineCompilerV1<'_> {
    /// Assemble the complete unsigned inventory only after every logical route
    /// closes under the exact newly compiled Omega key and ordered terminal catalog.
    /// Signing, independent genesis authentication, complete finality qualification,
    /// measured proof bounds and authenticated installation remain separate.
    /// # Errors
    /// Missing/reordered route, another scope/dependency, changed finality anchor,
    /// nonuniform terminal catalog or invalid/over-capacity canonical metadata.
    pub fn inventory(
        &self,
        sigmas: &CompiledSigmasV1,
        operations: &[CompiledOperationV1],
        omega: &CompiledOmegaV1,
        finality: FinalityV1,
    ) -> Result<ProducerInventoryV1, CompilationErrorV1> {
        let routes = compiled_routes();
        if operations.len() != routes.len() {
            return Err(CompilationErrorV1::Closure);
        }
        let mut originals = Vec::new();
        let mut sigma = [0; 16];
        for (out, key) in sigma.iter_mut().zip(sigmas.keys()) {
            *out = intern(&mut originals, key.original)?;
        }
        let mut programs = Vec::new();
        let mut dispatch = Vec::new();
        let mut terminals = Vec::new();
        let mut terminal_keys: Vec<&KeyArtifact<Eq>> = Vec::new();
        let anchor = iroha_kagemusha_proof::finality::history::HistoryAnchor {
            network: finality.network,
            instance: finality.instance,
            initial_context: finality.initial_context,
            initial_epoch: finality.initial_epoch,
            parameters: finality.parameters,
        };
        for (operation, route) in operations.iter().zip(routes) {
            if operation.scope != self.scope
                || operation.route != route
                || if route.variant == Variant::Bootstrap {
                    operation.predecessor.is_some()
                } else {
                    operation
                        .predecessor
                        .as_ref()
                        .is_none_or(|k| !equal(k, &omega.key.metadata))
                }
                || (route.variant == Variant::Load && operation.anchor != Some(anchor))
                || (route.variant != Variant::Load && operation.anchor.is_some())
                || (route.variant != Variant::Load && operation.receipt.is_some())
            {
                return Err(CompilationErrorV1::Closure);
            }
            if route.variant == Variant::Load {
                require_receipt(
                    &finality,
                    operation.receipt.ok_or(CompilationErrorV1::Closure)?,
                )?;
            }
            let record = OperationV1 {
                variant: u8::try_from(
                    Variant::ALL
                        .iter()
                        .position(|v| *v == route.variant)
                        .ok_or(CompilationErrorV1::Closure)?
                        + 1,
                )
                .map_err(|_| CompilationErrorV1::Closure)?,
                own_class: operation.own.clone(),
                incoming_class: operation.incoming.clone(),
                context: operation.context.clone(),
                q: operation
                    .q
                    .iter()
                    .map(|k| intern(&mut originals, k.original))
                    .collect::<Result<_, _>>()?,
                a: operation
                    .a
                    .iter()
                    .map(|k| intern(&mut originals, k.original))
                    .collect::<Result<_, _>>()?,
                w: operation
                    .w
                    .iter()
                    .map(|k| intern(&mut originals, k.original))
                    .collect::<Result<_, _>>()?,
            };
            let index = if let Some(index) = programs.iter().position(|old| *old == record) {
                index
            } else {
                let terminal = &operation.terminal().metadata;
                if !terminal_keys.iter().any(|old| equal(old, terminal)) {
                    if terminal_keys
                        .first()
                        .is_some_and(|first| first.binding() != terminal.binding())
                    {
                        return Err(CompilationErrorV1::Closure);
                    }
                    terminal_keys.push(terminal);
                    terminals.push(*record.a.last().ok_or(CompilationErrorV1::Closure)?);
                }
                programs.push(record);
                programs.len() - 1
            };
            dispatch.push(u32::try_from(index).map_err(|_| CompilationErrorV1::Closure)?);
        }
        if terminal_keys.len() != omega.terminals.len()
            || !terminal_keys
                .iter()
                .zip(&omega.terminals)
                .all(|(a, b)| equal(a, b))
        {
            return Err(CompilationErrorV1::Closure);
        }
        let omega = intern(&mut originals, omega.key.original)?;
        let inventory = ProducerInventoryV1 {
            version: 1,
            native_profile: artifact_digest(b"native-profile", &native_profile_transcript_v1()?),
            originals,
            sigma,
            operations: programs,
            routes: dispatch,
            terminals,
            omega,
            finality,
        };
        inventory.validate()?;
        Ok(inventory)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_kagemusha_proof::finality::{
        catalog::{ArtifactSink, DirectoryCatalog},
        continuity::tree::OriginalBytes,
        native::{ArtifactId, Composition, ImportLimits, NodeId},
    };

    #[test]
    fn receipt_wrapper_original_is_unique_and_matches_both_exact_metadata_addresses() {
        let directory = tempfile::tempdir().unwrap();
        let limits = ImportLimits {
            key: super::super::tests::limits(),
            maximum_artifacts: 8,
            maximum_original_bytes: 1024,
        };
        let mut store =
            DirectoryCatalog::create(directory.path().join("originals"), limits).unwrap();
        // These are metadata-comparison fixtures; no proof/source importer accepts them.
        let original = OriginalBytes {
            descriptor: vec![1, 2],
            verifying_key: vec![3, 4],
            proving_key: vec![5],
        };
        let expected = [
            BlobV1::of(&original.descriptor),
            BlobV1::of(&original.verifying_key),
        ];
        let source = ArtifactId::Source(NodeId::Composition(Composition::Receipt));
        let wrapper = ArtifactId::Wrapper(NodeId::Composition(Composition::Receipt));
        store.store(&source, &original).unwrap();
        store.store(&wrapper, &original).unwrap();
        let bytes = store.inventory().unwrap();
        let records: Vec<ArtifactRecord> = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .unwrap();
        let finality = FinalityV1 {
            network: [1; 32],
            instance: [2; 32],
            initial_context: [3; 32],
            initial_epoch: 0,
            parameters: [1; 6],
            originals: records,
        };
        require_receipt(&finality, expected).unwrap();
        let index = finality
            .originals
            .iter()
            .position(|r| r.matches_identity(&wrapper).unwrap())
            .unwrap();
        for field in 0..2 {
            let mut changed = finality.clone();
            changed.originals[index].lengths[field] += 1;
            assert!(require_receipt(&changed, expected).is_err());
            let mut changed = finality.clone();
            changed.originals[index].sha256[field][0] ^= 1;
            assert!(require_receipt(&changed, expected).is_err());
        }
        let mut changed = finality.clone();
        changed.originals.remove(index);
        assert!(
            require_receipt(&changed, expected).is_err(),
            "matching source is not the wrapper"
        );
        let mut changed = finality.clone();
        changed.originals.push(finality.originals[index].clone());
        assert!(require_receipt(&changed, expected).is_err());
        let mut changed = finality;
        changed.originals[index].name.push(0);
        assert!(require_receipt(&changed, expected).is_err());
    }
}
