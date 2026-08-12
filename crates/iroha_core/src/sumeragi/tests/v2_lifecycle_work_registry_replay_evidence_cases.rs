    #[test]
    fn certified_pipeline_replay_evidence_is_retained_by_every_closed_carrier() {
        let source = include_str!("v2_lifecycle_work_registry.rs");
        let production = source
            .split("\n#[cfg(test)]\nmod tests {")
            .next()
            .expect("registry has one production prefix");

        let fetch = production
            .split("struct CertifiedFetchCompletion {")
            .nth(1)
            .expect("certified Fetch completion has one declaration")
            .split("/// Closed durable form of one admitted `StoreBody` effect.")
            .next()
            .expect("Store carrier follows certified Fetch completion");
        for required in [
            "replay_evidence: CertifiedFetchReplayEvidenceV1",
            "dequeued_certified_response(&self.dequeued)",
            ".exactly_matches_fetch(",
        ] {
            assert!(
                fetch.contains(required),
                "certified Fetch carrier omitted {required}"
            );
        }

        let store = production
            .split("struct DurableStoreBody {")
            .nth(1)
            .expect("durable Store has one declaration")
            .split("/// Closed durable form of one admitted `ValidateBody` effect.")
            .next()
            .expect("Validate carrier follows Store");
        for required in [
            "replay_evidence: CertifiedStoreReplayEvidenceV1",
            ".exactly_matches_store(&self.effect, &self.durable_receipt)",
        ] {
            assert!(store.contains(required), "Store carrier omitted {required}");
        }

        let validate = production
            .split("struct DurableValidateBody {")
            .nth(1)
            .expect("durable Validate has one declaration")
            .split("/// Same-address closed result of one completed durable body validation.")
            .next()
            .expect("Validate completion follows its carrier");
        for required in [
            "replay_evidence: DurableValidateReplayEvidenceV1",
            ".exactly_matches_validate_pending(",
            "&self.effect,\n                &self.durable_receipt,\n                &self.pending",
        ] {
            assert!(
                validate.contains(required),
                "Validate carrier omitted {required}"
            );
        }
        let completion = production
            .split("struct DurableValidateCompletion {")
            .nth(1)
            .expect("durable Validate completion has one declaration")
            .split("impl DurableValidateCompletion")
            .next()
            .expect("Validate completion validation follows its declaration");
        assert!(completion.contains("incumbent: DurableValidateBody"));

        let fetch_successor = production
            .split("pub(super) struct PreparedCertifiedFetchStoreSuccessor<'a> {")
            .nth(1)
            .expect("Fetch-to-Store successor has one declaration")
            .split("/// Borrow-bound registry conversion prepared")
            .next()
            .expect("certified Fetch completion token follows its successor");
        assert!(fetch_successor.contains("_replay_evidence: CertifiedStoreReplayEvidenceV1"));
        let validate_successor = production
            .split("pub(super) struct PreparedDurableStoreValidateSuccessor<'a> {")
            .nth(1)
            .expect("Store-to-Validate successor has one declaration")
            .split("/// Move-only Store-successor projection")
            .next()
            .expect("Fetch successor follows Validate successor");
        assert!(validate_successor.contains("_replay_evidence: CertifiedValidateReplayEvidenceV1"));

        let fetch_projection = production
            .split("pub(super) fn seal_store_successor(")
            .nth(1)
            .expect("Fetch-to-Store projection has one implementation")
            .split("impl<'a> PreparedDurableStoreExecution<'a>")
            .next()
            .expect("Store execution follows Fetch projection");
        assert!(fetch_projection.contains("completion.replay_evidence.project_store("));
        assert!(fetch_projection.contains("_replay_evidence: replay_evidence"));
        let validate_projection = production
            .split("pub(super) fn seal_validate_successor(")
            .nth(1)
            .expect("Store-to-Validate projection has one implementation")
            .split("// READY_DURABLE_VALIDATE_ADAPTER_JOIN_BEGIN")
            .next()
            .expect("Ready Validate join follows Store projection");
        assert!(validate_projection.contains("store.replay_evidence.project_validate("));
        assert!(validate_projection.contains("&validate_pending"));
        assert!(validate_projection.contains("_replay_evidence: replay_evidence"));

        let detached = production
            .split("struct DetachedRecoveredValidateCompletion {")
            .nth(1)
            .expect("recovered Validate completion has one detached declaration")
            .split("pub(crate) struct AuthenticatedRecoveredWalValidateLifecycleRepair")
            .next()
            .expect("authenticated repair follows detached evidence");
        for required in [
            "replay_evidence: DetachedValidateReplayEvidenceV1",
            "#[allow(variant_size_differences, clippy::large_enum_variant)]",
            "Retained(DurableValidateReplayEvidenceV1)",
            "RecoveredBodyMarker(DurableBodyReceipt)",
            "Self::Retained(evidence) => evidence.exactly_matches_durable_body(receipt)",
            "Self::RecoveredBodyMarker(recovered) => recovered == receipt",
        ] {
            assert!(
                detached.contains(required),
                "detached Validate replay evidence omitted {required}"
            );
        }
        assert!(
            !detached.contains("=> true"),
            "detached recovery must not use a truth-sentinel provenance bypass"
        );

        let recovered_join = production
            .split_once("// RECOVERED_WAL_VALIDATE_REGISTRY_JOIN_BEGIN")
            .expect("recovered Validate join begins")
            .1
            .split_once("// RECOVERED_WAL_VALIDATE_REGISTRY_JOIN_END")
            .expect("recovered Validate join ends")
            .0;
        assert!(
            production.contains(
                "replay_evidence: DetachedValidateReplayEvidenceV1::RecoveredBodyMarker("
            )
        );
        for required in [
            "replay_evidence: DetachedValidateReplayEvidenceV1::Retained(replay_evidence)",
            "let DurableValidateBody {",
            "replay_evidence,",
            "completion.restore(effect, pending)",
        ] {
            assert!(
                recovered_join.contains(required),
                "recovered Validate join dropped {required}"
            );
        }
        let restore = production
            .split("impl DetachedRecoveredValidateCompletion {")
            .nth(1)
            .expect("detached Validate has one restore implementation")
            .split("/// Ownership-preserving failure")
            .next()
            .expect("recovered join error follows detached restore");
        assert!(restore.contains(
            "let DetachedValidateReplayEvidenceV1::Retained(replay_evidence) = self.replay_evidence"
        ));
        assert!(restore.contains("replay_evidence,"));
    }
