const KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_DESCRIPTOR_BYTES_V2: usize = 16 * 1024 * 1024;
const KAGEMUSHA_REVIEWED_SOURCE_DIFF_DOMAIN_V2: &[u8] = b"iroha-source-diff-v2\0";
const KAGEMUSHA_REVIEWED_SOURCE_TRACKED_DIFF_DOMAIN_V2: &[u8] = b"tracked-binary-diff-sha256\0";
const KAGEMUSHA_REVIEWED_SOURCE_UNTRACKED_MANIFEST_DOMAIN_V2: &[u8] =
    b"untracked-path-blob-manifest-sha256\0";
const KAGEMUSHA_REVIEWED_SOURCE_TRACKED_CARGO_LOCK_DOMAIN_V2: &[u8] = b"tracked-cargo-lock-v2\0";
fn kagemusha_reviewed_tracked_cargo_lock_json(
    lock: &KagemushaReviewedTrackedCargoLockV2,
) -> String {
    let mut out = String::new();
    out.push_str("{\"git_blob_oid\":");
    append_python_ascii_json_string(&mut out, &lock.git_blob_oid);
    out.push_str(",\"git_mode\":");
    append_python_ascii_json_string(&mut out, &lock.git_mode);
    out.push_str(",\"path\":");
    append_python_ascii_json_string(&mut out, &lock.path);
    out.push_str(",\"sha256\":\"");
    out.push_str(&hex::encode(lock.sha256));
    out.push_str("\",\"size_bytes\":");
    out.push_str(&lock.size_bytes.to_string());
    out.push('}');
    out
}
fn kagemusha_reviewed_source_fingerprint_v2(
    tracked_binary_diff_sha256: [u8; 32],
    untracked_manifest_sha256: [u8; 32],
    tracked_cargo_lock: &KagemushaReviewedTrackedCargoLockV2,
) -> [u8; 32] {
    let lock_descriptor_sha256 = Sha256::digest(kagemusha_reviewed_tracked_cargo_lock_json(
        tracked_cargo_lock,
    ));
    let mut combined = Sha256::new();
    combined.update(KAGEMUSHA_REVIEWED_SOURCE_DIFF_DOMAIN_V2);
    combined.update(KAGEMUSHA_REVIEWED_SOURCE_TRACKED_DIFF_DOMAIN_V2);
    combined.update(tracked_binary_diff_sha256);
    combined.update(KAGEMUSHA_REVIEWED_SOURCE_UNTRACKED_MANIFEST_DOMAIN_V2);
    combined.update(untracked_manifest_sha256);
    combined.update(KAGEMUSHA_REVIEWED_SOURCE_TRACKED_CARGO_LOCK_DOMAIN_V2);
    combined.update(lock_descriptor_sha256);
    combined.finalize().into()
}
impl KagemushaReviewedSourceClosureV2 {
    /// Validate the clean tracked-tree shape and its redundant `Cargo.lock` identity.
    ///
    /// Inclusion of the described blob in `source_git_tree` is authenticated by
    /// the source-seal producer. This path-free value validates the exact stable
    /// projection and rejects every V1 ignored-lock representation.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure or derived digest is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        let empty_sha256: [u8; 32] = Sha256::digest([]).into();
        let lock = &self.tracked_cargo_lock;
        let expected_fingerprint = kagemusha_reviewed_source_fingerprint_v2(
            self.tracked_binary_diff_sha256,
            self.untracked_path_mode_blob_oid_manifest_sha256,
            lock,
        );
        if self.schema != KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_SCHEMA_V2
            || !is_kagemusha_source_commit(&self.base_commit)
            || self.base_commit != self.source_commit
            || !is_kagemusha_source_commit(&self.source_git_tree)
            || self.source_repo_dirty
            || self.source_tree_sha256 == [0; 32]
            || self.tracked_binary_diff_sha256 != empty_sha256
            || self.untracked_file_count != 0
            || usize::try_from(self.untracked_file_count).ok()
                != Some(self.untracked_path_mode_blob_oid_manifest.len())
            || usize::try_from(self.untracked_file_count)
                .ok()
                .is_none_or(|count| {
                    count > KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_UNTRACKED_FILES_V2
                })
            || !self.untracked_path_mode_blob_oid_manifest.is_empty()
            || self.untracked_path_mode_blob_oid_manifest_sha256 != empty_sha256
            || lock.path != "Cargo.lock"
            || lock.git_mode != "100644"
            || !is_kagemusha_source_commit(&lock.git_blob_oid)
            || lock.sha256 == [0; 32]
            || lock.size_bytes == 0
            || lock.size_bytes > KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_CARGO_LOCK_BYTES_V2
            || self.combined_source_fingerprint_sha256 != expected_fingerprint
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v5.reviewed_source_closure",
            });
        }
        Ok(())
    }
    /// Return exact compact sorted-key ASCII JSON plus one terminal LF.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the closure is invalid or exceeds its bound.
    pub fn canonical_descriptor_bytes(&self) -> Result<Vec<u8>, KagemushaValidationError> {
        self.validate()?;
        let mut out = String::new();
        out.push_str("{\"base_commit\":");
        append_python_ascii_json_string(&mut out, &self.base_commit);
        out.push_str(",\"combined_source_fingerprint_sha256\":\"");
        out.push_str(&hex::encode(self.combined_source_fingerprint_sha256));
        out.push_str("\",\"schema\":");
        append_python_ascii_json_string(&mut out, &self.schema);
        out.push_str(",\"source_commit\":");
        append_python_ascii_json_string(&mut out, &self.source_commit);
        out.push_str(",\"source_git_tree\":");
        append_python_ascii_json_string(&mut out, &self.source_git_tree);
        out.push_str(",\"source_repo_dirty\":false,\"source_tree_sha256\":\"");
        out.push_str(&hex::encode(self.source_tree_sha256));
        out.push_str("\",\"tracked_binary_diff_sha256\":\"");
        out.push_str(&hex::encode(self.tracked_binary_diff_sha256));
        out.push_str("\",\"tracked_cargo_lock\":");
        out.push_str(&kagemusha_reviewed_tracked_cargo_lock_json(
            &self.tracked_cargo_lock,
        ));
        out.push_str(",\"untracked_file_count\":0,\"untracked_path_mode_blob_oid_manifest\":[]");
        out.push_str(",\"untracked_path_mode_blob_oid_manifest_sha256\":\"");
        out.push_str(&hex::encode(
            self.untracked_path_mode_blob_oid_manifest_sha256,
        ));
        out.push_str("\"}\n");
        if out.len() > KAGEMUSHA_REVIEWED_SOURCE_CLOSURE_MAX_DESCRIPTOR_BYTES_V2 {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v5.reviewed_source_closure.size",
            });
        }
        Ok(out.into_bytes())
    }
    /// SHA-256 of the exact canonical compact sorted-key ASCII JSON plus LF.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the closure is invalid.
    pub fn canonical_descriptor_sha256(&self) -> Result<[u8; 32], KagemushaValidationError> {
        Ok(Sha256::digest(self.canonical_descriptor_bytes()?).into())
    }
}
impl KagemushaRecursiveSpendArtifactManifestV5 {
    /// Validate the complete, explicitly versioned V5 release shape.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when any source, profile, or release invariant is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        self.validate_with_attestation_state(true)
    }
    /// Validate an immutable V5 release candidate before qualification or evidence exists.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when any source, profile, or candidate invariant is invalid.
    pub fn validate_unsigned_candidate(&self) -> Result<(), KagemushaValidationError> {
        self.validate_with_attestation_state(false)
    }
    /// SHA-256 identity of a complete canonical V5 release manifest.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when validation or canonical encoding fails.
    pub fn canonical_sha256(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate()?;
        Ok(Sha256::digest(norito::encode_canonical(self)?).into())
    }
    /// Reconstruct the exact immutable candidate that preceded V5 finalization.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the release or reconstructed candidate is invalid.
    pub fn immutable_candidate(
        &self,
    ) -> Result<KagemushaRecursiveSpendCandidateV5, KagemushaValidationError> {
        self.validate()?;
        self.immutable_candidate_unchecked_for_qualification()
    }
    fn validate_with_attestation_state(
        &self,
        finalized: bool,
    ) -> Result<(), KagemushaValidationError> {
        self.validate_manifest_identity_and_attestation_fields(finalized)?;
        self.topup_finality_roster_artifact.validate()?;
        let mut names = std::collections::BTreeSet::new();
        let mut digests = std::collections::BTreeSet::new();
        names.insert(
            self.topup_finality_roster_artifact
                .file_name
                .to_ascii_lowercase(),
        );
        digests.insert(self.topup_finality_roster_artifact.sha256);
        let mut structure_digests = std::collections::BTreeSet::new();
        for profile in &self.profiles {
            profile.validate()?;
            let _ = profile.circuit_params_sha256()?;
            if !structure_digests.insert(profile.compiled_protocol_structure_sha256) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v5.artifact_manifest.profile_identity",
                });
            }
            for artifact in &profile.artifacts {
                let name_is_new = names.insert(artifact.file_name.to_ascii_lowercase());
                let framed_digest_is_new = digests.insert(artifact.sha256);
                let payload_digest_is_new = digests.insert(artifact.payload_sha256);
                if !name_is_new || !framed_digest_is_new || !payload_digest_is_new {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "pasta_cycle.v5.artifact_manifest.artifact_identity",
                    });
                }
            }
        }
        if finalized {
            for evidence_digest in [
                self.qualification_receipt_sha256,
                self.qualified_candidate_sha256,
                self.benchmark_evidence_sha256,
                self.cryptographic_review_sha256,
            ] {
                if !digests.insert(evidence_digest) {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "pasta_cycle.v5.artifact_manifest.evidence_sha256",
                    });
                }
            }
            if !digests.insert(self.release_attestation_sha256) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v5.artifact_manifest.evidence_sha256",
                });
            }
            let candidate = self.immutable_candidate_unchecked_for_qualification()?;
            let expected_qualified_candidate =
                kagemusha_recursive_spend_qualified_candidate_sha256_v5(
                    candidate.sha256()?,
                    self.qualification_receipt_sha256,
                );
            if self.qualified_candidate_sha256 != expected_qualified_candidate {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v5.artifact_manifest.qualified_candidate",
                });
            }
        }
        Ok(())
    }
    fn validate_manifest_identity_and_attestation_fields(
        &self,
        finalized: bool,
    ) -> Result<(), KagemushaValidationError> {
        let reviewed_source_closure_valid = self.reviewed_source_closure.validate().is_ok()
            && self.reviewed_source_closure.source_commit == self.source_commit
            && self.reviewed_source_closure.source_git_tree == self.source_git_tree
            && self.reviewed_source_closure.source_tree_sha256 == self.source_tree_sha256
            && self.reviewed_source_closure.source_repo_dirty == self.source_repo_dirty
            && self
                .reviewed_source_closure
                .canonical_descriptor_sha256()
                .is_ok_and(|sha256| sha256 == self.reviewed_source_closure_descriptor_sha256);
        let measured_step_bytes = self.profiles.iter().try_fold(0_u32, |sum, profile| {
            sum.checked_add(profile.step_proof_size_bytes)
        });
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V5
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V5
            || self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4
            || !is_kagemusha_portable_identifier(&self.generation)
            || !is_kagemusha_source_commit(&self.source_commit)
            || !is_kagemusha_source_commit(&self.source_git_tree)
            || self.source_tree_sha256 == [0; 32]
            || self.source_repo_dirty
            || !reviewed_source_closure_valid
            || self.authenticated_source_seal_projection_sha256 == [0; 32]
            || !is_kagemusha_network_id(&self.network_id)
            || self.asset_scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
            || self.activation_height == 0
            || self.withdrawal_height <= self.activation_height
            || self.max_proof_bytes == 0
            || self.max_proof_bytes > KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
            || measured_step_bytes.is_none_or(|minimum| self.max_proof_bytes <= minimum)
            || self.profiles.len() != 2
            || self.profiles[0].parity != KagemushaPastaCycleParityV1::StepEq
            || self.profiles[1].parity != KagemushaPastaCycleParityV1::StepEp
            || self.topup_finality_roster_artifact.artifact_generation != self.generation
            || self.generation_memory_limit_bytes == 0
            || self.generation_memory_limit_bytes
                > KAGEMUSHA_RECURSIVE_SPEND_GENERATION_MEMORY_ABSOLUTE_MAX_BYTES_V4
            || self.generation_memory_enforcement_profile
                != KAGEMUSHA_RECURSIVE_SPEND_GENERATION_MEMORY_ENFORCEMENT_PROFILE_V4
            || (finalized && self.qualification_receipt_sha256 == [0; 32])
            || (finalized && self.qualified_candidate_sha256 == [0; 32])
            || (finalized && self.benchmark_evidence_sha256 == [0; 32])
            || (finalized && self.cryptographic_review_sha256 == [0; 32])
            || (finalized && self.release_attestation_sha256 == [0; 32])
            || (!finalized && self.qualification_receipt_sha256 != [0; 32])
            || (!finalized && self.qualified_candidate_sha256 != [0; 32])
            || (!finalized && self.benchmark_evidence_sha256 != [0; 32])
            || (!finalized && self.cryptographic_review_sha256 != [0; 32])
            || (!finalized && self.release_attestation_sha256 != [0; 32])
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v5.artifact_manifest",
            });
        }
        Ok(())
    }
    fn immutable_candidate_unchecked_for_qualification(
        &self,
    ) -> Result<KagemushaRecursiveSpendCandidateV5, KagemushaValidationError> {
        let mut manifest = self.clone();
        manifest.qualification_receipt_sha256 = [0; 32];
        manifest.qualified_candidate_sha256 = [0; 32];
        manifest.benchmark_evidence_sha256 = [0; 32];
        manifest.cryptographic_review_sha256 = [0; 32];
        manifest.release_attestation_sha256 = [0; 32];
        let candidate = KagemushaRecursiveSpendCandidateV5 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V5.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V5,
            manifest,
        };
        candidate.validate()?;
        Ok(candidate)
    }
    /// Build the non-circular source-bound V5 subject signed by every release authority.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when the manifest is invalid or cannot be encoded.
    pub fn release_attestation_subject(
        &self,
    ) -> Result<KagemushaRecursiveSpendReleaseAttestationSubjectV5, KagemushaReleaseVerificationError>
    {
        self.validate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        let mut subject_manifest = self.clone();
        subject_manifest.release_attestation_sha256 = [0; 32];
        let subject_bytes = norito::encode_canonical(&subject_manifest)
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        Ok(KagemushaRecursiveSpendReleaseAttestationSubjectV5 {
            manifest_subject_sha256: Sha256::digest(subject_bytes).into(),
            qualification_receipt_sha256: self.qualification_receipt_sha256,
            qualified_candidate_sha256: self.qualified_candidate_sha256,
            generation: self.generation.clone(),
            source_commit: self.source_commit.clone(),
            source_git_tree: self.source_git_tree.clone(),
            source_tree_sha256: self.source_tree_sha256,
            source_repo_dirty: self.source_repo_dirty,
            reviewed_source_closure_descriptor_sha256: self
                .reviewed_source_closure_descriptor_sha256,
            authenticated_source_seal_projection_sha256: self
                .authenticated_source_seal_projection_sha256,
            benchmark_evidence_sha256: self.benchmark_evidence_sha256,
            cryptographic_review_sha256: self.cryptographic_review_sha256,
        })
    }
}
impl KagemushaRecursiveSpendCandidateV5 {
    /// Validate the V2-closure-bound pre-evidence V5 candidate contract.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the candidate is not exact V5 material.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V5
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V5
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v5.candidate",
            });
        }
        self.manifest.validate_unsigned_candidate()
    }
    /// Return the SHA-256 identity of the canonical V5 candidate record.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the candidate is invalid or cannot be encoded.
    pub fn sha256(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate()?;
        Ok(Sha256::digest(norito::encode_canonical(self)?).into())
    }
    /// Return framed then payload identities for all eight cryptographic artifact roles.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the candidate or role inventory is invalid.
    pub fn artifact_role_digests(&self) -> Result<[[u8; 32]; 16], KagemushaValidationError> {
        self.validate()?;
        let canonical_roles = [
            (
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            ),
            (
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::ProvingKey,
            ),
            (
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            ),
            (
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            ),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            ),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::ProvingKey,
            ),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            ),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            ),
        ];
        let mut digests = [[0_u8; 32]; 16];
        for (index, (parity, kind)) in canonical_roles.into_iter().enumerate() {
            let descriptor = self
                .manifest
                .profiles
                .iter()
                .find(|profile| profile.parity == parity)
                .and_then(|profile| {
                    profile
                        .artifacts
                        .iter()
                        .find(|descriptor| descriptor.kind == kind)
                })
                .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v5.candidate.artifact_roles",
                })?;
            digests[2 * index] = descriptor.sha256;
            digests[2 * index + 1] = descriptor.payload_sha256;
        }
        Ok(digests)
    }
    /// Build the exact source-bound V5 subject signed by cryptographic reviewers.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a receipt identity or candidate binding is invalid.
    pub fn cryptographic_review_subject(
        &self,
        qualification_receipt_sha256: [u8; 32],
        qualified_candidate_sha256: [u8; 32],
    ) -> Result<KagemushaRecursiveSpendCryptographicReviewSubjectV5, KagemushaValidationError> {
        let candidate_sha256 = self.sha256()?;
        if qualification_receipt_sha256 == [0; 32]
            || qualified_candidate_sha256
                != kagemusha_recursive_spend_qualified_candidate_sha256_v5(
                    candidate_sha256,
                    qualification_receipt_sha256,
                )
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v5.qualified_candidate",
            });
        }
        Ok(KagemushaRecursiveSpendCryptographicReviewSubjectV5 {
            candidate_sha256,
            qualification_receipt_sha256,
            qualified_candidate_sha256,
            generation: self.manifest.generation.clone(),
            source_commit: self.manifest.source_commit.clone(),
            source_git_tree: self.manifest.source_git_tree.clone(),
            source_tree_sha256: self.manifest.source_tree_sha256,
            source_repo_dirty: self.manifest.source_repo_dirty,
            reviewed_source_closure_descriptor_sha256: self
                .manifest
                .reviewed_source_closure_descriptor_sha256,
            authenticated_source_seal_projection_sha256: self
                .manifest
                .authenticated_source_seal_projection_sha256,
            network_id: self.manifest.network_id,
            asset: self.manifest.asset.clone(),
            bridge_abi_version: self.manifest.bridge_abi_version,
        })
    }
}
/// Derive the non-circular identity of one V5 candidate and proof-bearing receipt.
#[must_use]
pub fn kagemusha_recursive_spend_qualified_candidate_sha256_v5(
    candidate_sha256: [u8; 32],
    qualification_receipt_sha256: [u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(KAGEMUSHA_RECURSIVE_SPEND_QUALIFIED_CANDIDATE_DOMAIN_V5);
    hasher.update([0]);
    hasher.update(candidate_sha256);
    hasher.update(qualification_receipt_sha256);
    hasher.finalize().into()
}
impl KagemushaRecursiveSpendArtifactManifestV4 {
    /// Validate the complete, explicitly versioned V4 release shape.
    ///
    /// This validates content binding only. A V4 release attestation must be
    /// authenticated separately before any artifact is used.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        self.validate_with_attestation_state(true)
    }
    /// Validate an immutable V4 release candidate before its attestation exists.
    ///
    /// Candidate manifests precede external evidence, so benchmark, review,
    /// and attestation digests must all remain zero. They are not valid release
    /// manifests and must never be accepted by production artifact readers.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_unsigned_candidate(&self) -> Result<(), KagemushaValidationError> {
        self.validate_with_attestation_state(false)
    }
    /// Return the SHA-256 identity of the canonical finalized V4 manifest.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn canonical_sha256(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate()?;
        Ok(Sha256::digest(norito::encode_canonical(self)?).into())
    }
    /// Reconstruct the byte-exact immutable candidate that preceded this finalized manifest.
    ///
    /// Finalization fills the qualification identities, internal-validation
    /// receipt digest, two external-evidence digests, and release-attestation
    /// digest. Clearing exactly those fields must therefore recover a valid
    /// candidate.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn immutable_candidate(
        &self,
    ) -> Result<KagemushaRecursiveSpendCandidateV4, KagemushaValidationError> {
        self.validate()?;
        let mut manifest = self.clone();
        manifest.qualification_receipt_sha256 = [0; 32];
        manifest.qualified_candidate_sha256 = [0; 32];
        manifest.internal_validation_receipt_sha256 = [0; 32];
        manifest.benchmark_evidence_sha256 = [0; 32];
        manifest.cryptographic_review_sha256 = [0; 32];
        manifest.release_attestation_sha256 = [0; 32];
        let candidate = KagemushaRecursiveSpendCandidateV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V4,
            manifest,
        };
        candidate.validate()?;
        Ok(candidate)
    }
    #[allow(
        clippy::too_many_lines,
        reason = "the fixed V4 manifest validator keeps all consensus-critical invariants in one auditable path"
    )]
    fn validate_with_attestation_state(
        &self,
        finalized: bool,
    ) -> Result<(), KagemushaValidationError> {
        let measured_step_bytes = self.profiles.iter().try_fold(0_u32, |sum, profile| {
            sum.checked_add(profile.step_proof_size_bytes)
        });
        let reviewed_source_closure_valid = self.reviewed_source_closure.validate().is_ok()
            && self.reviewed_source_closure.source_commit == self.source_commit
            && self.reviewed_source_closure.source_tree_sha256 == self.source_tree_sha256
            && self.reviewed_source_closure.source_repo_dirty == self.source_repo_dirty
            && self
                .reviewed_source_closure
                .canonical_descriptor_sha256()
                .is_ok_and(|sha256| sha256 == self.reviewed_source_closure_descriptor_sha256);
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_SCHEMA_V4
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_MANIFEST_VERSION_V4
            || self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4
            || self.proof_backend != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
            || self.transcript_profile != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_TRANSCRIPT_V4
            || !is_kagemusha_portable_identifier(&self.generation)
            || !is_kagemusha_source_commit(&self.source_commit)
            || self.source_tree_sha256 == [0; 32]
            || self.source_repo_dirty
            || !reviewed_source_closure_valid
            || self.authenticated_source_seal_projection_sha256 == [0; 32]
            || self.reviewed_cargo_binary_sha256 == [0; 32]
            || self.reviewed_rustc_binary_sha256 == [0; 32]
            || self.generator_binary_sha256 == [0; 32]
            || self.sealed_candidate_build_report_sha256 == [0; 32]
            || !is_kagemusha_network_id(&self.network_id)
            || self.asset_scale > KAGEMUSHA_SCALED_AMOUNT_MAX_SCALE_V2
            || self.activation_height == 0
            || self.withdrawal_height <= self.activation_height
            || self.max_proof_bytes == 0
            || self.max_proof_bytes > KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
            || measured_step_bytes.is_none_or(|minimum| self.max_proof_bytes <= minimum)
            || self.profiles.len() != 2
            || self.profiles[0].parity != KagemushaPastaCycleParityV1::StepEq
            || self.profiles[1].parity != KagemushaPastaCycleParityV1::StepEp
            || self.topup_finality_roster_artifact.artifact_generation != self.generation
            || self.generation_memory_limit_bytes == 0
            || self.generation_memory_limit_bytes
                > KAGEMUSHA_RECURSIVE_SPEND_GENERATION_MEMORY_ABSOLUTE_MAX_BYTES_V4
            || self.generation_memory_enforcement_profile
                != KAGEMUSHA_RECURSIVE_SPEND_GENERATION_MEMORY_ENFORCEMENT_PROFILE_V4
            || (finalized && self.qualification_receipt_sha256 == [0; 32])
            || (finalized && self.qualified_candidate_sha256 == [0; 32])
            || (finalized && self.internal_validation_receipt_sha256 == [0; 32])
            || (finalized && self.benchmark_evidence_sha256 == [0; 32])
            || (finalized && self.cryptographic_review_sha256 == [0; 32])
            || (finalized && self.release_attestation_sha256 == [0; 32])
            || (!finalized && self.benchmark_evidence_sha256 != [0; 32])
            || (!finalized && self.qualification_receipt_sha256 != [0; 32])
            || (!finalized && self.qualified_candidate_sha256 != [0; 32])
            || (!finalized && self.internal_validation_receipt_sha256 != [0; 32])
            || (!finalized && self.cryptographic_review_sha256 != [0; 32])
            || (!finalized && self.release_attestation_sha256 != [0; 32])
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.artifact_manifest",
            });
        }
        self.topup_finality_roster_artifact.validate()?;
        let mut names = std::collections::BTreeSet::new();
        let mut digests = std::collections::BTreeSet::new();
        names.insert(
            self.topup_finality_roster_artifact
                .file_name
                .to_ascii_lowercase(),
        );
        digests.insert(self.topup_finality_roster_artifact.sha256);
        let mut structure_digests = std::collections::BTreeSet::new();
        for profile in &self.profiles {
            profile.validate()?;
            let _ = profile.circuit_params_sha256()?;
            if !structure_digests.insert(profile.compiled_protocol_structure_sha256) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.artifact_manifest.profile_identity",
                });
            }
            for artifact in &profile.artifacts {
                let name_is_new = names.insert(artifact.file_name.to_ascii_lowercase());
                let framed_digest_is_new = digests.insert(artifact.sha256);
                let payload_digest_is_new = digests.insert(artifact.payload_sha256);
                if !name_is_new || !framed_digest_is_new || !payload_digest_is_new {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "pasta_cycle.v4.artifact_manifest.artifact_identity",
                    });
                }
            }
        }
        if finalized {
            for evidence_digest in [
                self.qualification_receipt_sha256,
                self.qualified_candidate_sha256,
                self.internal_validation_receipt_sha256,
                self.benchmark_evidence_sha256,
                self.cryptographic_review_sha256,
            ] {
                if !digests.insert(evidence_digest) {
                    return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                        field: "pasta_cycle.v4.artifact_manifest.evidence_sha256",
                    });
                }
            }
            if !digests.insert(self.release_attestation_sha256) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.artifact_manifest.evidence_sha256",
                });
            }
            let candidate = self.immutable_candidate_unchecked_for_qualification()?;
            let expected_qualified_candidate =
                kagemusha_recursive_spend_qualified_candidate_sha256_v4(
                    candidate.sha256()?,
                    self.qualification_receipt_sha256,
                );
            if self.qualified_candidate_sha256 != expected_qualified_candidate {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.artifact_manifest.qualified_candidate",
                });
            }
        }
        Ok(())
    }
    fn immutable_candidate_unchecked_for_qualification(
        &self,
    ) -> Result<KagemushaRecursiveSpendCandidateV4, KagemushaValidationError> {
        let mut manifest = self.clone();
        manifest.qualification_receipt_sha256 = [0; 32];
        manifest.qualified_candidate_sha256 = [0; 32];
        manifest.internal_validation_receipt_sha256 = [0; 32];
        manifest.benchmark_evidence_sha256 = [0; 32];
        manifest.cryptographic_review_sha256 = [0; 32];
        manifest.release_attestation_sha256 = [0; 32];
        let candidate = KagemushaRecursiveSpendCandidateV4 {
            schema: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V4,
            manifest,
        };
        candidate.validate()?;
        Ok(candidate)
    }
    /// Build the non-circular V4 subject signed by every release authority.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when the signing subject is invalid or cannot be encoded canonically.
    pub fn release_attestation_subject(
        &self,
    ) -> Result<KagemushaRecursiveSpendReleaseAttestationSubjectV4, KagemushaReleaseVerificationError>
    {
        self.validate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        self.release_attestation_subject_from_validated_manifest()
    }
    fn release_attestation_subject_from_validated_manifest(
        &self,
    ) -> Result<KagemushaRecursiveSpendReleaseAttestationSubjectV4, KagemushaReleaseVerificationError>
    {
        let mut subject_manifest = self.clone();
        subject_manifest.release_attestation_sha256 = [0; 32];
        let subject_bytes = norito::encode_canonical(&subject_manifest)
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        Ok(KagemushaRecursiveSpendReleaseAttestationSubjectV4 {
            manifest_subject_sha256: Sha256::digest(subject_bytes).into(),
            qualification_receipt_sha256: self.qualification_receipt_sha256,
            qualified_candidate_sha256: self.qualified_candidate_sha256,
            internal_validation_receipt_sha256: self.internal_validation_receipt_sha256,
            generation: self.generation.clone(),
            source_commit: self.source_commit.clone(),
            source_tree_sha256: self.source_tree_sha256,
            source_repo_dirty: self.source_repo_dirty,
            reviewed_source_closure_descriptor_sha256: self
                .reviewed_source_closure_descriptor_sha256,
            authenticated_source_seal_projection_sha256: self
                .authenticated_source_seal_projection_sha256,
            reviewed_cargo_binary_sha256: self.reviewed_cargo_binary_sha256,
            reviewed_rustc_binary_sha256: self.reviewed_rustc_binary_sha256,
            generator_binary_sha256: self.generator_binary_sha256,
            sealed_candidate_build_report_sha256: self.sealed_candidate_build_report_sha256,
            benchmark_evidence_sha256: self.benchmark_evidence_sha256,
            cryptographic_review_sha256: self.cryptographic_review_sha256,
        })
    }
}
impl KagemushaRecursiveSpendCandidateV4 {
    /// Validate the reviewed-source-closure-bound pre-evidence candidate contract.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaValidationError> {
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_SCHEMA_V4
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_CANDIDATE_VERSION_V4
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.candidate",
            });
        }
        self.manifest.validate_unsigned_candidate()
    }
    /// Return the SHA-256 identity of the canonical candidate record.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the value is invalid or its canonical digest preimage cannot be encoded.
    pub fn sha256(&self) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate()?;
        Ok(Sha256::digest(norito::encode_canonical(self)?).into())
    }
    /// Return framed then payload identities for all eight canonical artifact roles.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the candidate or its role inventory is invalid.
    pub fn artifact_role_digests(&self) -> Result<[[u8; 32]; 16], KagemushaValidationError> {
        self.validate()?;
        let canonical_roles = [
            (
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            ),
            (
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::ProvingKey,
            ),
            (
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            ),
            (
                KagemushaPastaCycleParityV1::StepEq,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            ),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::ParamsIpa,
            ),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::ProvingKey,
            ),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::VerifyingKey,
            ),
            (
                KagemushaPastaCycleParityV1::StepEp,
                KagemushaPastaCycleArtifactKindV4::BootstrapWitness,
            ),
        ];
        let mut digests = [[0_u8; 32]; 16];
        for (index, (parity, kind)) in canonical_roles.into_iter().enumerate() {
            let descriptor = self
                .manifest
                .profiles
                .iter()
                .find(|profile| profile.parity == parity)
                .and_then(|profile| {
                    profile
                        .artifacts
                        .iter()
                        .find(|descriptor| descriptor.kind == kind)
                })
                .ok_or(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.candidate.artifact_roles",
                })?;
            digests[2 * index] = descriptor.sha256;
            digests[2 * index + 1] = descriptor.payload_sha256;
        }
        Ok(digests)
    }
    /// Build the exact candidate-bound subject signed by cryptographic reviewers.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the signing subject is invalid or cannot be encoded canonically.
    pub fn cryptographic_review_subject(
        &self,
        qualification_receipt_sha256: [u8; 32],
        qualified_candidate_sha256: [u8; 32],
    ) -> Result<KagemushaRecursiveSpendCryptographicReviewSubjectV4, KagemushaValidationError> {
        let candidate_sha256 = self.sha256()?;
        if qualification_receipt_sha256 == [0; 32]
            || qualified_candidate_sha256
                != kagemusha_recursive_spend_qualified_candidate_sha256_v4(
                    candidate_sha256,
                    qualification_receipt_sha256,
                )
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.qualified_candidate",
            });
        }
        Ok(KagemushaRecursiveSpendCryptographicReviewSubjectV4 {
            candidate_sha256,
            qualification_receipt_sha256,
            qualified_candidate_sha256,
            generation: self.manifest.generation.clone(),
            source_commit: self.manifest.source_commit.clone(),
            source_tree_sha256: self.manifest.source_tree_sha256,
            source_repo_dirty: self.manifest.source_repo_dirty,
            reviewed_source_closure_descriptor_sha256: self
                .manifest
                .reviewed_source_closure_descriptor_sha256,
            authenticated_source_seal_projection_sha256: self
                .manifest
                .authenticated_source_seal_projection_sha256,
            reviewed_cargo_binary_sha256: self.manifest.reviewed_cargo_binary_sha256,
            reviewed_rustc_binary_sha256: self.manifest.reviewed_rustc_binary_sha256,
            generator_binary_sha256: self.manifest.generator_binary_sha256,
            sealed_candidate_build_report_sha256: self
                .manifest
                .sealed_candidate_build_report_sha256,
            network_id: self.manifest.network_id,
            asset: self.manifest.asset.clone(),
            bridge_abi_version: self.manifest.bridge_abi_version,
        })
    }
}
/// Derive the non-circular identity of one candidate and its proof-bearing receipt.
#[must_use]
pub fn kagemusha_recursive_spend_qualified_candidate_sha256_v4(
    candidate_sha256: [u8; 32],
    qualification_receipt_sha256: [u8; 32],
) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(KAGEMUSHA_RECURSIVE_SPEND_QUALIFIED_CANDIDATE_DOMAIN_V4);
    hasher.update([0]);
    hasher.update(candidate_sha256);
    hasher.update(qualification_receipt_sha256);
    hasher.finalize().into()
}
impl KagemushaRecursiveSpendQualificationReceiptV4 {
    /// Construct a receipt from the two exact proof-pair byte strings.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the candidate or either bounded pair is invalid.
    pub fn new(
        candidate: &KagemushaRecursiveSpendCandidateV4,
        initialization_pair: Vec<u8>,
        append_pair: Vec<u8>,
    ) -> Result<Self, KagemushaValidationError> {
        let candidate_sha256 = candidate.sha256()?;
        let manifest_sha256 = Sha256::digest(norito::encode_canonical(&candidate.manifest)?).into();
        let receipt = Self {
            schema: KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_SCHEMA_V4.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_VERSION_V4,
            candidate_sha256,
            manifest_sha256,
            authenticated_source_seal_projection_sha256: candidate
                .manifest
                .authenticated_source_seal_projection_sha256,
            reviewed_cargo_binary_sha256: candidate.manifest.reviewed_cargo_binary_sha256,
            reviewed_rustc_binary_sha256: candidate.manifest.reviewed_rustc_binary_sha256,
            generator_binary_sha256: candidate.manifest.generator_binary_sha256,
            sealed_candidate_build_report_sha256: candidate
                .manifest
                .sealed_candidate_build_report_sha256,
            generation_memory_limit_bytes: candidate.manifest.generation_memory_limit_bytes,
            generation_memory_enforcement_profile: candidate
                .manifest
                .generation_memory_enforcement_profile
                .clone(),
            artifact_role_digests: candidate.artifact_role_digests()?,
            initialization_pair,
            append_pair,
        };
        receipt.validate_against_candidate(candidate)?;
        Ok(receipt)
    }
    /// Decode canonical, bounded Norito bytes and bind every receipt field to a candidate.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] for missing, oversized, non-canonical,
    /// malformed, role-substituted, or candidate-substituted receipt bytes.
    pub fn decode_canonical_against_candidate(
        bytes: &[u8],
        candidate: &KagemushaRecursiveSpendCandidateV4,
    ) -> Result<Self, KagemushaValidationError> {
        if bytes.is_empty()
            || bytes.len() > KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.qualification_receipt.bytes",
            });
        }
        let limits = norito::core::DecodeLimits::new(
            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4,
            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4,
            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4,
            4 * KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4,
            32,
        );
        let receipt: Self = norito::decode_canonical_with_limits(bytes, limits).map_err(|_| {
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.qualification_receipt.canonical",
            }
        })?;
        receipt.validate_against_candidate(candidate)?;
        Ok(receipt)
    }
    /// Validate structural bounds and exact candidate, manifest, and role identities.
    ///
    /// Proof counters and parent semantics are intentionally not trusted here;
    /// the Core terminal verifier must derive them from both proof byte strings.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when any receipt identity or bound differs.
    pub fn validate_against_candidate(
        &self,
        candidate: &KagemushaRecursiveSpendCandidateV4,
    ) -> Result<(), KagemushaValidationError> {
        let candidate_sha256 = candidate.sha256()?;
        let manifest_sha256: [u8; 32] =
            Sha256::digest(norito::encode_canonical(&candidate.manifest)?).into();
        let maximum_pair_bytes =
            usize::try_from(candidate.manifest.max_proof_bytes).map_err(|_| {
                KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.qualification_receipt.pair_bound",
                }
            })?;
        let encoded_size_is_bounded = norito::encode_canonical(self).is_ok_and(|bytes| {
            bytes.len() <= KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V4
        });
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_SCHEMA_V4
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_VERSION_V4
            || self.candidate_sha256 != candidate_sha256
            || self.manifest_sha256 != manifest_sha256
            || self.authenticated_source_seal_projection_sha256
                != candidate
                    .manifest
                    .authenticated_source_seal_projection_sha256
            || self.reviewed_cargo_binary_sha256 != candidate.manifest.reviewed_cargo_binary_sha256
            || self.reviewed_rustc_binary_sha256 != candidate.manifest.reviewed_rustc_binary_sha256
            || self.generator_binary_sha256 != candidate.manifest.generator_binary_sha256
            || self.sealed_candidate_build_report_sha256
                != candidate.manifest.sealed_candidate_build_report_sha256
            || self.generation_memory_limit_bytes
                != candidate.manifest.generation_memory_limit_bytes
            || self.generation_memory_enforcement_profile
                != candidate.manifest.generation_memory_enforcement_profile
            || self.artifact_role_digests != candidate.artifact_role_digests()?
            || self.initialization_pair.is_empty()
            || self.initialization_pair.len() > maximum_pair_bytes
            || self.append_pair.is_empty()
            || self.append_pair.len() > maximum_pair_bytes
            || self.initialization_pair == self.append_pair
            || !encoded_size_is_bounded
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.qualification_receipt",
            });
        }
        Ok(())
    }
    /// SHA-256 of the exact canonical receipt after candidate binding.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when validation or canonical encoding fails.
    pub fn canonical_sha256_against_candidate(
        &self,
        candidate: &KagemushaRecursiveSpendCandidateV4,
    ) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_against_candidate(candidate)?;
        Ok(Sha256::digest(norito::encode_canonical(self)?).into())
    }
    /// Domain-separated identity of this exact candidate and receipt.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when validation or canonical encoding fails.
    pub fn qualified_candidate_sha256(
        &self,
        candidate: &KagemushaRecursiveSpendCandidateV4,
    ) -> Result<[u8; 32], KagemushaValidationError> {
        Ok(kagemusha_recursive_spend_qualified_candidate_sha256_v4(
            candidate.sha256()?,
            self.canonical_sha256_against_candidate(candidate)?,
        ))
    }
    /// Exact canonical initialization proof pair.
    #[must_use]
    pub fn initialization_pair(&self) -> &[u8] {
        &self.initialization_pair
    }
    /// Exact canonical one-parent child proof pair.
    #[must_use]
    pub fn append_pair(&self) -> &[u8] {
        &self.append_pair
    }
    /// Exact candidate identity embedded in this receipt.
    #[must_use]
    pub const fn candidate_sha256(&self) -> [u8; 32] {
        self.candidate_sha256
    }
    /// Exact manifest identity embedded in this receipt.
    #[must_use]
    pub const fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_sha256
    }
    /// Exact in-process physical-memory ceiling bound by this receipt.
    #[must_use]
    pub const fn generation_memory_limit_bytes(&self) -> u64 {
        self.generation_memory_limit_bytes
    }
    /// Exact mandatory in-process memory enforcement profile bound by this receipt.
    #[must_use]
    pub fn generation_memory_enforcement_profile(&self) -> &str {
        &self.generation_memory_enforcement_profile
    }
    /// Framed then payload digests for the eight exact artifact roles.
    #[must_use]
    pub const fn artifact_role_digests(&self) -> [[u8; 32]; 16] {
        self.artifact_role_digests
    }
}
impl KagemushaRecursiveSpendQualificationReceiptV5 {
    /// Construct a V5 receipt from two exact proof-pair byte strings.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the candidate or either bounded pair is invalid.
    pub fn new(
        candidate: &KagemushaRecursiveSpendCandidateV5,
        initialization_pair: Vec<u8>,
        append_pair: Vec<u8>,
    ) -> Result<Self, KagemushaValidationError> {
        let candidate_sha256 = candidate.sha256()?;
        let manifest_sha256 = Sha256::digest(norito::encode_canonical(&candidate.manifest)?).into();
        let receipt = Self {
            schema: KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_SCHEMA_V5.to_owned(),
            version: KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_VERSION_V5,
            candidate_sha256,
            manifest_sha256,
            generation_memory_limit_bytes: candidate.manifest.generation_memory_limit_bytes,
            generation_memory_enforcement_profile: candidate
                .manifest
                .generation_memory_enforcement_profile
                .clone(),
            artifact_role_digests: candidate.artifact_role_digests()?,
            initialization_pair,
            append_pair,
        };
        receipt.validate_against_candidate(candidate)?;
        Ok(receipt)
    }
    /// Decode canonical bounded Norito bytes and bind every field to a V5 candidate.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] for non-canonical, oversized, or substituted bytes.
    pub fn decode_canonical_against_candidate(
        bytes: &[u8],
        candidate: &KagemushaRecursiveSpendCandidateV5,
    ) -> Result<Self, KagemushaValidationError> {
        if bytes.is_empty()
            || bytes.len() > KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V5
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v5.qualification_receipt.bytes",
            });
        }
        let limits = norito::core::DecodeLimits::new(
            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V5,
            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V5,
            KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V5,
            4 * KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V5,
            32,
        );
        let receipt: Self = norito::decode_canonical_with_limits(bytes, limits).map_err(|_| {
            KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v5.qualification_receipt.canonical",
            }
        })?;
        receipt.validate_against_candidate(candidate)?;
        Ok(receipt)
    }
    /// Validate structural bounds and exact V5 candidate, manifest, and role identities.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when any identity or bound differs.
    pub fn validate_against_candidate(
        &self,
        candidate: &KagemushaRecursiveSpendCandidateV5,
    ) -> Result<(), KagemushaValidationError> {
        let candidate_sha256 = candidate.sha256()?;
        let manifest_sha256: [u8; 32] =
            Sha256::digest(norito::encode_canonical(&candidate.manifest)?).into();
        let maximum_pair_bytes =
            usize::try_from(candidate.manifest.max_proof_bytes).map_err(|_| {
                KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v5.qualification_receipt.pair_bound",
                }
            })?;
        let encoded_size_is_bounded = norito::encode_canonical(self).is_ok_and(|bytes| {
            bytes.len() <= KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_MAX_BYTES_V5
        });
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_SCHEMA_V5
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_QUALIFICATION_RECEIPT_VERSION_V5
            || self.candidate_sha256 != candidate_sha256
            || self.manifest_sha256 != manifest_sha256
            || self.generation_memory_limit_bytes
                != candidate.manifest.generation_memory_limit_bytes
            || self.generation_memory_enforcement_profile
                != candidate.manifest.generation_memory_enforcement_profile
            || self.artifact_role_digests != candidate.artifact_role_digests()?
            || self.initialization_pair.is_empty()
            || self.initialization_pair.len() > maximum_pair_bytes
            || self.append_pair.is_empty()
            || self.append_pair.len() > maximum_pair_bytes
            || self.initialization_pair == self.append_pair
            || !encoded_size_is_bounded
        {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v5.qualification_receipt",
            });
        }
        Ok(())
    }
    /// SHA-256 of the exact canonical receipt after candidate binding.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when validation or encoding fails.
    pub fn canonical_sha256_against_candidate(
        &self,
        candidate: &KagemushaRecursiveSpendCandidateV5,
    ) -> Result<[u8; 32], KagemushaValidationError> {
        self.validate_against_candidate(candidate)?;
        Ok(Sha256::digest(norito::encode_canonical(self)?).into())
    }
    /// Domain-separated identity of this exact V5 candidate and receipt.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when validation or encoding fails.
    pub fn qualified_candidate_sha256(
        &self,
        candidate: &KagemushaRecursiveSpendCandidateV5,
    ) -> Result<[u8; 32], KagemushaValidationError> {
        Ok(kagemusha_recursive_spend_qualified_candidate_sha256_v5(
            candidate.sha256()?,
            self.canonical_sha256_against_candidate(candidate)?,
        ))
    }
    /// Exact canonical initialization proof pair.
    #[must_use]
    pub fn initialization_pair(&self) -> &[u8] {
        &self.initialization_pair
    }
    /// Exact canonical one-parent child proof pair.
    #[must_use]
    pub fn append_pair(&self) -> &[u8] {
        &self.append_pair
    }
    /// Exact candidate identity embedded in this receipt.
    #[must_use]
    pub const fn candidate_sha256(&self) -> [u8; 32] {
        self.candidate_sha256
    }
    /// Exact manifest identity embedded in this receipt.
    #[must_use]
    pub const fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_sha256
    }
    /// Framed then payload digests for the eight exact artifact roles.
    #[must_use]
    pub const fn artifact_role_digests(&self) -> [[u8; 32]; 16] {
        self.artifact_role_digests
    }
}
impl KagemushaRecursiveSpendCryptographicReviewCheckV4 {
    /// Exact canonical check order required by every production V4 review.
    pub const ALL: [Self; KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_CHECK_COUNT_V4] = [
        Self::RecursiveCircuitConstraintCoverage,
        Self::RecursiveCycleAndTranscriptBinding,
        Self::PublicInputAndStateTransitionBinding,
        Self::ArtifactParameterAndVerifyingKeyBinding,
        Self::NullifierReplayAndFinalityBinding,
        Self::ParserCanonicalizationAndResourceBounds,
    ];
}
impl KagemushaRecursiveSpendCryptographicReviewPayloadV4 {
    /// Construct the canonical approved-review payload for an immutable candidate.
    ///
    /// The six check-evidence digests must follow
    /// [`KagemushaRecursiveSpendCryptographicReviewCheckV4::ALL`]. Final release
    /// authentication still validates every digest and reviewer signature.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when the supplied inputs fail canonical validation or required contextual bindings.
    pub fn approved(
        candidate: &KagemushaRecursiveSpendCandidateV4,
        qualification_receipt_sha256: [u8; 32],
        qualified_candidate_sha256: [u8; 32],
        report_sha256: [u8; 32],
        check_evidence_sha256: [[u8; 32];
            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_CHECK_COUNT_V4],
    ) -> Result<Self, KagemushaValidationError> {
        let subject = candidate.cryptographic_review_subject(
            qualification_receipt_sha256,
            qualified_candidate_sha256,
        )?;
        let mut evidence_digests = std::collections::BTreeSet::new();
        evidence_digests.insert(subject.candidate_sha256);
        evidence_digests.insert(subject.qualification_receipt_sha256);
        evidence_digests.insert(subject.qualified_candidate_sha256);
        if report_sha256 == [0; 32] || !evidence_digests.insert(report_sha256) {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v4.cryptographic_review_evidence",
            });
        }
        for evidence_sha256 in &check_evidence_sha256 {
            if *evidence_sha256 == [0; 32] || !evidence_digests.insert(*evidence_sha256) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v4.cryptographic_review_evidence",
                });
            }
        }
        let checks = KagemushaRecursiveSpendCryptographicReviewCheckV4::ALL
            .into_iter()
            .zip(check_evidence_sha256)
            .map(|(check, evidence_sha256)| {
                KagemushaRecursiveSpendCryptographicReviewCheckResultV4 {
                    check,
                    status: KagemushaRecursiveSpendCryptographicReviewCheckStatusV4::Passed,
                    evidence_sha256,
                }
            })
            .collect();
        Ok(Self {
            domain: KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_DOMAIN_V4.to_owned(),
            subject,
            decision: KagemushaRecursiveSpendCryptographicReviewDecisionV4::Approved,
            report_sha256,
            artifact_roles: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4
                .map(str::to_owned)
                .to_vec(),
            checks,
        })
    }
}
impl KagemushaRecursiveSpendCryptographicReviewPayloadV5 {
    /// Construct the canonical approved-review payload for an immutable V5 candidate.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaValidationError`] when identities or evidence digests are invalid.
    pub fn approved(
        candidate: &KagemushaRecursiveSpendCandidateV5,
        qualification_receipt_sha256: [u8; 32],
        qualified_candidate_sha256: [u8; 32],
        report_sha256: [u8; 32],
        check_evidence_sha256: [[u8; 32];
            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_CHECK_COUNT_V4],
    ) -> Result<Self, KagemushaValidationError> {
        let subject = candidate.cryptographic_review_subject(
            qualification_receipt_sha256,
            qualified_candidate_sha256,
        )?;
        let mut evidence_digests = std::collections::BTreeSet::new();
        evidence_digests.insert(subject.candidate_sha256);
        evidence_digests.insert(subject.qualification_receipt_sha256);
        evidence_digests.insert(subject.qualified_candidate_sha256);
        if report_sha256 == [0; 32] || !evidence_digests.insert(report_sha256) {
            return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                field: "pasta_cycle.v5.cryptographic_review_evidence",
            });
        }
        for evidence_sha256 in &check_evidence_sha256 {
            if *evidence_sha256 == [0; 32] || !evidence_digests.insert(*evidence_sha256) {
                return Err(KagemushaValidationError::InvalidRecursiveSpendProof {
                    field: "pasta_cycle.v5.cryptographic_review_evidence",
                });
            }
        }
        let checks = KagemushaRecursiveSpendCryptographicReviewCheckV4::ALL
            .into_iter()
            .zip(check_evidence_sha256)
            .map(|(check, evidence_sha256)| {
                KagemushaRecursiveSpendCryptographicReviewCheckResultV4 {
                    check,
                    status: KagemushaRecursiveSpendCryptographicReviewCheckStatusV4::Passed,
                    evidence_sha256,
                }
            })
            .collect();
        Ok(Self {
            domain: KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_DOMAIN_V5.to_owned(),
            subject,
            decision: KagemushaRecursiveSpendCryptographicReviewDecisionV4::Approved,
            report_sha256,
            artifact_roles: KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4
                .map(str::to_owned)
                .to_vec(),
            checks,
        })
    }
}
impl KagemushaRecursiveSpendReleaseApprovalRoleV1 {
    const fn index(self) -> usize {
        match self {
            Self::Release => 0,
            Self::CryptographicReview => 1,
            Self::PhysicalDeviceBenchmark => 2,
        }
    }
}
impl KagemushaRecursiveSpendReleaseAttestationSubjectV4 {
    /// Return the exact V4 domain- and role-separated approval payload.
    #[must_use]
    pub fn approval_payload(
        &self,
        role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
    ) -> KagemushaRecursiveSpendReleaseApprovalPayloadV4 {
        KagemushaRecursiveSpendReleaseApprovalPayloadV4 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_APPROVAL_DOMAIN_V4.to_owned(),
            role,
            subject: self.clone(),
        }
    }
}
impl KagemushaRecursiveSpendReleaseAttestationSubjectV5 {
    /// Return the exact V5 domain- and role-separated approval payload.
    #[must_use]
    pub fn approval_payload(
        &self,
        role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
    ) -> KagemushaRecursiveSpendReleaseApprovalPayloadV5 {
        KagemushaRecursiveSpendReleaseApprovalPayloadV5 {
            domain: KAGEMUSHA_RECURSIVE_SPEND_RELEASE_APPROVAL_DOMAIN_V5.to_owned(),
            role,
            subject: self.clone(),
        }
    }
}
impl KagemushaRecursiveSpendReleasePolicyV1 {
    /// Validate canonical role order, thresholds, signer order, and role independence.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaReleaseVerificationError> {
        let expected_roles = [
            KagemushaRecursiveSpendReleaseApprovalRoleV1::Release,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
        ];
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_POLICY_SCHEMA_V1
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V1
            || !is_kagemusha_portable_identifier(&self.policy_id)
            || self.internal_validation_runner_identity_sha256 == [0; 32]
            || self.roles.len() != expected_roles.len()
        {
            return Err(KagemushaReleaseVerificationError::InvalidPolicy);
        }
        let mut all_signers = std::collections::BTreeSet::new();
        let mut required_approvals = 0_usize;
        for (role_policy, expected_role) in self.roles.iter().zip(expected_roles) {
            let signer_count = role_policy.authorized_signers.len();
            if role_policy.role != expected_role
                || signer_count == 0
                || signer_count > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1
                || role_policy.threshold == 0
                || usize::from(role_policy.threshold) > signer_count
                || !role_policy
                    .authorized_signers
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
                || role_policy
                    .authorized_signers
                    .iter()
                    .any(|signer| !all_signers.insert(signer.clone()))
            {
                return Err(KagemushaReleaseVerificationError::InvalidPolicy);
            }
            required_approvals += usize::from(role_policy.threshold);
        }
        if required_approvals > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1 {
            return Err(KagemushaReleaseVerificationError::InvalidPolicy);
        }
        Ok(())
    }
    fn role_policy(
        &self,
        role: KagemushaRecursiveSpendReleaseApprovalRoleV1,
    ) -> Option<&KagemushaRecursiveSpendReleaseRolePolicyV1> {
        self.roles
            .get(role.index())
            .filter(|policy| policy.role == role)
    }
}
impl KagemushaRecursiveSpendCryptographicReviewEvidenceV4 {
    fn validate_against_candidate(
        &self,
        candidate: &KagemushaRecursiveSpendCandidateV4,
        qualification_receipt_sha256: [u8; 32],
        qualified_candidate_sha256: [u8; 32],
    ) -> Result<Vec<PublicKey>, KagemushaReleaseVerificationError> {
        let expected_subject = candidate
            .cryptographic_review_subject(qualification_receipt_sha256, qualified_candidate_sha256)
            .map_err(|_| KagemushaReleaseVerificationError::InvalidCryptographicReview)?;
        let expected_artifact_roles =
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.map(str::to_owned);
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_SCHEMA_V4
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_VERSION_V4
            || self.payload.domain != KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_DOMAIN_V4
            || self.payload.subject != expected_subject
            || self.payload.decision
                != KagemushaRecursiveSpendCryptographicReviewDecisionV4::Approved
            || self.payload.artifact_roles != expected_artifact_roles
            || self.payload.checks.len()
                != KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_CHECK_COUNT_V4
            || self.approvals.is_empty()
            || self.approvals.len() > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1
            || !self
                .approvals
                .windows(2)
                .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(KagemushaReleaseVerificationError::InvalidCryptographicReview);
        }
        let mut evidence_digests = std::collections::BTreeSet::new();
        evidence_digests.insert(self.payload.subject.candidate_sha256);
        evidence_digests.insert(self.payload.subject.qualification_receipt_sha256);
        evidence_digests.insert(self.payload.subject.qualified_candidate_sha256);
        if self.payload.report_sha256 == [0; 32]
            || !evidence_digests.insert(self.payload.report_sha256)
        {
            return Err(KagemushaReleaseVerificationError::InvalidCryptographicReview);
        }
        for (result, expected_check) in self
            .payload
            .checks
            .iter()
            .zip(KagemushaRecursiveSpendCryptographicReviewCheckV4::ALL)
        {
            if result.check != expected_check
                || result.status != KagemushaRecursiveSpendCryptographicReviewCheckStatusV4::Passed
                || result.evidence_sha256 == [0; 32]
                || !evidence_digests.insert(result.evidence_sha256)
            {
                return Err(KagemushaReleaseVerificationError::InvalidCryptographicReview);
            }
        }
        let mut reviewer_keys = Vec::with_capacity(self.approvals.len());
        for approval in &self.approvals {
            approval
                .signature
                .verify(&approval.public_key, &self.payload)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidSignature {
                    role: KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
                })?;
            reviewer_keys.push(approval.public_key.clone());
        }
        Ok(reviewer_keys)
    }
    /// Decode canonical Norito review bytes and validate their candidate binding.
    ///
    /// This structural entry point verifies every embedded signature. Release
    /// authentication additionally authorizes those identities against the local
    /// policy and binds the exact same reviewer set into the release attestation.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_canonical_bytes_against_candidate(
        bytes: &[u8],
        candidate: &KagemushaRecursiveSpendCandidateV4,
        qualification_receipt_sha256: [u8; 32],
        qualified_candidate_sha256: [u8; 32],
    ) -> Result<Vec<PublicKey>, KagemushaReleaseVerificationError> {
        if bytes.is_empty()
            || bytes.len() > KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4
        {
            return Err(KagemushaReleaseVerificationError::EvidenceMismatch {
                role: KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            });
        }
        let decode_limits = norito::core::DecodeLimits::new(
            16 * 1024,
            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
            128 * 1024,
            4 * KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
            64,
        );
        let evidence: Self = norito::decode_canonical_with_limits(bytes, decode_limits)
            .map_err(|_| KagemushaReleaseVerificationError::InvalidCryptographicReview)?;
        evidence.validate_against_candidate(
            candidate,
            qualification_receipt_sha256,
            qualified_candidate_sha256,
        )
    }
    fn authenticate_canonical_bytes(
        bytes: &[u8],
        candidate: &KagemushaRecursiveSpendCandidateV4,
        qualification_receipt_sha256: [u8; 32],
        qualified_candidate_sha256: [u8; 32],
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
    ) -> Result<Vec<PublicKey>, KagemushaReleaseVerificationError> {
        policy.validate()?;
        let reviewer_keys = Self::validate_canonical_bytes_against_candidate(
            bytes,
            candidate,
            qualification_receipt_sha256,
            qualified_candidate_sha256,
        )?;
        let role = KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview;
        let role_policy = policy
            .role_policy(role)
            .ok_or(KagemushaReleaseVerificationError::InvalidPolicy)?;
        for public_key in &reviewer_keys {
            if role_policy
                .authorized_signers
                .binary_search(public_key)
                .is_err()
            {
                return Err(KagemushaReleaseVerificationError::UnknownSigner { role });
            }
        }
        let collected = u16::try_from(reviewer_keys.len())
            .map_err(|_| KagemushaReleaseVerificationError::InvalidCryptographicReview)?;
        if collected < role_policy.threshold {
            return Err(KagemushaReleaseVerificationError::InsufficientThreshold {
                role,
                collected,
                required: role_policy.threshold,
            });
        }
        Ok(reviewer_keys)
    }
}
impl KagemushaRecursiveSpendCryptographicReviewEvidenceV5 {
    fn validate_against_candidate(
        &self,
        candidate: &KagemushaRecursiveSpendCandidateV5,
        qualification_receipt_sha256: [u8; 32],
        qualified_candidate_sha256: [u8; 32],
    ) -> Result<Vec<PublicKey>, KagemushaReleaseVerificationError> {
        let expected_subject = candidate
            .cryptographic_review_subject(qualification_receipt_sha256, qualified_candidate_sha256)
            .map_err(|_| KagemushaReleaseVerificationError::InvalidCryptographicReview)?;
        let expected_artifact_roles =
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.map(str::to_owned);
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_SCHEMA_V5
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_VERSION_V5
            || self.payload.domain != KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_DOMAIN_V5
            || self.payload.subject != expected_subject
            || self.payload.decision
                != KagemushaRecursiveSpendCryptographicReviewDecisionV4::Approved
            || self.payload.artifact_roles != expected_artifact_roles
            || self.payload.checks.len()
                != KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_CHECK_COUNT_V4
            || self.approvals.is_empty()
            || self.approvals.len() > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1
            || !self
                .approvals
                .windows(2)
                .all(|pair| pair[0].public_key < pair[1].public_key)
        {
            return Err(KagemushaReleaseVerificationError::InvalidCryptographicReview);
        }
        let mut evidence_digests = std::collections::BTreeSet::new();
        evidence_digests.insert(self.payload.subject.candidate_sha256);
        evidence_digests.insert(self.payload.subject.qualification_receipt_sha256);
        evidence_digests.insert(self.payload.subject.qualified_candidate_sha256);
        if self.payload.report_sha256 == [0; 32]
            || !evidence_digests.insert(self.payload.report_sha256)
        {
            return Err(KagemushaReleaseVerificationError::InvalidCryptographicReview);
        }
        for (result, expected_check) in self
            .payload
            .checks
            .iter()
            .zip(KagemushaRecursiveSpendCryptographicReviewCheckV4::ALL)
        {
            if result.check != expected_check
                || result.status != KagemushaRecursiveSpendCryptographicReviewCheckStatusV4::Passed
                || result.evidence_sha256 == [0; 32]
                || !evidence_digests.insert(result.evidence_sha256)
            {
                return Err(KagemushaReleaseVerificationError::InvalidCryptographicReview);
            }
        }
        let mut reviewer_keys = Vec::with_capacity(self.approvals.len());
        for approval in &self.approvals {
            approval
                .signature
                .verify(&approval.public_key, &self.payload)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidSignature {
                    role: KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
                })?;
            reviewer_keys.push(approval.public_key.clone());
        }
        Ok(reviewer_keys)
    }
    /// Decode canonical V5 review bytes and validate their exact candidate binding.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when structure, signatures, or bindings fail.
    pub fn validate_canonical_bytes_against_candidate(
        bytes: &[u8],
        candidate: &KagemushaRecursiveSpendCandidateV5,
        qualification_receipt_sha256: [u8; 32],
        qualified_candidate_sha256: [u8; 32],
    ) -> Result<Vec<PublicKey>, KagemushaReleaseVerificationError> {
        if bytes.is_empty()
            || bytes.len() > KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4
        {
            return Err(KagemushaReleaseVerificationError::EvidenceMismatch {
                role: KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
            });
        }
        let decode_limits = norito::core::DecodeLimits::new(
            16 * 1024,
            KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
            128 * 1024,
            4 * KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
            64,
        );
        let evidence: Self = norito::decode_canonical_with_limits(bytes, decode_limits)
            .map_err(|_| KagemushaReleaseVerificationError::InvalidCryptographicReview)?;
        evidence.validate_against_candidate(
            candidate,
            qualification_receipt_sha256,
            qualified_candidate_sha256,
        )
    }
    fn authenticate_canonical_bytes(
        bytes: &[u8],
        candidate: &KagemushaRecursiveSpendCandidateV5,
        qualification_receipt_sha256: [u8; 32],
        qualified_candidate_sha256: [u8; 32],
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
    ) -> Result<Vec<PublicKey>, KagemushaReleaseVerificationError> {
        policy.validate()?;
        let reviewer_keys = Self::validate_canonical_bytes_against_candidate(
            bytes,
            candidate,
            qualification_receipt_sha256,
            qualified_candidate_sha256,
        )?;
        let role = KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview;
        let role_policy = policy
            .role_policy(role)
            .ok_or(KagemushaReleaseVerificationError::InvalidPolicy)?;
        for public_key in &reviewer_keys {
            if role_policy
                .authorized_signers
                .binary_search(public_key)
                .is_err()
            {
                return Err(KagemushaReleaseVerificationError::UnknownSigner { role });
            }
        }
        let collected = u16::try_from(reviewer_keys.len())
            .map_err(|_| KagemushaReleaseVerificationError::InvalidCryptographicReview)?;
        if collected < role_policy.threshold {
            return Err(KagemushaReleaseVerificationError::InsufficientThreshold {
                role,
                collected,
                required: role_policy.threshold,
            });
        }
        Ok(reviewer_keys)
    }
}

fn validate_internal_validation_receipt_v4(
    manifest: &KagemushaRecursiveSpendArtifactManifestV4,
    candidate: &KagemushaRecursiveSpendCandidateV4,
    bytes: &[u8],
    expected_runner_identity_sha256: Option<[u8; 32]>,
) -> Result<(), KagemushaReleaseVerificationError> {
    let receipt = KagemushaRecursiveSpendInternalValidationReceiptV1::decode_canonical(bytes)
        .map_err(|_| KagemushaReleaseVerificationError::InvalidInternalValidationReceipt)?;
    let receipt_sha256: [u8; 32] = Sha256::digest(bytes).into();
    let candidate_sha256 = candidate
        .sha256()
        .map_err(|_| KagemushaReleaseVerificationError::InvalidInternalValidationReceipt)?;
    let body = &receipt.body;
    if receipt_sha256 != manifest.internal_validation_receipt_sha256
        || body.candidate_sha256 != candidate_sha256
        || body.qualification_receipt_sha256 != manifest.qualification_receipt_sha256
        || body.qualified_candidate_sha256 != manifest.qualified_candidate_sha256
        || body.source_commit != manifest.source_commit
        || body.source_tree_sha256 != manifest.source_tree_sha256
        || body.source_repo_dirty != manifest.source_repo_dirty
        || body.reviewed_source_closure_descriptor_sha256
            != manifest.reviewed_source_closure_descriptor_sha256
        || body.authenticated_source_seal_projection_sha256
            != manifest.authenticated_source_seal_projection_sha256
        || body.tracked_cargo_lock.sha256
            != manifest.reviewed_source_closure.ignored_cargo_lock_sha256
        || body.tracked_cargo_lock.size_bytes
            != manifest
                .reviewed_source_closure
                .ignored_cargo_lock_size_bytes
        || body.reviewed_cargo_binary_sha256 != manifest.reviewed_cargo_binary_sha256
        || body.reviewed_rustc_binary_sha256 != manifest.reviewed_rustc_binary_sha256
        || body.generator_binary_sha256 != manifest.generator_binary_sha256
        || body.sealed_candidate_build_report_sha256
            != manifest.sealed_candidate_build_report_sha256
        || expected_runner_identity_sha256
            .is_some_and(|expected| body.validation_runner_identity_sha256 != expected)
    {
        return Err(KagemushaReleaseVerificationError::InvalidInternalValidationReceipt);
    }
    Ok(())
}

impl KagemushaAuthenticatedReleaseV4 {
    fn verify_attestation(
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
        attestation: &KagemushaRecursiveSpendReleaseAttestationV4,
    ) -> Result<Self, KagemushaReleaseVerificationError> {
        manifest
            .validate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        policy.validate()?;
        let expected_subject = manifest.release_attestation_subject()?;
        if attestation.schema != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V4
            || attestation.version != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4
            || attestation.subject != expected_subject
            || attestation.approvals.is_empty()
            || attestation.approvals.len() > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1
        {
            return Err(KagemushaReleaseVerificationError::InvalidAttestation);
        }
        let attestation_bytes = norito::encode_canonical(attestation)
            .map_err(|_| KagemushaReleaseVerificationError::InvalidAttestation)?;
        let attestation_sha256: [u8; 32] = Sha256::digest(attestation_bytes).into();
        if attestation_sha256 != manifest.release_attestation_sha256 {
            return Err(KagemushaReleaseVerificationError::InvalidAttestation);
        }
        let mut counts = [0_u16; 3];
        let mut approved_signers = Vec::with_capacity(attestation.approvals.len());
        let mut previous: Option<(KagemushaRecursiveSpendReleaseApprovalRoleV1, &PublicKey)> = None;
        for approval in &attestation.approvals {
            let identity = (approval.role, &approval.public_key);
            if previous.is_some_and(|previous| previous >= identity) {
                return Err(KagemushaReleaseVerificationError::DuplicateOrUnorderedSigner);
            }
            previous = Some(identity);
            let role_policy = policy.role_policy(approval.role).ok_or(
                KagemushaReleaseVerificationError::UnknownSigner {
                    role: approval.role,
                },
            )?;
            if role_policy
                .authorized_signers
                .binary_search(&approval.public_key)
                .is_err()
            {
                return Err(KagemushaReleaseVerificationError::UnknownSigner {
                    role: approval.role,
                });
            }
            let payload = expected_subject.approval_payload(approval.role);
            approval
                .signature
                .verify(&approval.public_key, &payload)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidSignature {
                    role: approval.role,
                })?;
            counts[approval.role.index()] = counts[approval.role.index()].saturating_add(1);
            approved_signers.push(KagemushaRecursiveSpendApprovedSignerV1 {
                role: approval.role,
                public_key: approval.public_key.clone(),
            });
        }
        for role_policy in &policy.roles {
            let collected = counts[role_policy.role.index()];
            if collected < role_policy.threshold {
                return Err(KagemushaReleaseVerificationError::InsufficientThreshold {
                    role: role_policy.role,
                    collected,
                    required: role_policy.threshold,
                });
            }
        }
        let manifest_sha256 = Sha256::digest(
            norito::encode_canonical(manifest)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?,
        )
        .into();
        let release_policy_sha256 = Sha256::digest(
            norito::encode_canonical(policy)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidPolicy)?,
        )
        .into();
        Ok(Self {
            manifest: manifest.clone(),
            manifest_sha256,
            release_attestation_sha256: attestation_sha256,
            release_policy_sha256,
            approved_signers,
        })
    }
    /// Authenticate a V4 release and hash-check its exact internal and external evidence files.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when structural, policy, signature, or cryptographic authentication fails.
    pub fn verify(
        manifest: &KagemushaRecursiveSpendArtifactManifestV4,
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
        attestation: &KagemushaRecursiveSpendReleaseAttestationV4,
        internal_validation_receipt: &[u8],
        benchmark_evidence: &[u8],
        cryptographic_review: &[u8],
    ) -> Result<Self, KagemushaReleaseVerificationError> {
        for (role, bytes, expected_digest, maximum_bytes) in [
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
                benchmark_evidence,
                manifest.benchmark_evidence_sha256,
                KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
            ),
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
                cryptographic_review,
                manifest.cryptographic_review_sha256,
                KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
            ),
        ] {
            if bytes.is_empty()
                || bytes.len() > maximum_bytes
                || <[u8; 32]>::from(Sha256::digest(bytes)) != expected_digest
            {
                return Err(KagemushaReleaseVerificationError::EvidenceMismatch { role });
            }
        }
        let candidate = manifest
            .immutable_candidate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        let review_signers =
            KagemushaRecursiveSpendCryptographicReviewEvidenceV4::authenticate_canonical_bytes(
                cryptographic_review,
                &candidate,
                manifest.qualification_receipt_sha256,
                manifest.qualified_candidate_sha256,
                policy,
            )?;
        validate_internal_validation_receipt_v4(
            manifest,
            &candidate,
            internal_validation_receipt,
            Some(policy.internal_validation_runner_identity_sha256),
        )?;
        let authenticated = Self::verify_attestation(manifest, policy, attestation)?;
        let attested_review_signers = authenticated
            .approved_signers
            .iter()
            .filter(|signer| {
                signer.role == KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview
            })
            .map(|signer| signer.public_key.clone())
            .collect::<Vec<_>>();
        if review_signers != attested_review_signers {
            return Err(KagemushaReleaseVerificationError::InvalidCryptographicReview);
        }
        Ok(authenticated)
    }
    /// Authenticated V4 manifest selected by this runtime proof.
    #[must_use]
    pub fn manifest(&self) -> &KagemushaRecursiveSpendArtifactManifestV4 {
        &self.manifest
    }
    /// SHA-256 of the exact canonical V4 manifest.
    #[must_use]
    pub const fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_sha256
    }
    /// SHA-256 of the exact signed V4 release envelope.
    #[must_use]
    pub const fn release_attestation_sha256(&self) -> [u8; 32] {
        self.release_attestation_sha256
    }
    /// SHA-256 of the exact locally trusted release policy.
    #[must_use]
    pub const fn release_policy_sha256(&self) -> [u8; 32] {
        self.release_policy_sha256
    }
    /// Canonically ordered role/signer identities whose V4 approvals verified.
    #[must_use]
    pub fn approved_signers(&self) -> &[KagemushaRecursiveSpendApprovedSignerV1] {
        &self.approved_signers
    }
}
impl KagemushaAuthenticatedReleaseV5 {
    fn verify_attestation(
        manifest: &KagemushaRecursiveSpendArtifactManifestV5,
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
        attestation: &KagemushaRecursiveSpendReleaseAttestationV5,
    ) -> Result<Self, KagemushaReleaseVerificationError> {
        manifest
            .validate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        policy.validate()?;
        let expected_subject = manifest.release_attestation_subject()?;
        if attestation.schema != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_ATTESTATION_SCHEMA_V5
            || attestation.version != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V5
            || attestation.subject != expected_subject
            || attestation.approvals.is_empty()
            || attestation.approvals.len() > KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1
        {
            return Err(KagemushaReleaseVerificationError::InvalidAttestation);
        }
        let attestation_bytes = norito::encode_canonical(attestation)
            .map_err(|_| KagemushaReleaseVerificationError::InvalidAttestation)?;
        let attestation_sha256: [u8; 32] = Sha256::digest(attestation_bytes).into();
        if attestation_sha256 != manifest.release_attestation_sha256 {
            return Err(KagemushaReleaseVerificationError::InvalidAttestation);
        }
        let mut counts = [0_u16; 3];
        let mut approved_signers = Vec::with_capacity(attestation.approvals.len());
        let mut previous: Option<(KagemushaRecursiveSpendReleaseApprovalRoleV1, &PublicKey)> = None;
        for approval in &attestation.approvals {
            let identity = (approval.role, &approval.public_key);
            if previous.is_some_and(|previous| previous >= identity) {
                return Err(KagemushaReleaseVerificationError::DuplicateOrUnorderedSigner);
            }
            previous = Some(identity);
            let role_policy = policy.role_policy(approval.role).ok_or(
                KagemushaReleaseVerificationError::UnknownSigner {
                    role: approval.role,
                },
            )?;
            if role_policy
                .authorized_signers
                .binary_search(&approval.public_key)
                .is_err()
            {
                return Err(KagemushaReleaseVerificationError::UnknownSigner {
                    role: approval.role,
                });
            }
            let payload = expected_subject.approval_payload(approval.role);
            approval
                .signature
                .verify(&approval.public_key, &payload)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidSignature {
                    role: approval.role,
                })?;
            counts[approval.role.index()] = counts[approval.role.index()].saturating_add(1);
            approved_signers.push(KagemushaRecursiveSpendApprovedSignerV1 {
                role: approval.role,
                public_key: approval.public_key.clone(),
            });
        }
        for role_policy in &policy.roles {
            let collected = counts[role_policy.role.index()];
            if collected < role_policy.threshold {
                return Err(KagemushaReleaseVerificationError::InsufficientThreshold {
                    role: role_policy.role,
                    collected,
                    required: role_policy.threshold,
                });
            }
        }
        let manifest_sha256 = Sha256::digest(
            norito::encode_canonical(manifest)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?,
        )
        .into();
        let release_policy_sha256 = Sha256::digest(
            norito::encode_canonical(policy)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidPolicy)?,
        )
        .into();
        Ok(Self {
            manifest: manifest.clone(),
            manifest_sha256,
            release_attestation_sha256: attestation_sha256,
            release_policy_sha256,
            approved_signers,
        })
    }
    /// Authenticate a V5 release and hash-check its exact evidence files.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when policy, signatures, or evidence fail.
    pub fn verify(
        manifest: &KagemushaRecursiveSpendArtifactManifestV5,
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
        attestation: &KagemushaRecursiveSpendReleaseAttestationV5,
        benchmark_evidence: &[u8],
        cryptographic_review: &[u8],
    ) -> Result<Self, KagemushaReleaseVerificationError> {
        for (role, bytes, expected_digest, maximum_bytes) in [
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
                benchmark_evidence,
                manifest.benchmark_evidence_sha256,
                KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
            ),
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
                cryptographic_review,
                manifest.cryptographic_review_sha256,
                KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
            ),
        ] {
            if bytes.is_empty()
                || bytes.len() > maximum_bytes
                || <[u8; 32]>::from(Sha256::digest(bytes)) != expected_digest
            {
                return Err(KagemushaReleaseVerificationError::EvidenceMismatch { role });
            }
        }
        let candidate = manifest
            .immutable_candidate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        let review_signers =
            KagemushaRecursiveSpendCryptographicReviewEvidenceV5::authenticate_canonical_bytes(
                cryptographic_review,
                &candidate,
                manifest.qualification_receipt_sha256,
                manifest.qualified_candidate_sha256,
                policy,
            )?;
        let authenticated = Self::verify_attestation(manifest, policy, attestation)?;
        let attested_review_signers = authenticated
            .approved_signers
            .iter()
            .filter(|signer| {
                signer.role == KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview
            })
            .map(|signer| signer.public_key.clone())
            .collect::<Vec<_>>();
        if review_signers != attested_review_signers {
            return Err(KagemushaReleaseVerificationError::InvalidCryptographicReview);
        }
        Ok(authenticated)
    }
    /// Authenticated V5 manifest selected by this runtime proof.
    #[must_use]
    pub fn manifest(&self) -> &KagemushaRecursiveSpendArtifactManifestV5 {
        &self.manifest
    }
    /// SHA-256 of the exact canonical V5 manifest.
    #[must_use]
    pub const fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_sha256
    }
    /// SHA-256 of the exact signed V5 release envelope.
    #[must_use]
    pub const fn release_attestation_sha256(&self) -> [u8; 32] {
        self.release_attestation_sha256
    }
    /// SHA-256 of the exact locally trusted release policy.
    #[must_use]
    pub const fn release_policy_sha256(&self) -> [u8; 32] {
        self.release_policy_sha256
    }
    /// Canonically ordered role/signer identities whose V5 approvals verified.
    #[must_use]
    pub fn approved_signers(&self) -> &[KagemushaRecursiveSpendApprovedSignerV1] {
        &self.approved_signers
    }
}
impl KagemushaRecursiveSpendPromotedReleaseV4 {
    /// Validate the standalone Kagemusha V4 promotion marker.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate(&self) -> Result<(), KagemushaReleaseVerificationError> {
        let digests = [
            self.candidate_sha256,
            self.qualification_receipt_sha256,
            self.qualified_candidate_sha256,
            self.internal_validation_receipt_sha256,
            self.manifest_sha256,
            self.release_attestation_sha256,
            self.release_policy_sha256,
        ];
        let digests_are_distinct_and_nonzero = digests.iter().all(|digest| *digest != [0; 32])
            && digests
                .iter()
                .enumerate()
                .all(|(index, digest)| !digests[..index].contains(digest));
        let signers_are_canonical = !self.approved_signers.is_empty()
            && self.approved_signers.len() <= KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1
            && self
                .approved_signers
                .windows(2)
                .all(|pair| pair[0] < pair[1]);
        let mut represented_roles = [false; 3];
        for signer in &self.approved_signers {
            represented_roles[signer.role.index()] = true;
        }
        let expected_artifact_roles =
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.map(str::to_owned);
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V4
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V4
            || !is_kagemusha_portable_identifier(&self.generation)
            || self.authenticated_source_seal_projection_sha256 == [0; 32]
            || self.reviewed_cargo_binary_sha256 == [0; 32]
            || self.reviewed_rustc_binary_sha256 == [0; 32]
            || self.generator_binary_sha256 == [0; 32]
            || self.sealed_candidate_build_report_sha256 == [0; 32]
            || !digests_are_distinct_and_nonzero
            || !signers_are_canonical
            || represented_roles
                .into_iter()
                .any(|represented| !represented)
            || !self.artifact_inventory_verified
            || self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4
            || self.artifact_roles != expected_artifact_roles
            || self.max_proof_bytes == 0
            || self.max_proof_bytes > KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
        {
            return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
        }
        Ok(())
    }
    /// Require this marker to identify one exact authenticated V4 release.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_against_authenticated_release(
        &self,
        release: &KagemushaAuthenticatedReleaseV4,
    ) -> Result<(), KagemushaReleaseVerificationError> {
        self.validate()?;
        let candidate_sha256 = release
            .manifest()
            .immutable_candidate()
            .and_then(|candidate| candidate.sha256())
            .map_err(|_| KagemushaReleaseVerificationError::InvalidPromotionRecord)?;
        if self.candidate_sha256 != candidate_sha256
            || self.qualification_receipt_sha256 != release.manifest().qualification_receipt_sha256
            || self.qualified_candidate_sha256 != release.manifest().qualified_candidate_sha256
            || self.internal_validation_receipt_sha256
                != release.manifest().internal_validation_receipt_sha256
            || self.generation != release.manifest().generation
            || self.authenticated_source_seal_projection_sha256
                != release
                    .manifest()
                    .authenticated_source_seal_projection_sha256
            || self.reviewed_cargo_binary_sha256 != release.manifest().reviewed_cargo_binary_sha256
            || self.reviewed_rustc_binary_sha256 != release.manifest().reviewed_rustc_binary_sha256
            || self.generator_binary_sha256 != release.manifest().generator_binary_sha256
            || self.sealed_candidate_build_report_sha256
                != release.manifest().sealed_candidate_build_report_sha256
            || self.manifest_sha256 != release.manifest_sha256()
            || self.release_attestation_sha256 != release.release_attestation_sha256()
            || self.release_policy_sha256 != release.release_policy_sha256()
            || self.approved_signers != release.approved_signers()
            || self.max_proof_bytes != release.manifest().max_proof_bytes
        {
            return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
        }
        Ok(())
    }
    /// Require this marker to bind the immutable candidate and finalized release.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_against_candidate_and_authenticated_release(
        &self,
        candidate: &KagemushaRecursiveSpendCandidateV4,
        release: &KagemushaAuthenticatedReleaseV4,
    ) -> Result<(), KagemushaReleaseVerificationError> {
        self.validate_against_authenticated_release(release)?;
        let candidate_sha256 = candidate
            .sha256()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidPromotionRecord)?;
        if self.candidate_sha256 != candidate_sha256
            || self.qualified_candidate_sha256
                != kagemusha_recursive_spend_qualified_candidate_sha256_v4(
                    candidate_sha256,
                    self.qualification_receipt_sha256,
                )
            || candidate.manifest.generation != release.manifest().generation
            || candidate.manifest.source_commit != release.manifest().source_commit
            || candidate.manifest.source_tree_sha256 != release.manifest().source_tree_sha256
            || candidate.manifest.source_repo_dirty != release.manifest().source_repo_dirty
            || candidate.manifest.reviewed_source_closure
                != release.manifest().reviewed_source_closure
            || candidate.manifest.reviewed_source_closure_descriptor_sha256
                != release.manifest().reviewed_source_closure_descriptor_sha256
            || candidate
                .manifest
                .authenticated_source_seal_projection_sha256
                != release
                    .manifest()
                    .authenticated_source_seal_projection_sha256
            || candidate.manifest.reviewed_cargo_binary_sha256
                != release.manifest().reviewed_cargo_binary_sha256
            || candidate.manifest.reviewed_rustc_binary_sha256
                != release.manifest().reviewed_rustc_binary_sha256
            || candidate.manifest.generator_binary_sha256
                != release.manifest().generator_binary_sha256
            || candidate.manifest.sealed_candidate_build_report_sha256
                != release.manifest().sealed_candidate_build_report_sha256
            || candidate.manifest.network_id != release.manifest().network_id
            || candidate.manifest.asset != release.manifest().asset
            || candidate.manifest.asset_scale != release.manifest().asset_scale
            || candidate.manifest.activation_height != release.manifest().activation_height
            || candidate.manifest.withdrawal_height != release.manifest().withdrawal_height
            || candidate.manifest.max_proof_bytes != release.manifest().max_proof_bytes
            || candidate.manifest.profiles != release.manifest().profiles
            || candidate.manifest.topup_finality_roster_artifact
                != release.manifest().topup_finality_roster_artifact
        {
            return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
        }
        Ok(())
    }
}
impl KagemushaRecursiveSpendPromotedReleaseV5 {
    /// Validate the standalone V5 promotion marker.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when structure or identity is invalid.
    pub fn validate(&self) -> Result<(), KagemushaReleaseVerificationError> {
        let digests = [
            self.candidate_sha256,
            self.qualification_receipt_sha256,
            self.qualified_candidate_sha256,
            self.manifest_sha256,
            self.release_attestation_sha256,
            self.release_policy_sha256,
        ];
        let digests_are_distinct_and_nonzero = digests.iter().all(|digest| *digest != [0; 32])
            && digests
                .iter()
                .enumerate()
                .all(|(index, digest)| !digests[..index].contains(digest));
        let signers_are_canonical = !self.approved_signers.is_empty()
            && self.approved_signers.len() <= KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_APPROVALS_V1
            && self
                .approved_signers
                .windows(2)
                .all(|pair| pair[0] < pair[1]);
        let mut represented_roles = [false; 3];
        for signer in &self.approved_signers {
            represented_roles[signer.role.index()] = true;
        }
        let expected_artifact_roles =
            KAGEMUSHA_RECURSIVE_SPEND_ARTIFACT_ROLES_V4.map(str::to_owned);
        if self.schema != KAGEMUSHA_RECURSIVE_SPEND_PROMOTED_RELEASE_SCHEMA_V5
            || self.version != KAGEMUSHA_RECURSIVE_SPEND_RELEASE_AUTH_VERSION_V5
            || !is_kagemusha_portable_identifier(&self.generation)
            || !digests_are_distinct_and_nonzero
            || !signers_are_canonical
            || represented_roles
                .into_iter()
                .any(|represented| !represented)
            || !self.artifact_inventory_verified
            || self.bridge_abi_version != KAGEMUSHA_RECURSIVE_SPEND_NATIVE_BRIDGE_ABI_V4
            || self.artifact_roles != expected_artifact_roles
            || self.max_proof_bytes == 0
            || self.max_proof_bytes > KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
        {
            return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
        }
        Ok(())
    }
    /// Require this marker to identify one exact authenticated V5 release.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when any release identity differs.
    pub fn validate_against_authenticated_release(
        &self,
        release: &KagemushaAuthenticatedReleaseV5,
    ) -> Result<(), KagemushaReleaseVerificationError> {
        self.validate()?;
        let candidate_sha256 = release
            .manifest()
            .immutable_candidate()
            .and_then(|candidate| candidate.sha256())
            .map_err(|_| KagemushaReleaseVerificationError::InvalidPromotionRecord)?;
        if self.candidate_sha256 != candidate_sha256
            || self.qualification_receipt_sha256 != release.manifest().qualification_receipt_sha256
            || self.qualified_candidate_sha256 != release.manifest().qualified_candidate_sha256
            || self.generation != release.manifest().generation
            || self.manifest_sha256 != release.manifest_sha256()
            || self.release_attestation_sha256 != release.release_attestation_sha256()
            || self.release_policy_sha256 != release.release_policy_sha256()
            || self.approved_signers != release.approved_signers()
            || self.max_proof_bytes != release.manifest().max_proof_bytes
        {
            return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
        }
        Ok(())
    }
    /// Require this marker to bind the immutable V5 candidate and finalized release.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when any candidate/release field differs.
    pub fn validate_against_candidate_and_authenticated_release(
        &self,
        candidate: &KagemushaRecursiveSpendCandidateV5,
        release: &KagemushaAuthenticatedReleaseV5,
    ) -> Result<(), KagemushaReleaseVerificationError> {
        self.validate_against_authenticated_release(release)?;
        let candidate_sha256 = candidate
            .sha256()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidPromotionRecord)?;
        if self.candidate_sha256 != candidate_sha256
            || self.qualified_candidate_sha256
                != kagemusha_recursive_spend_qualified_candidate_sha256_v5(
                    candidate_sha256,
                    self.qualification_receipt_sha256,
                )
            || candidate.manifest.generation != release.manifest().generation
            || candidate.manifest.source_commit != release.manifest().source_commit
            || candidate.manifest.source_git_tree != release.manifest().source_git_tree
            || candidate.manifest.source_tree_sha256 != release.manifest().source_tree_sha256
            || candidate.manifest.source_repo_dirty != release.manifest().source_repo_dirty
            || candidate.manifest.reviewed_source_closure
                != release.manifest().reviewed_source_closure
            || candidate.manifest.reviewed_source_closure_descriptor_sha256
                != release.manifest().reviewed_source_closure_descriptor_sha256
            || candidate
                .manifest
                .authenticated_source_seal_projection_sha256
                != release
                    .manifest()
                    .authenticated_source_seal_projection_sha256
            || candidate.manifest.network_id != release.manifest().network_id
            || candidate.manifest.asset != release.manifest().asset
            || candidate.manifest.asset_scale != release.manifest().asset_scale
            || candidate.manifest.activation_height != release.manifest().activation_height
            || candidate.manifest.withdrawal_height != release.manifest().withdrawal_height
            || candidate.manifest.max_proof_bytes != release.manifest().max_proof_bytes
            || candidate.manifest.profiles != release.manifest().profiles
            || candidate.manifest.topup_finality_roster_artifact
                != release.manifest().topup_finality_roster_artifact
        {
            return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
        }
        Ok(())
    }
}
impl KagemushaRecursiveSpendReleaseRecordV4 {
    /// Validate deterministic release hashes without consulting local trust policy.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when a required structure, bound, authorization, or contextual binding is invalid.
    pub fn validate_structure(&self) -> Result<(), KagemushaReleaseVerificationError> {
        self.manifest
            .validate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        self.promotion_record.validate()?;
        let manifest_sha256: [u8; 32] = Sha256::digest(
            norito::encode_canonical(&self.manifest)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?,
        )
        .into();
        let attestation_sha256: [u8; 32] = Sha256::digest(
            norito::encode_canonical(&self.release_attestation)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidAttestation)?,
        )
        .into();
        let summaries = [
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
                self.physical_device_benchmark_summary.as_slice(),
                self.manifest.benchmark_evidence_sha256,
                KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
            ),
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
                self.cryptographic_review_summary.as_slice(),
                self.manifest.cryptographic_review_sha256,
                KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
            ),
        ];
        for (role, summary, expected_sha256, maximum_bytes) in summaries {
            if summary.is_empty()
                || summary.len() > maximum_bytes
                || <[u8; 32]>::from(Sha256::digest(summary)) != expected_sha256
            {
                return Err(KagemushaReleaseVerificationError::EvidenceMismatch { role });
            }
        }
        let candidate = self
            .manifest
            .immutable_candidate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        KagemushaRecursiveSpendCryptographicReviewEvidenceV4::validate_canonical_bytes_against_candidate(
            &self.cryptographic_review_summary,
            &candidate,
            self.manifest.qualification_receipt_sha256,
            self.manifest.qualified_candidate_sha256,
        )?;
        validate_internal_validation_receipt_v4(
            &self.manifest,
            &candidate,
            &self.internal_validation_receipt,
            None,
        )?;
        if attestation_sha256 != self.manifest.release_attestation_sha256
            || self.promotion_record.generation != self.manifest.generation
            || self
                .promotion_record
                .authenticated_source_seal_projection_sha256
                != self.manifest.authenticated_source_seal_projection_sha256
            || self.promotion_record.reviewed_cargo_binary_sha256
                != self.manifest.reviewed_cargo_binary_sha256
            || self.promotion_record.reviewed_rustc_binary_sha256
                != self.manifest.reviewed_rustc_binary_sha256
            || self.promotion_record.generator_binary_sha256
                != self.manifest.generator_binary_sha256
            || self.promotion_record.sealed_candidate_build_report_sha256
                != self.manifest.sealed_candidate_build_report_sha256
            || self.promotion_record.qualification_receipt_sha256
                != self.manifest.qualification_receipt_sha256
            || self.promotion_record.qualified_candidate_sha256
                != self.manifest.qualified_candidate_sha256
            || self.promotion_record.internal_validation_receipt_sha256
                != self.manifest.internal_validation_receipt_sha256
            || self.promotion_record.manifest_sha256 != manifest_sha256
            || self.promotion_record.release_attestation_sha256 != attestation_sha256
            || self.promotion_record.max_proof_bytes != self.manifest.max_proof_bytes
        {
            return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
        }
        Ok(())
    }
    /// Authenticate every signed release field against the configured policy.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when structural, policy, signature, or cryptographic authentication fails.
    pub fn authenticate(
        &self,
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
    ) -> Result<KagemushaAuthenticatedReleaseV4, KagemushaReleaseVerificationError> {
        self.validate_structure()?;
        let release = KagemushaAuthenticatedReleaseV4::verify(
            &self.manifest,
            policy,
            &self.release_attestation,
            &self.internal_validation_receipt,
            &self.physical_device_benchmark_summary,
            &self.cryptographic_review_summary,
        )?;
        self.promotion_record
            .validate_against_authenticated_release(&release)?;
        Ok(release)
    }
}
impl KagemushaRecursiveSpendReleaseRecordV5 {
    /// Validate deterministic V5 release hashes without consulting local trust policy.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when any structure or digest differs.
    pub fn validate_structure(&self) -> Result<(), KagemushaReleaseVerificationError> {
        self.manifest
            .validate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        self.promotion_record.validate()?;
        let manifest_sha256: [u8; 32] = Sha256::digest(
            norito::encode_canonical(&self.manifest)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?,
        )
        .into();
        let attestation_sha256: [u8; 32] = Sha256::digest(
            norito::encode_canonical(&self.release_attestation)
                .map_err(|_| KagemushaReleaseVerificationError::InvalidAttestation)?,
        )
        .into();
        let summaries = [
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::PhysicalDeviceBenchmark,
                self.physical_device_benchmark_summary.as_slice(),
                self.manifest.benchmark_evidence_sha256,
                KAGEMUSHA_RECURSIVE_SPEND_RELEASE_MAX_EVIDENCE_BYTES_V1,
            ),
            (
                KagemushaRecursiveSpendReleaseApprovalRoleV1::CryptographicReview,
                self.cryptographic_review_summary.as_slice(),
                self.manifest.cryptographic_review_sha256,
                KAGEMUSHA_RECURSIVE_SPEND_CRYPTOGRAPHIC_REVIEW_MAX_BYTES_V4,
            ),
        ];
        for (role, summary, expected_sha256, maximum_bytes) in summaries {
            if summary.is_empty()
                || summary.len() > maximum_bytes
                || <[u8; 32]>::from(Sha256::digest(summary)) != expected_sha256
            {
                return Err(KagemushaReleaseVerificationError::EvidenceMismatch { role });
            }
        }
        let candidate = self
            .manifest
            .immutable_candidate()
            .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
        KagemushaRecursiveSpendCryptographicReviewEvidenceV5::validate_canonical_bytes_against_candidate(
            &self.cryptographic_review_summary,
            &candidate,
            self.manifest.qualification_receipt_sha256,
            self.manifest.qualified_candidate_sha256,
        )?;
        if attestation_sha256 != self.manifest.release_attestation_sha256
            || self.promotion_record.generation != self.manifest.generation
            || self.promotion_record.qualification_receipt_sha256
                != self.manifest.qualification_receipt_sha256
            || self.promotion_record.qualified_candidate_sha256
                != self.manifest.qualified_candidate_sha256
            || self.promotion_record.manifest_sha256 != manifest_sha256
            || self.promotion_record.release_attestation_sha256 != attestation_sha256
            || self.promotion_record.max_proof_bytes != self.manifest.max_proof_bytes
        {
            return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
        }
        Ok(())
    }
    /// Authenticate every signed V5 release field against the configured policy.
    ///
    /// # Errors
    ///
    /// Returns [`KagemushaReleaseVerificationError`] when trust or signature checks fail.
    pub fn authenticate(
        &self,
        policy: &KagemushaRecursiveSpendReleasePolicyV1,
    ) -> Result<KagemushaAuthenticatedReleaseV5, KagemushaReleaseVerificationError> {
        self.validate_structure()?;
        let release = KagemushaAuthenticatedReleaseV5::verify(
            &self.manifest,
            policy,
            &self.release_attestation,
            &self.physical_device_benchmark_summary,
            &self.cryptographic_review_summary,
        )?;
        self.promotion_record
            .validate_against_authenticated_release(&release)?;
        Ok(release)
    }
}
macro_rules! impl_kagemusha_recursive_spend_release_activation {
    ($activation:ident, $key_id:ident, $owner_id:ident, $schema_hash:ident) => {
        impl $activation {
            /// Validate the release-bound Eq/Ep registry shape before consensus admission.
            ///
            /// # Errors
            ///
            /// Returns [`KagemushaReleaseVerificationError`] when release or verifier bindings differ.
            pub fn validate_structure(&self) -> Result<(), KagemushaReleaseVerificationError> {
                self.release_record.validate_structure()?;
                let manifest_sha256 = self
                    .release_record
                    .manifest
                    .canonical_sha256()
                    .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
                let expected_vesta_verifier_key_id =
                    $key_id(KagemushaPastaCycleParityV1::StepEq, manifest_sha256);
                let expected_pallas_verifier_key_id =
                    $key_id(KagemushaPastaCycleParityV1::StepEp, manifest_sha256);
                if self.configured_policy_sha256 == [0; 32]
                    || self.configured_policy_sha256
                        != self.release_record.promotion_record.release_policy_sha256
                    || self.step_eq_verifier_key_id != expected_vesta_verifier_key_id
                    || self.step_ep_verifier_key_id != expected_pallas_verifier_key_id
                    || !self.step_eq_verifier_key_id.is_portable_registry_id()
                    || !self.step_ep_verifier_key_id.is_portable_registry_id()
                    || self.step_eq_verifier_record.version == 0
                    || self.step_eq_verifier_record.version != self.step_ep_verifier_record.version
                {
                    return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
                }
                self.validate_verifier_record(
                    &self.step_eq_verifier_record,
                    KagemushaPastaCycleParityV1::StepEq,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EQ_VERIFIER_CURVE_V4,
                )?;
                self.validate_verifier_record(
                    &self.step_ep_verifier_record,
                    KagemushaPastaCycleParityV1::StepEp,
                    KAGEMUSHA_RECURSIVE_SPEND_STEP_EP_VERIFIER_CURVE_V4,
                )?;
                Ok(())
            }
            fn validate_verifier_record(
                &self,
                record: &VerifyingKeyRecord,
                parity: KagemushaPastaCycleParityV1,
                expected_curve: &str,
            ) -> Result<(), KagemushaReleaseVerificationError> {
                let manifest = &self.release_record.manifest;
                let profile = manifest
                    .profiles
                    .iter()
                    .find(|profile| profile.parity == parity)
                    .ok_or(KagemushaReleaseVerificationError::InvalidManifest)?;
                let descriptor = profile
                    .artifacts
                    .get(2)
                    .filter(|artifact| {
                        artifact.kind == KagemushaPastaCycleArtifactKindV4::VerifyingKey
                    })
                    .ok_or(KagemushaReleaseVerificationError::InvalidManifest)?;
                let key = record
                    .key
                    .as_ref()
                    .ok_or(KagemushaReleaseVerificationError::InvalidPromotionRecord)?;
                let key_len = u64::try_from(key.bytes.len())
                    .map_err(|_| KagemushaReleaseVerificationError::InvalidPromotionRecord)?;
                let manifest_sha256 = manifest
                    .canonical_sha256()
                    .map_err(|_| KagemushaReleaseVerificationError::InvalidManifest)?;
                let expected_owner = $owner_id(manifest_sha256);
                let expected_schema_hash = $schema_hash(manifest, parity)
                    .map_err(|_| KagemushaReleaseVerificationError::InvalidPromotionRecord)?;
                let expected_commitment = verifying_key_commitment_v1(key)?;
                if record.circuit_id != profile.circuit_id
                    || record.owner_manifest_id.as_deref() != Some(expected_owner.as_str())
                    || record.namespace != KAGEMUSHA_VERIFIER_NAMESPACE
                    || record.backend != BackendTag::Halo2IpaPasta
                    || record.curve != expected_curve
                    || record.public_inputs_schema_hash != expected_schema_hash
                    || record.commitment != expected_commitment
                    || u64::from(record.vk_len) != key_len
                    || record.max_proof_bytes != manifest.max_proof_bytes
                    || record.activation_height != Some(manifest.activation_height)
                    || record.withdraw_height.is_some()
                    || record.status != ConfidentialStatus::Active
                    || key.backend.as_str() != KAGEMUSHA_RECURSIVE_SPEND_PASTA_CYCLE_BACKEND_V4
                    || key_len != descriptor.payload_size_bytes
                    || <[u8; 32]>::from(Sha256::digest(&key.bytes)) != descriptor.payload_sha256
                {
                    return Err(KagemushaReleaseVerificationError::InvalidPromotionRecord);
                }
                Ok(())
            }
        }
    };
}
impl_kagemusha_recursive_spend_release_activation!(
    KagemushaRecursiveSpendReleaseActivationV4,
    kagemusha_recursive_spend_verifier_key_id_v4,
    kagemusha_recursive_spend_verifier_owner_manifest_id_v4,
    kagemusha_recursive_spend_verifier_public_inputs_schema_hash_v4
);
impl_kagemusha_recursive_spend_release_activation!(
    KagemushaRecursiveSpendReleaseActivationV5,
    kagemusha_recursive_spend_verifier_key_id_v5,
    kagemusha_recursive_spend_verifier_owner_manifest_id_v5,
    kagemusha_recursive_spend_verifier_public_inputs_schema_hash_v5
);
