/// Fully homomorphic encryption scheme family used by a parameter set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "scheme", content = "value"))]
pub enum FheSchemeV1 {
    /// Brakerski/Fan-Vercauteren integer arithmetic scheme.
    #[default]
    Bfv,
    /// Brakerski-Gentry-Vaikuntanathan integer arithmetic scheme.
    Bgv,
    /// Approximate arithmetic CKKS scheme.
    Ckks,
}
/// Governance lifecycle state for a registered FHE parameter set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "lifecycle", content = "value"))]
pub enum FheParamLifecycleV1 {
    /// Parameter set is published and awaiting activation.
    #[default]
    Proposed,
    /// Parameter set is active and may be used for job admission.
    Active,
    /// Parameter set is still valid but scheduled for migration/retirement.
    Deprecated,
    /// Parameter set is withdrawn and must be rejected for new jobs.
    Withdrawn,
}
/// Governance-managed FHE parameter-set descriptor for `Soracloud` workloads.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct FheParamSetV1 {
    /// Schema version; must equal [`FHE_PARAM_SET_VERSION_V1`].
    pub schema_version: u16,
    /// Stable on-chain identifier for the parameter family.
    pub param_set: Name,
    /// Monotonic version number under the same `param_set` name.
    pub version: NonZeroU32,
    /// Backend profile identifier.
    pub backend: String,
    /// Cryptosystem family used by this parameter set.
    pub scheme: FheSchemeV1,
    /// RNS modulus chain in bits, canonical order from highest to lowest level.
    #[norito(default)]
    pub ciphertext_modulus_bits: Vec<NonZeroU16>,
    /// Plaintext modulus size in bits.
    pub plaintext_modulus_bits: NonZeroU16,
    /// Polynomial modulus degree.
    pub polynomial_modulus_degree: NonZeroU32,
    /// Number of plaintext slots exposed by this profile.
    pub slot_count: NonZeroU32,
    /// Minimum targeted security level in bits.
    pub security_level_bits: NonZeroU16,
    /// Maximum admissible multiplication depth under this chain.
    pub max_multiplicative_depth: NonZeroU16,
    /// Governance lifecycle state.
    pub lifecycle: FheParamLifecycleV1,
    /// First block where this set can be admitted.
    #[norito(default)]
    pub activation_height: Option<u64>,
    /// Optional block where this set enters deprecation.
    #[norito(default)]
    pub deprecation_height: Option<u64>,
    /// Optional block where this set is fully withdrawn.
    #[norito(default)]
    pub withdraw_height: Option<u64>,
    /// Canonical digest of backend parameter bytes.
    pub parameter_digest: Hash,
    /// Domain-separated digest of the backend RNS coefficient-modulus chain.
    pub rns_modulus_chain_digest: Hash,
    /// Domain-separated digest of the backend key-switch decomposition RNS chain.
    pub key_switch_decomposition_chain_digest: Hash,
}
impl FheParamSetV1 {
    /// Validate schema version and deterministic lifecycle constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// parameter/lifecycle fields violate deterministic governance rules.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != FHE_PARAM_SET_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "fhe parameter set",
                expected: FHE_PARAM_SET_VERSION_V1,
                found: self.schema_version,
            });
        }
        if self.backend.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "fhe parameter set",
                field: "backend",
            });
        }
        if self.scheme != FheSchemeV1::Bfv {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe parameter set",
                field: "scheme",
                reason: "first-release FHE parameter sets currently support BFV only".to_string(),
            });
        }
        if self.backend != REGISTERED_SORACLOUD_BFV_BACKEND_V1 {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe parameter set",
                field: "backend",
                reason: format!(
                    "must match registered BFV backend `{REGISTERED_SORACLOUD_BFV_BACKEND_V1}`"
                ),
            });
        }
        validate_soracloud_fhe_digest_hash(
            "fhe parameter set",
            "parameter_digest",
            self.parameter_digest,
        )?;
        validate_soracloud_fhe_digest_hash(
            "fhe parameter set",
            "rns_modulus_chain_digest",
            self.rns_modulus_chain_digest,
        )?;
        validate_soracloud_fhe_digest_hash(
            "fhe parameter set",
            "key_switch_decomposition_chain_digest",
            self.key_switch_decomposition_chain_digest,
        )?;
        if self.ciphertext_modulus_bits.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "fhe parameter set",
                field: "ciphertext_modulus_bits",
            });
        }
        let mut previous_bits = u16::MAX;
        for modulus_bits in &self.ciphertext_modulus_bits {
            let current = modulus_bits.get();
            if !(2..=120).contains(&current) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "fhe parameter set",
                    field: "ciphertext_modulus_bits",
                    reason: format!("value {current} must be within 2..=120"),
                });
            }
            if current > previous_bits {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "fhe parameter set",
                    field: "ciphertext_modulus_bits",
                    reason: "chain must be non-increasing".to_string(),
                });
            }
            previous_bits = current;
        }
        let largest_modulus = self
            .ciphertext_modulus_bits
            .first()
            .expect("ciphertext modulus chain is non-empty due prior check")
            .get();
        if self.plaintext_modulus_bits.get() >= largest_modulus {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe parameter set",
                field: "plaintext_modulus_bits",
                reason: format!(
                    "must be smaller than the largest ciphertext modulus ({largest_modulus})"
                ),
            });
        }
        if self.slot_count > self.polynomial_modulus_degree {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe parameter set",
                field: "slot_count",
                reason: "cannot exceed polynomial_modulus_degree".to_string(),
            });
        }
        let chain_len = u16::try_from(self.ciphertext_modulus_bits.len()).map_err(|_| {
            SoracloudManifestError::InvalidField {
                manifest: "fhe parameter set",
                field: "ciphertext_modulus_bits",
                reason: "chain length exceeds supported u16 range".to_string(),
            }
        })?;
        if self.max_multiplicative_depth.get() >= chain_len {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe parameter set",
                field: "max_multiplicative_depth",
                reason: format!(
                    "must be smaller than ciphertext modulus chain length ({chain_len})"
                ),
            });
        }
        let evaluator_budget = BfvEvaluationBudget::exact_evaluator_v1();
        if self.max_multiplicative_depth.get() > evaluator_budget.max_multiplicative_depth {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe parameter set",
                field: "max_multiplicative_depth",
                reason: format!(
                    "cannot exceed exact BFV evaluator multiplicative-depth budget ({})",
                    evaluator_budget.max_multiplicative_depth
                ),
            });
        }
        if let Some(deprecation_height) = self.deprecation_height {
            let Some(activation_height) = self.activation_height else {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "fhe parameter set",
                    field: "deprecation_height",
                    reason: "requires activation_height".to_string(),
                });
            };
            if deprecation_height <= activation_height {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "fhe parameter set",
                    field: "deprecation_height",
                    reason: "must be strictly greater than activation_height".to_string(),
                });
            }
        }
        if let Some(withdraw_height) = self.withdraw_height {
            let Some(activation_height) = self.activation_height else {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "fhe parameter set",
                    field: "withdraw_height",
                    reason: "requires activation_height".to_string(),
                });
            };
            if withdraw_height <= activation_height {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "fhe parameter set",
                    field: "withdraw_height",
                    reason: "must be strictly greater than activation_height".to_string(),
                });
            }
        }
        if let (Some(deprecation_height), Some(withdraw_height)) =
            (self.deprecation_height, self.withdraw_height)
            && withdraw_height <= deprecation_height
        {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe parameter set",
                field: "withdraw_height",
                reason: "must be strictly greater than deprecation_height".to_string(),
            });
        }
        match self.lifecycle {
            FheParamLifecycleV1::Proposed => {
                if self.deprecation_height.is_some() || self.withdraw_height.is_some() {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe parameter set",
                        field: "lifecycle",
                        reason: "proposed sets cannot define deprecation/withdraw heights"
                            .to_string(),
                    });
                }
            }
            FheParamLifecycleV1::Active => {
                if self.activation_height.is_none() {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe parameter set",
                        field: "lifecycle",
                        reason: "active sets require activation_height".to_string(),
                    });
                }
            }
            FheParamLifecycleV1::Deprecated => {
                if self.activation_height.is_none() || self.deprecation_height.is_none() {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe parameter set",
                        field: "lifecycle",
                        reason: "deprecated sets require activation_height and deprecation_height"
                            .to_string(),
                    });
                }
            }
            FheParamLifecycleV1::Withdrawn => {
                if self.activation_height.is_none() || self.withdraw_height.is_none() {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe parameter set",
                        field: "lifecycle",
                        reason: "withdrawn sets require activation_height and withdraw_height"
                            .to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}
/// Rounding mode used for deterministic ciphertext arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "rounding_mode", content = "value"))]
pub enum FheDeterministicRoundingModeV1 {
    /// Always round toward negative infinity.
    Floor,
    /// Round to nearest value; ties resolve to even.
    #[default]
    NearestTiesToEven,
}
/// Public BFV refresh transcript derivation mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(
    feature = "json",
    norito(tag = "refresh_transcript_mode", content = "value")
)]
pub enum BfvRefreshTranscriptModeV1 {
    /// First-release exact-lift encrypted-zero refresh transcript derivation.
    #[default]
    ExactLift,
    /// Rounded bounded-noise encrypted-zero refresh transcript derivation.
    BoundedNoise,
}
/// Public BFV ciphertext bound semantics attached to FHE state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema, Default)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "bound_mode", content = "value"))]
pub enum BfvCiphertextBoundModeV1 {
    /// Bound is an exact plaintext-modulus residual multiple.
    #[default]
    ExactResidualMultiple,
    /// Bound is a rounded BFV centered-noise bound.
    BoundedNoise,
}
/// Public transcript seed for one BFV rotation refresh key.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct BfvRotationRefreshTranscriptV1 {
    /// Rotation step count whose public refresh key is derived from `seed`.
    pub rotation_steps: u32,
    /// Public deterministic seed for the encrypted-zero refresh key.
    pub seed: Vec<u8>,
}
/// Public transcript seed for the BFV bootstrap refresh key.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct BfvBootstrapRefreshTranscriptV1 {
    /// Bootstrap key id whose refresh rounds are derived from `seed`.
    pub key_id: String,
    /// Maximum refresh-round capacity bound into the transcript.
    pub max_refresh_rounds: u16,
    /// Public deterministic seed for the encrypted-zero refresh rounds.
    pub seed: Vec<u8>,
}
/// Public transcript inventory for BFV evaluation-key refresh material.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct BfvEvaluationKeyRefreshTranscriptV1 {
    /// Public BFV key used to derive rotation/bootstrap encrypted-zero masks.
    pub public_key: BfvPublicKey,
    /// One transcript seed per public rotation refresh key.
    #[norito(default)]
    pub rotation_transcripts: Vec<BfvRotationRefreshTranscriptV1>,
    /// Optional bootstrap refresh transcript.
    #[norito(default)]
    pub bootstrap_transcript: Option<BfvBootstrapRefreshTranscriptV1>,
}
fn validate_bfv_refresh_transcript_seed(
    manifest: &'static str,
    field: &'static str,
    seed: &[u8],
) -> Result<(), SoracloudManifestError> {
    if seed.is_empty() {
        return Err(SoracloudManifestError::EmptyField { manifest, field });
    }
    if seed.iter().all(|byte| *byte == 0) {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason: "must not be all zero".to_string(),
        });
    }
    if seed.len() > BFV_REFRESH_TRANSCRIPT_SEED_MAX_BYTES {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason: format!("cannot exceed {BFV_REFRESH_TRANSCRIPT_SEED_MAX_BYTES} bytes"),
        });
    }
    Ok(())
}
fn validate_bfv_refresh_transcript_bootstrap_key_id(
    key_id: &str,
) -> Result<(), SoracloudManifestError> {
    if key_id.is_empty() {
        return Err(SoracloudManifestError::EmptyField {
            manifest: "bfv evaluation-key refresh transcript",
            field: "bootstrap_transcript.key_id",
        });
    }
    if key_id.len() > BFV_REFRESH_TRANSCRIPT_BOOTSTRAP_KEY_ID_MAX_BYTES {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "bfv evaluation-key refresh transcript",
            field: "bootstrap_transcript.key_id",
            reason: format!(
                "cannot exceed {BFV_REFRESH_TRANSCRIPT_BOOTSTRAP_KEY_ID_MAX_BYTES} bytes"
            ),
        });
    }
    if key_id.trim() != key_id {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "bfv evaluation-key refresh transcript",
            field: "bootstrap_transcript.key_id",
            reason: "must be canonical without surrounding whitespace".to_string(),
        });
    }
    if !key_id
        .bytes()
        .all(is_bfv_refresh_transcript_bootstrap_key_id_byte)
    {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "bfv evaluation-key refresh transcript",
            field: "bootstrap_transcript.key_id",
            reason: "must contain only ASCII alphanumeric, '.', '_', or '-' bytes".to_string(),
        });
    }
    Ok(())
}
fn is_bfv_refresh_transcript_bootstrap_key_id_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-')
}
fn validate_bfv_refresh_transcript_bootstrap_rounds(
    max_refresh_rounds: u16,
) -> Result<(), SoracloudManifestError> {
    if max_refresh_rounds == 0 {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "bfv evaluation-key refresh transcript",
            field: "bootstrap_transcript.max_refresh_rounds",
            reason: "must be greater than zero".to_string(),
        });
    }
    if max_refresh_rounds > BFV_REFRESH_TRANSCRIPT_MAX_BOOTSTRAP_REFRESH_ROUNDS {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "bfv evaluation-key refresh transcript",
            field: "bootstrap_transcript.max_refresh_rounds",
            reason: format!(
                "cannot exceed BFV bootstrap-key refresh-round limit ({BFV_REFRESH_TRANSCRIPT_MAX_BOOTSTRAP_REFRESH_ROUNDS})"
            ),
        });
    }
    Ok(())
}
impl BfvEvaluationKeyRefreshTranscriptV1 {
    /// Validate bounded public transcript metadata before recomputing refresh keys.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the transcript inventory is
    /// unbounded or carries non-canonical public seed/key-id metadata.
    pub fn validate_seed_bounds(&self) -> Result<(), SoracloudManifestError> {
        if self.rotation_transcripts.len() > BFV_REFRESH_TRANSCRIPT_MAX_ROTATION_TRANSCRIPTS {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "bfv evaluation-key refresh transcript",
                field: "rotation_transcripts",
                reason: format!(
                    "cannot exceed {BFV_REFRESH_TRANSCRIPT_MAX_ROTATION_TRANSCRIPTS} entries"
                ),
            });
        }
        let mut seen_rotation_steps = BTreeSet::new();
        for transcript in &self.rotation_transcripts {
            if transcript.rotation_steps == 0 {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "bfv evaluation-key refresh transcript",
                    field: "rotation_transcripts.rotation_steps",
                    reason: "must be greater than zero".to_string(),
                });
            }
            if !seen_rotation_steps.insert(transcript.rotation_steps) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "bfv evaluation-key refresh transcript",
                    field: "rotation_transcripts.rotation_steps",
                    reason: format!(
                        "duplicate rotation transcript for {} steps",
                        transcript.rotation_steps
                    ),
                });
            }
            validate_bfv_refresh_transcript_seed(
                "bfv evaluation-key refresh transcript",
                "rotation_transcripts.seed",
                &transcript.seed,
            )?;
        }
        if let Some(transcript) = self.bootstrap_transcript.as_ref() {
            validate_bfv_refresh_transcript_bootstrap_key_id(&transcript.key_id)?;
            validate_bfv_refresh_transcript_bootstrap_rounds(transcript.max_refresh_rounds)?;
            validate_bfv_refresh_transcript_seed(
                "bfv evaluation-key refresh transcript",
                "bootstrap_transcript.seed",
                &transcript.seed,
            )?;
        }
        Ok(())
    }
    /// Derive the exact-lift public-key proof statement digest.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the public key is malformed or
    /// canonical statement digesting fails.
    pub fn public_key_proof_statement_digest(
        &self,
        params: &iroha_crypto::fhe_bfv::BfvParameters,
    ) -> Result<Hash, SoracloudManifestError> {
        self.public_key_proof_statement_digest_with_mode(
            params,
            BfvRefreshTranscriptModeV1::ExactLift,
        )
    }
    /// Derive the public-key proof statement digest for the selected BFV mode.
    ///
    /// The statement binds the transcript's public key to the BFV parameter set and public-key
    /// digest under the exact-lift or bounded-noise crypto domain. It is the public-input hash that
    /// future proof-carrying public key admission must verify before accepting key material.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the public key is malformed, parameter capacity is
    /// insufficient for the selected mode, or canonical statement digesting fails.
    pub fn public_key_proof_statement_digest_with_mode(
        &self,
        params: &iroha_crypto::fhe_bfv::BfvParameters,
        mode: BfvRefreshTranscriptModeV1,
    ) -> Result<Hash, SoracloudManifestError> {
        validate_bfv_public_key(params, &self.public_key).map_err(|err| {
            SoracloudManifestError::InvalidField {
                manifest: "bfv evaluation-key refresh transcript",
                field: "public_key",
                reason: err.to_string(),
            }
        })?;
        let digest = match mode {
            BfvRefreshTranscriptModeV1::ExactLift => {
                iroha_crypto::fhe_bfv::bfv_public_key_proof_statement_digest(
                    params,
                    &self.public_key,
                )
            }
            BfvRefreshTranscriptModeV1::BoundedNoise => {
                iroha_crypto::fhe_bfv::bfv_bounded_noise_public_key_proof_statement_digest(
                    params,
                    &self.public_key,
                )
            }
        };
        digest.map_err(|err| SoracloudManifestError::InvalidField {
            manifest: "bfv evaluation-key refresh transcript",
            field: "public_key_proof_statement_digest",
            reason: err.to_string(),
        })
    }
    /// Derive the exact-lift ciphertext proof statement digest for this public key.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the transcript public key,
    /// ciphertext, declared bound, or canonical statement digesting fails.
    pub fn ciphertext_proof_statement_digest(
        &self,
        params: &iroha_crypto::fhe_bfv::BfvParameters,
        ciphertext: &BfvCiphertext,
        declared_bound: u128,
    ) -> Result<Hash, SoracloudManifestError> {
        self.ciphertext_proof_statement_digest_with_mode(
            params,
            ciphertext,
            declared_bound,
            BfvRefreshTranscriptModeV1::ExactLift,
        )
    }
    /// Derive the ciphertext proof statement digest for the selected BFV mode.
    ///
    /// The statement binds the transcript's public key, public-key digest, ciphertext bytes,
    /// ciphertext digest, and declared residual/noise bound under exact-lift or bounded-noise
    /// crypto domains. This is the verifier-facing public-input hash for ciphertext admission; it
    /// does not replace the attached proof.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the public key is malformed,
    /// the ciphertext is malformed or all zero, the declared bound exceeds the
    /// selected mode's capacity, or canonical statement digesting fails.
    pub fn ciphertext_proof_statement_digest_with_mode(
        &self,
        params: &iroha_crypto::fhe_bfv::BfvParameters,
        ciphertext: &BfvCiphertext,
        declared_bound: u128,
        mode: BfvRefreshTranscriptModeV1,
    ) -> Result<Hash, SoracloudManifestError> {
        validate_bfv_public_key(params, &self.public_key).map_err(|err| {
            SoracloudManifestError::InvalidField {
                manifest: "bfv evaluation-key refresh transcript",
                field: "public_key",
                reason: err.to_string(),
            }
        })?;
        let digest = match mode {
            BfvRefreshTranscriptModeV1::ExactLift => {
                iroha_crypto::fhe_bfv::bfv_ciphertext_exact_residual_proof_statement_digest(
                    params,
                    &self.public_key,
                    ciphertext,
                    declared_bound,
                )
            }
            BfvRefreshTranscriptModeV1::BoundedNoise => {
                iroha_crypto::fhe_bfv::bfv_bounded_noise_ciphertext_proof_statement_digest(
                    params,
                    &self.public_key,
                    ciphertext,
                    declared_bound,
                )
            }
        };
        digest.map_err(|err| SoracloudManifestError::InvalidField {
            manifest: "bfv evaluation-key refresh transcript",
            field: "ciphertext_proof_statement_digest",
            reason: err.to_string(),
        })
    }
    /// Validate and digest this transcript inventory against evaluation keys.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the transcript inventory does
    /// not cover the public refresh material or canonical digesting fails.
    pub fn digest_for_evaluation_keys(
        &self,
        params: &iroha_crypto::fhe_bfv::BfvParameters,
        evaluation_keys: &BfvEvaluationKeyBundle,
    ) -> Result<Hash, SoracloudManifestError> {
        self.digest_for_evaluation_keys_with_mode(
            params,
            evaluation_keys,
            BfvRefreshTranscriptModeV1::ExactLift,
        )
    }
    /// Validate and digest this transcript inventory for the selected BFV mode.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the transcript inventory does
    /// not cover the public refresh material or canonical digesting fails.
    pub fn digest_for_evaluation_keys_with_mode(
        &self,
        params: &iroha_crypto::fhe_bfv::BfvParameters,
        evaluation_keys: &BfvEvaluationKeyBundle,
        mode: BfvRefreshTranscriptModeV1,
    ) -> Result<Hash, SoracloudManifestError> {
        self.validate_seed_bounds()?;
        validate_bfv_public_key(params, &self.public_key).map_err(|err| {
            SoracloudManifestError::InvalidField {
                manifest: "bfv evaluation-key refresh transcript",
                field: "public_key",
                reason: err.to_string(),
            }
        })?;
        let rotation_transcripts = self
            .rotation_transcripts
            .iter()
            .map(|transcript| BfvRotationKeyTranscriptSeed {
                rotation_steps: transcript.rotation_steps,
                seed: transcript.seed.as_slice(),
            })
            .collect::<Vec<_>>();
        let bootstrap_transcript =
            self.bootstrap_transcript
                .as_ref()
                .map(|transcript| BfvBootstrapKeyTranscriptSeed {
                    key_id: transcript.key_id.as_str(),
                    max_refresh_rounds: transcript.max_refresh_rounds,
                    seed: transcript.seed.as_slice(),
                });
        let digest = match mode {
            BfvRefreshTranscriptModeV1::ExactLift => evaluation_keys.refresh_transcript_digest(
                params,
                &self.public_key,
                &rotation_transcripts,
                bootstrap_transcript,
            ),
            BfvRefreshTranscriptModeV1::BoundedNoise => evaluation_keys
                .bounded_noise_refresh_transcript_digest(
                    params,
                    &self.public_key,
                    &rotation_transcripts,
                    bootstrap_transcript,
                ),
        };
        digest.map_err(|err| SoracloudManifestError::InvalidField {
            manifest: "bfv evaluation-key refresh transcript",
            field: "refresh_transcript",
            reason: err.to_string(),
        })
    }
    /// Derive the bootstrap-key zero-refresh proof statement digest, if present.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when transcript metadata, public-key
    /// shape, bootstrap-key material, or canonical digesting fails.
    pub fn bootstrap_key_zero_refresh_proof_statement_digest_for_evaluation_keys_with_mode(
        &self,
        params: &iroha_crypto::fhe_bfv::BfvParameters,
        evaluation_keys: &BfvEvaluationKeyBundle,
        mode: BfvRefreshTranscriptModeV1,
    ) -> Result<Option<Hash>, SoracloudManifestError> {
        self.validate_seed_bounds()?;
        validate_bfv_public_key(params, &self.public_key).map_err(|err| {
            SoracloudManifestError::InvalidField {
                manifest: "bfv evaluation-key refresh transcript",
                field: "public_key",
                reason: err.to_string(),
            }
        })?;
        let Some(bootstrap_key) = evaluation_keys.bootstrap_key.as_ref() else {
            return Ok(None);
        };
        let Some(transcript) = self.bootstrap_transcript.as_ref() else {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "bfv evaluation-key refresh transcript",
                field: "bootstrap_transcript",
                reason: "must be present when evaluation keys carry a bootstrap key".to_string(),
            });
        };
        if transcript.key_id != bootstrap_key.key_id
            || transcript.max_refresh_rounds != bootstrap_key.max_refresh_rounds
        {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "bfv evaluation-key refresh transcript",
                field: "bootstrap_transcript",
                reason: "must match evaluation-key bootstrap key metadata".to_string(),
            });
        }
        let rotation_transcripts = self
            .rotation_transcripts
            .iter()
            .map(|transcript| BfvRotationKeyTranscriptSeed {
                rotation_steps: transcript.rotation_steps,
                seed: transcript.seed.as_slice(),
            })
            .collect::<Vec<_>>();
        let bootstrap_transcript = Some(BfvBootstrapKeyTranscriptSeed {
            key_id: transcript.key_id.as_str(),
            max_refresh_rounds: transcript.max_refresh_rounds,
            seed: transcript.seed.as_slice(),
        });
        let digest = match mode {
            BfvRefreshTranscriptModeV1::ExactLift => evaluation_keys
                .bootstrap_key_zero_refresh_proof_statement_digest_for_transcript(
                    params,
                    &self.public_key,
                    &rotation_transcripts,
                    bootstrap_transcript,
                ),
            BfvRefreshTranscriptModeV1::BoundedNoise => evaluation_keys
                .bounded_noise_bootstrap_key_zero_refresh_proof_statement_digest_for_transcript(
                    params,
                    &self.public_key,
                    &rotation_transcripts,
                    bootstrap_transcript,
                ),
        };
        digest.map_err(|err| SoracloudManifestError::InvalidField {
            manifest: "bfv evaluation-key refresh transcript",
            field: "bootstrap_key_zero_refresh_proof_statement_digest",
            reason: err.to_string(),
        })
    }
}
/// Deterministic execution policy for validator-side ciphertext operations.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct FheExecutionPolicyV1 {
    /// Schema version; must equal [`FHE_EXECUTION_POLICY_VERSION_V1`].
    pub schema_version: u16,
    /// Stable policy identifier.
    pub policy_name: Name,
    /// Referenced parameter-set name.
    pub param_set: Name,
    /// Referenced parameter-set version.
    pub param_set_version: NonZeroU32,
    /// Domain-separated digest of the BFV evaluation-key bundle admitted for this policy.
    pub evaluation_key_digest: Hash,
    /// Domain-separated digest of the BFV refresh transcript inventory.
    pub evaluation_key_refresh_transcript_digest: Hash,
    /// BFV refresh transcript derivation mode bound by this policy.
    #[norito(default)]
    pub refresh_transcript_mode: BfvRefreshTranscriptModeV1,
    /// Governed proof statement digest for public BFV key material.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub public_key_proof_statement_digest: Option<Hash>,
    /// Governed proof statement digest for bootstrap-capable public zero-refresh material.
    #[norito(default)]
    pub bootstrap_key_zero_refresh_proof_statement_digest: Option<Hash>,
    /// Release audit package approved for governed full-bootstrap artifact execution.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub full_bootstrap_release_audit_package: Option<BfvFullBootstrapReleaseAuditPackageV1>,
    /// Caller-pinned digest of the approved full-bootstrap release audit package.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub full_bootstrap_release_audit_package_digest: Option<Hash>,
    /// Trusted reviewer identifier expected by the full-bootstrap release audit package.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub full_bootstrap_release_audit_trusted_reviewer_id: Option<String>,
    /// Trusted reviewer public key expected by the full-bootstrap release audit package.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub full_bootstrap_release_audit_trusted_reviewer_public_key: Option<PublicKey>,
    /// Maximum admitted ciphertext size in bytes.
    pub max_ciphertext_bytes: NonZeroU64,
    /// Maximum admitted plaintext input size in bytes.
    pub max_plaintext_bytes: NonZeroU64,
    /// Maximum ciphertext inputs per operation.
    pub max_input_ciphertexts: NonZeroU16,
    /// Maximum ciphertext outputs per operation.
    pub max_output_ciphertexts: NonZeroU16,
    /// Maximum multiplication depth requested by an admitted job.
    pub max_multiplication_depth: NonZeroU16,
    /// Maximum homomorphic rotations per job.
    pub max_rotation_count: NonZeroU32,
    /// Maximum bootstrap operations per job.
    pub max_bootstrap_count: u16,
    /// Canonical rounding mode used by evaluators.
    pub rounding_mode: FheDeterministicRoundingModeV1,
}
impl FheExecutionPolicyV1 {
    /// Validate schema version and deterministic policy constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// execution limits violate deterministic admission rules.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != FHE_EXECUTION_POLICY_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "fhe execution policy",
                expected: FHE_EXECUTION_POLICY_VERSION_V1,
                found: self.schema_version,
            });
        }
        validate_soracloud_fhe_digest_hash(
            "fhe execution policy",
            "evaluation_key_digest",
            self.evaluation_key_digest,
        )?;
        validate_soracloud_fhe_digest_hash(
            "fhe execution policy",
            "evaluation_key_refresh_transcript_digest",
            self.evaluation_key_refresh_transcript_digest,
        )?;
        if self.max_plaintext_bytes > self.max_ciphertext_bytes {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "max_plaintext_bytes",
                reason: "cannot exceed max_ciphertext_bytes".to_string(),
            });
        }
        if self.max_output_ciphertexts > self.max_input_ciphertexts {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "max_output_ciphertexts",
                reason: "cannot exceed max_input_ciphertexts".to_string(),
            });
        }
        if self.rounding_mode != FheDeterministicRoundingModeV1::NearestTiesToEven {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "rounding_mode",
                reason: "only nearest-ties-to-even rounding is supported for first-release BFV execution policies"
                    .to_string(),
            });
        }
        let evaluator_budget = BfvEvaluationBudget::exact_evaluator_v1();
        if self.max_multiplication_depth.get() > evaluator_budget.max_multiplicative_depth {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "max_multiplication_depth",
                reason: format!(
                    "cannot exceed exact BFV evaluator multiplicative-depth budget ({})",
                    evaluator_budget.max_multiplicative_depth
                ),
            });
        }
        if self.max_bootstrap_count > evaluator_budget.max_bootstrap_refresh_rounds {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "max_bootstrap_count",
                reason: format!(
                    "cannot exceed exact BFV evaluator bootstrap-refresh budget ({})",
                    evaluator_budget.max_bootstrap_refresh_rounds
                ),
            });
        }
        let has_zero_refresh_statement = self
            .bootstrap_key_zero_refresh_proof_statement_digest
            .is_some();
        let Some(public_key_statement_hash) = self.public_key_proof_statement_digest else {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "public_key_proof_statement_digest",
                reason:
                    "production execution policies must bind a public-key proof statement digest"
                        .to_string(),
            });
        };
        validate_soracloud_fhe_statement_hash(
            "fhe execution policy",
            "public_key_proof_statement_digest",
            public_key_statement_hash,
        )?;
        if let Some(statement_hash) = self.bootstrap_key_zero_refresh_proof_statement_digest {
            validate_soracloud_fhe_statement_hash(
                "fhe execution policy",
                "bootstrap_key_zero_refresh_proof_statement_digest",
                statement_hash,
            )?;
        }
        let release_audit_field_count = [
            self.full_bootstrap_release_audit_package.is_some(),
            self.full_bootstrap_release_audit_package_digest.is_some(),
            self.full_bootstrap_release_audit_trusted_reviewer_id
                .is_some(),
            self.full_bootstrap_release_audit_trusted_reviewer_public_key
                .is_some(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count();
        let has_full_bootstrap_material = release_audit_field_count == 4;
        if release_audit_field_count > 0 {
            if release_audit_field_count != 4 {
                let (field, reason) = if self.full_bootstrap_release_audit_package.is_none() {
                    (
                        "full_bootstrap_release_audit_package",
                        "requires release audit package",
                    )
                } else if self.full_bootstrap_release_audit_package_digest.is_none() {
                    (
                        "full_bootstrap_release_audit_package_digest",
                        "requires release audit package digest",
                    )
                } else if self
                    .full_bootstrap_release_audit_trusted_reviewer_id
                    .is_none()
                {
                    (
                        "full_bootstrap_release_audit_trusted_reviewer_id",
                        "requires trusted release audit reviewer id",
                    )
                } else {
                    (
                        "full_bootstrap_release_audit_trusted_reviewer_public_key",
                        "requires trusted release audit reviewer public key",
                    )
                };
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "fhe execution policy",
                    field,
                    reason: reason.to_string(),
                });
            }
            let package = self
                .full_bootstrap_release_audit_package
                .as_ref()
                .expect("field count confirms package is present");
            let expected_package_digest = self
                .full_bootstrap_release_audit_package_digest
                .expect("field count confirms digest is present");
            validate_soracloud_fhe_digest_hash(
                "fhe execution policy",
                "full_bootstrap_release_audit_package_digest",
                expected_package_digest,
            )?;
            validate_soracloud_no_full_bootstrap_placeholder_digest(
                "fhe execution policy",
                "full_bootstrap_release_audit_package_digest",
                expected_package_digest,
            )?;
            if expected_package_digest == package.record_digest {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "fhe execution policy",
                    field: "full_bootstrap_release_audit_package_digest",
                    reason: "must be distinct from the package record digest".to_string(),
                });
            }
            if expected_package_digest == package.manifest_digest {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "fhe execution policy",
                    field: "full_bootstrap_release_audit_package_digest",
                    reason: "must be distinct from the package manifest digest".to_string(),
                });
            }
            let actual_package_digest =
                iroha_crypto::fhe_bfv::bfv_full_bootstrap_release_audit_package_digest_v1(package)
                    .map_err(|err| SoracloudManifestError::InvalidField {
                        manifest: "fhe execution policy",
                        field: "full_bootstrap_release_audit_package",
                        reason: err.to_string(),
                    })?;
            if expected_package_digest != actual_package_digest {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "fhe execution policy",
                    field: "full_bootstrap_release_audit_package_digest",
                    reason: "does not match the embedded release audit package".to_string(),
                });
            }
            let reviewer_id = self
                .full_bootstrap_release_audit_trusted_reviewer_id
                .as_ref()
                .expect("field count confirms reviewer id is present");
            if reviewer_id.trim().is_empty() {
                return Err(SoracloudManifestError::EmptyField {
                    manifest: "fhe execution policy",
                    field: "full_bootstrap_release_audit_trusted_reviewer_id",
                });
            }
            validate_bfv_full_bootstrap_release_audit_trusted_reviewer_id_v1(reviewer_id).map_err(
                |err| SoracloudManifestError::InvalidField {
                    manifest: "fhe execution policy",
                    field: "full_bootstrap_release_audit_trusted_reviewer_id",
                    reason: err.to_string(),
                },
            )?;
            let reviewer_public_key = self
                .full_bootstrap_release_audit_trusted_reviewer_public_key
                .as_ref()
                .expect("field count confirms reviewer public key is present");
            validate_bfv_full_bootstrap_release_audit_trusted_reviewer_public_key_v1(
                reviewer_public_key,
            )
            .map_err(|err| SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "full_bootstrap_release_audit_trusted_reviewer_public_key",
                reason: err.to_string(),
            })?;
            iroha_crypto::fhe_bfv::validate_bfv_full_bootstrap_release_audit_package_trusted_reviewer_v1(
                package,
                reviewer_id,
                reviewer_public_key,
            )
            .map_err(|err| SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "full_bootstrap_release_audit_package",
                reason: err.to_string(),
            })?;
            iroha_crypto::fhe_bfv::validate_bfv_full_bootstrap_release_audit_package_external_review_markers_v1(
                package,
            )
            .map_err(|err| SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "full_bootstrap_release_audit_package",
                reason: err.to_string(),
            })?;
        }
        if has_zero_refresh_statement && has_full_bootstrap_material {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "full_bootstrap_release_audit_package",
                reason:
                    "bootstrap-capable policies must select exactly one governed bootstrap mode"
                        .to_string(),
            });
        }
        if self.max_bootstrap_count > 0
            && !has_zero_refresh_statement
            && !has_full_bootstrap_material
        {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "bootstrap_key_zero_refresh_proof_statement_digest",
                reason:
                    "bootstrap-capable policies must bind a bootstrap-key proof statement digest"
                        .to_string(),
            });
        }
        if self.max_bootstrap_count == 0 && has_zero_refresh_statement {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "bootstrap_key_zero_refresh_proof_statement_digest",
                reason: "policies without bootstrap budget must not bind bootstrap-key proof statement digest"
                    .to_string(),
            });
        }
        if self.max_bootstrap_count == 0 && has_full_bootstrap_material {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "full_bootstrap_release_audit_package",
                reason: "policies without bootstrap budget must not bind governed full-bootstrap material"
                    .to_string(),
            });
        }
        Ok(())
    }
    /// Validate this policy against an admitted FHE parameter set.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when parameter identifiers do not
    /// match, policy depth exceeds the parameter budget, or the parameter set
    /// lifecycle is not admissible for new job execution.
    pub fn validate_for_param_set(
        &self,
        param_set: &FheParamSetV1,
    ) -> Result<(), SoracloudManifestError> {
        self.validate()?;
        param_set.validate()?;
        if self.param_set != param_set.param_set {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "param_set",
                reason: format!(
                    "policy references `{}` but parameter set is `{}`",
                    self.param_set, param_set.param_set
                ),
            });
        }
        if self.param_set_version != param_set.version {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "param_set_version",
                reason: format!(
                    "policy references version {} but parameter set is version {}",
                    self.param_set_version, param_set.version
                ),
            });
        }
        if self.max_multiplication_depth > param_set.max_multiplicative_depth {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "max_multiplication_depth",
                reason: format!(
                    "cannot exceed parameter-set maximum ({})",
                    param_set.max_multiplicative_depth
                ),
            });
        }
        match param_set.lifecycle {
            FheParamLifecycleV1::Proposed => Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "param_set.lifecycle",
                reason: "parameter set is not active yet".to_string(),
            }),
            FheParamLifecycleV1::Active | FheParamLifecycleV1::Deprecated => Ok(()),
            FheParamLifecycleV1::Withdrawn => Err(SoracloudManifestError::InvalidField {
                manifest: "fhe execution policy",
                field: "param_set.lifecycle",
                reason: "parameter set is withdrawn".to_string(),
            }),
        }
    }
}
/// Governance admission bundle coupling an FHE parameter set and execution policy.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct FheGovernanceBundleV1 {
    /// Schema version; must equal [`FHE_GOVERNANCE_BUNDLE_VERSION_V1`].
    pub schema_version: u16,
    /// Governance-authored parameter set descriptor.
    pub param_set: FheParamSetV1,
    /// Deterministic execution policy bound to the parameter set.
    pub execution_policy: FheExecutionPolicyV1,
}
impl FheGovernanceBundleV1 {
    /// Validate deterministic admission constraints across FHE governance records.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// policy/parameter references are inconsistent.
    pub fn validate_for_admission(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != FHE_GOVERNANCE_BUNDLE_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "fhe governance bundle",
                expected: FHE_GOVERNANCE_BUNDLE_VERSION_V1,
                found: self.schema_version,
            });
        }
        self.execution_policy
            .validate_for_param_set(&self.param_set)?;
        if self
            .execution_policy
            .public_key_proof_statement_digest
            .is_none()
        {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe governance bundle",
                field: "public_key_proof_statement_digest",
                reason:
                    "production governance bundles must bind a public-key proof statement digest"
                        .to_string(),
            });
        }
        Ok(())
    }
}
/// Exact immutable reference to one governed Soracloud FHE policy version.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoracloudFhePolicyReferenceV1 {
    /// Schema version; must equal [`SORACLOUD_FHE_POLICY_REFERENCE_VERSION_V1`].
    pub schema_version: u16,
    /// Stable policy identifier within the service.
    pub policy_name: Name,
    /// Exact monotonic policy-material version.
    pub version: NonZeroU32,
    /// Digest of the exact governed material authorized for execution.
    pub material_digest: Hash,
}
impl SoracloudFhePolicyReferenceV1 {
    /// Validate the exact governed-material reference.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the schema or digest is invalid.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != SORACLOUD_FHE_POLICY_REFERENCE_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "soracloud fhe policy reference",
                expected: SORACLOUD_FHE_POLICY_REFERENCE_VERSION_V1,
                found: self.schema_version,
            });
        }
        validate_soracloud_fhe_digest_hash(
            "soracloud fhe policy reference",
            "material_digest",
            self.material_digest,
        )
    }
}
/// Exact service-and-policy scope carried by the FHE governance permission.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoracloudFheGovernancePermissionScopeV1 {
    /// Schema version; must equal [`SORACLOUD_FHE_GOVERNANCE_PERMISSION_SCOPE_VERSION_V1`].
    pub schema_version: u16,
    /// Service whose governed material may be changed.
    pub service_name: Name,
    /// Policy whose governed material may be changed.
    pub policy_name: Name,
}
impl SoracloudFheGovernancePermissionScopeV1 {
    /// Validate the permission scope version.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] for an unsupported schema version.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != SORACLOUD_FHE_GOVERNANCE_PERMISSION_SCOPE_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "soracloud fhe governance permission scope",
                expected: SORACLOUD_FHE_GOVERNANCE_PERMISSION_SCOPE_VERSION_V1,
                found: self.schema_version,
            });
        }
        Ok(())
    }
}
/// Immutable, governance-authenticated material for one Soracloud FHE policy version.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoracloudFheGovernedMaterialV1 {
    /// Schema version; must equal [`SORACLOUD_FHE_GOVERNED_MATERIAL_VERSION_V1`].
    pub schema_version: u16,
    /// Service to which this material is cryptographically scoped.
    pub service_name: Name,
    /// Stable policy identifier within the service.
    pub policy_name: Name,
    /// Monotonic material version for this service and policy.
    pub version: NonZeroU32,
    /// Governance-authored parameter set and execution policy.
    pub governance_bundle: FheGovernanceBundleV1,
    /// Exact public evaluation-key bundle admitted by governance.
    pub evaluation_keys: BfvEvaluationKeyBundle,
    /// Exact deterministic refresh transcript admitted by governance.
    pub evaluation_key_refresh_transcript: BfvEvaluationKeyRefreshTranscriptV1,
    /// Concrete governed artifacts required by full-bootstrap key material.
    pub full_bootstrap_circuit_artifacts: Option<BfvFullBootstrapCircuitArtifactBundleV1>,
    /// Canonical domain-separated digest of every preceding material field.
    pub material_digest: Hash,
}
impl SoracloudFheGovernedMaterialV1 {
    const DIGEST_DOMAIN: &'static [u8] = b"iroha.soracloud.fhe.governed_material.v1";
    /// Compute the canonical digest for this immutable governed material.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when canonical Norito encoding fails.
    pub fn computed_material_digest(&self) -> Result<Hash, SoracloudManifestError> {
        let bytes = norito::encode_canonical(&(
            self.schema_version,
            self.service_name.clone(),
            self.policy_name.clone(),
            self.version,
            self.governance_bundle.clone(),
            self.evaluation_keys.clone(),
            self.evaluation_key_refresh_transcript.clone(),
            self.full_bootstrap_circuit_artifacts.clone(),
        ))
        .map_err(|err| SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe governed material",
            field: "material_digest",
            reason: format!("canonical Norito encoding failed: {err}"),
        })?;
        Ok(Hash::new_from_chunks(&[Self::DIGEST_DOMAIN, &bytes]))
    }
    /// Return the exact immutable reference carried by execution instructions.
    #[must_use]
    pub fn policy_reference(&self) -> SoracloudFhePolicyReferenceV1 {
        SoracloudFhePolicyReferenceV1 {
            schema_version: SORACLOUD_FHE_POLICY_REFERENCE_VERSION_V1,
            policy_name: self.policy_name.clone(),
            version: self.version,
            material_digest: self.material_digest,
        }
    }
    /// Validate all governed material and its canonical digest.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when any policy, key, transcript,
    /// artifact, release-audit, or digest binding is inconsistent.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != SORACLOUD_FHE_GOVERNED_MATERIAL_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "soracloud fhe governed material",
                expected: SORACLOUD_FHE_GOVERNED_MATERIAL_VERSION_V1,
                found: self.schema_version,
            });
        }
        self.governance_bundle.validate_for_admission()?;
        let policy = &self.governance_bundle.execution_policy;
        if self.policy_name != policy.policy_name {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe governed material",
                field: "policy_name",
                reason: "must match governance_bundle.execution_policy.policy_name".to_string(),
            });
        }
        if self.governance_bundle.param_set.lifecycle != FheParamLifecycleV1::Active {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe governed material",
                field: "governance_bundle.param_set.lifecycle",
                reason: "new governed versions require an active parameter set".to_string(),
            });
        }
        let params = ram_lfe_bfv_parameters_v1();
        self.evaluation_keys.validate(&params).map_err(|err| {
            SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe governed material",
                field: "evaluation_keys",
                reason: err.to_string(),
            }
        })?;
        let evaluation_key_digest = self.evaluation_keys.digest(&params).map_err(|err| {
            SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe governed material",
                field: "evaluation_keys",
                reason: err.to_string(),
            }
        })?;
        if evaluation_key_digest != policy.evaluation_key_digest {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe governed material",
                field: "evaluation_keys",
                reason: "digest does not match the governed execution policy".to_string(),
            });
        }
        let refresh_digest = self
            .evaluation_key_refresh_transcript
            .digest_for_evaluation_keys_with_mode(
                &params,
                &self.evaluation_keys,
                policy.refresh_transcript_mode,
            )?;
        if refresh_digest != policy.evaluation_key_refresh_transcript_digest {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe governed material",
                field: "evaluation_key_refresh_transcript",
                reason: "digest does not match the governed execution policy".to_string(),
            });
        }
        let public_key_statement = self
            .evaluation_key_refresh_transcript
            .public_key_proof_statement_digest_with_mode(&params, policy.refresh_transcript_mode)?;
        if Some(public_key_statement) != policy.public_key_proof_statement_digest {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe governed material",
                field: "evaluation_key_refresh_transcript.public_key",
                reason: "proof statement digest does not match the governed execution policy"
                    .to_string(),
            });
        }
        match self.evaluation_keys.bootstrap_key.as_ref() {
            None => {
                if self.full_bootstrap_circuit_artifacts.is_some() {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "soracloud fhe governed material",
                        field: "full_bootstrap_circuit_artifacts",
                        reason: "requires a FullBootstrapV1 evaluation key".to_string(),
                    });
                }
            }
            Some(bootstrap_key)
                if bootstrap_key.mode
                    == iroha_crypto::fhe_bfv::BfvBootstrapKeyMode::RefreshOnlyV1 =>
            {
                if self.full_bootstrap_circuit_artifacts.is_some() {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "soracloud fhe governed material",
                        field: "full_bootstrap_circuit_artifacts",
                        reason: "refresh-only bootstrap keys must not carry circuit artifacts"
                            .to_string(),
                    });
                }
                let statement = self
                    .evaluation_key_refresh_transcript
                    .bootstrap_key_zero_refresh_proof_statement_digest_for_evaluation_keys_with_mode(
                        &params,
                        &self.evaluation_keys,
                        policy.refresh_transcript_mode,
                    )?;
                if statement != policy.bootstrap_key_zero_refresh_proof_statement_digest {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "soracloud fhe governed material",
                        field: "evaluation_key_refresh_transcript.bootstrap_transcript",
                        reason:
                            "proof statement digest does not match the governed execution policy"
                                .to_string(),
                    });
                }
            }
            Some(bootstrap_key) => {
                if policy
                    .bootstrap_key_zero_refresh_proof_statement_digest
                    .is_some()
                {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "soracloud fhe governed material",
                        field: "governance_bundle.execution_policy.bootstrap_key_zero_refresh_proof_statement_digest",
                        reason: "FullBootstrapV1 material must not use the refresh-only proof"
                            .to_string(),
                    });
                }
                let material = bootstrap_key
                    .full_bootstrap_material
                    .as_ref()
                    .ok_or_else(|| SoracloudManifestError::InvalidField {
                        manifest: "soracloud fhe governed material",
                        field: "evaluation_keys.bootstrap_key.full_bootstrap_material",
                        reason: "FullBootstrapV1 key must carry governed material".to_string(),
                    })?;
                let artifacts =
                    self.full_bootstrap_circuit_artifacts
                        .as_ref()
                        .ok_or_else(|| SoracloudManifestError::InvalidField {
                            manifest: "soracloud fhe governed material",
                            field: "full_bootstrap_circuit_artifacts",
                            reason: "FullBootstrapV1 key requires exact governed artifacts"
                                .to_string(),
                        })?;
                iroha_crypto::fhe_bfv::validate_bfv_full_bootstrap_circuit_artifact_bundle_v1(
                    &params, material, artifacts,
                )
                .map_err(|err| SoracloudManifestError::InvalidField {
                    manifest: "soracloud fhe governed material",
                    field: "full_bootstrap_circuit_artifacts",
                    reason: err.to_string(),
                })?;
                let package = policy.full_bootstrap_release_audit_package.as_ref().ok_or_else(|| {
                    SoracloudManifestError::InvalidField {
                        manifest: "soracloud fhe governed material",
                        field: "governance_bundle.execution_policy.full_bootstrap_release_audit_package",
                        reason: "FullBootstrapV1 material requires an approved release audit package"
                            .to_string(),
                    }
                })?;
                iroha_crypto::fhe_bfv::validate_bfv_full_bootstrap_release_audit_package_for_artifacts_trusted_reviewer_and_digest_v1(
                    &params,
                    material,
                    artifacts,
                    package,
                    policy.full_bootstrap_release_audit_package_digest.expect(
                        "policy validation requires a release audit package digest",
                    ),
                    policy
                        .full_bootstrap_release_audit_trusted_reviewer_id
                        .as_deref()
                        .expect("policy validation requires a release audit reviewer id"),
                    policy
                        .full_bootstrap_release_audit_trusted_reviewer_public_key
                        .as_ref()
                        .expect("policy validation requires a release audit reviewer key"),
                )
                .map_err(|err| SoracloudManifestError::InvalidField {
                    manifest: "soracloud fhe governed material",
                    field: "governance_bundle.execution_policy.full_bootstrap_release_audit_package",
                    reason: err.to_string(),
                })?;
            }
        }
        let computed_digest = self.computed_material_digest()?;
        if self.material_digest != computed_digest {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe governed material",
                field: "material_digest",
                reason: "does not match the canonical governed material".to_string(),
            });
        }
        Ok(())
    }
}
/// Lifecycle of one immutable governed Soracloud FHE policy version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "lifecycle", content = "value"))]
pub enum SoracloudFhePolicyVersionLifecycleV1 {
    /// Exact version currently authorized for execution.
    Active,
    /// Immutable historical version replaced by a monotonic rotation.
    Superseded,
    /// Immutable historical version explicitly revoked by governance.
    Revoked,
}
/// Lifecycle wrapper for one immutable governed material version.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoracloudFhePolicyVersionStateV1 {
    /// Immutable authenticated material.
    pub material: SoracloudFheGovernedMaterialV1,
    /// Canonical signed transaction hash that admitted this version.
    pub admitted_by_transaction_hash: Hash,
    /// Current lifecycle of this version.
    pub lifecycle: SoracloudFhePolicyVersionLifecycleV1,
    /// Governance transaction that superseded or revoked this version.
    pub deactivated_by_transaction_hash: Option<Hash>,
}
impl SoracloudFhePolicyVersionStateV1 {
    /// Validate immutable material and lifecycle metadata.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] for inconsistent lifecycle fields.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        self.material.validate()?;
        validate_soracloud_fhe_digest_hash(
            "soracloud fhe policy version state",
            "admitted_by_transaction_hash",
            self.admitted_by_transaction_hash,
        )?;
        match (self.lifecycle, self.deactivated_by_transaction_hash) {
            (SoracloudFhePolicyVersionLifecycleV1::Active, None) => Ok(()),
            (SoracloudFhePolicyVersionLifecycleV1::Active, Some(_)) => {
                Err(SoracloudManifestError::InvalidField {
                    manifest: "soracloud fhe policy version state",
                    field: "deactivated_by_transaction_hash",
                    reason: "active material must not carry a deactivation transaction".to_string(),
                })
            }
            (_, Some(hash)) => validate_soracloud_fhe_digest_hash(
                "soracloud fhe policy version state",
                "deactivated_by_transaction_hash",
                hash,
            ),
            (_, None) => Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe policy version state",
                field: "deactivated_by_transaction_hash",
                reason: "inactive material must identify its governance transition".to_string(),
            }),
        }
    }
}
/// Complete monotonic lifecycle history for one service-scoped FHE policy.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoracloudFhePolicyRecordV1 {
    /// Schema version; must equal [`SORACLOUD_FHE_POLICY_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Service that owns this policy history.
    pub service_name: Name,
    /// Stable policy identifier within the service.
    pub policy_name: Name,
    /// Exact active version, or `None` after permanent revocation.
    pub active_version: Option<NonZeroU32>,
    /// Immutable version history keyed by consecutive monotonic version.
    pub versions: BTreeMap<NonZeroU32, SoracloudFhePolicyVersionStateV1>,
}
impl SoracloudFhePolicyRecordV1 {
    /// Validate monotonic version history and active/revoked lifecycle invariants.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when keys, embedded identities,
    /// versions, or lifecycle states are inconsistent.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != SORACLOUD_FHE_POLICY_RECORD_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "soracloud fhe policy record",
                expected: SORACLOUD_FHE_POLICY_RECORD_VERSION_V1,
                found: self.schema_version,
            });
        }
        if self.versions.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "soracloud fhe policy record",
                field: "versions",
            });
        }
        let mut expected_version = 1_u32;
        let mut active_count = 0_u32;
        for (version, state) in &self.versions {
            if version.get() != expected_version {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "soracloud fhe policy record",
                    field: "versions",
                    reason: "versions must be consecutive and begin at one".to_string(),
                });
            }
            expected_version = expected_version.checked_add(1).ok_or_else(|| {
                SoracloudManifestError::InvalidField {
                    manifest: "soracloud fhe policy record",
                    field: "versions",
                    reason: "version sequence exceeds u32".to_string(),
                }
            })?;
            state.validate()?;
            if state.material.service_name != self.service_name
                || state.material.policy_name != self.policy_name
                || state.material.version != *version
            {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "soracloud fhe policy record",
                    field: "versions",
                    reason: "map key and embedded service, policy, and version must match"
                        .to_string(),
                });
            }
            if state.lifecycle == SoracloudFhePolicyVersionLifecycleV1::Active {
                active_count = active_count.saturating_add(1);
                if self.active_version != Some(*version) {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "soracloud fhe policy record",
                        field: "active_version",
                        reason: "must identify the sole active version".to_string(),
                    });
                }
            }
        }
        let latest_version = *self
            .versions
            .last_key_value()
            .expect("non-empty history established above")
            .0;
        if let Some(active_version) = self.active_version {
            if active_count != 1 || active_version != latest_version {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "soracloud fhe policy record",
                    field: "active_version",
                    reason: "the sole active version must be the latest version".to_string(),
                });
            }
            for (version, state) in &self.versions {
                if *version != active_version
                    && state.lifecycle != SoracloudFhePolicyVersionLifecycleV1::Superseded
                {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "soracloud fhe policy record",
                        field: "versions.lifecycle",
                        reason: "all older versions must be superseded".to_string(),
                    });
                }
            }
        } else {
            if active_count != 0
                || self.versions.get(&latest_version).is_none_or(|state| {
                    state.lifecycle != SoracloudFhePolicyVersionLifecycleV1::Revoked
                })
            {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "soracloud fhe policy record",
                    field: "active_version",
                    reason:
                        "revoked policies must have no active version and a revoked latest version"
                            .to_string(),
                });
            }
            for (version, state) in &self.versions {
                if *version != latest_version
                    && state.lifecycle != SoracloudFhePolicyVersionLifecycleV1::Superseded
                {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "soracloud fhe policy record",
                        field: "versions.lifecycle",
                        reason: "all pre-revocation versions must be superseded".to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}
/// Proof envelope admitting a client-provided BFV ciphertext as Soracloud FHE input.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoracloudFheInputAdmissionProofV1 {
    /// Schema version; must equal [`SORACLOUD_FHE_INPUT_ADMISSION_PROOF_VERSION_V1`].
    pub schema_version: u16,
    /// Public BFV key that the admitted ciphertext statement binds.
    #[norito(default)]
    pub public_key: Option<BfvPublicKey>,
    /// Per-slot ciphertext proof statement digests bound into `statement_hash`.
    #[norito(default)]
    pub ciphertext_proof_statement_digests: Vec<Hash>,
    /// Public BFV ciphertext bound value proven for the ciphertext.
    pub residual_multiple_bound: u128,
    /// Semantics of `residual_multiple_bound`.
    #[norito(default)]
    pub bound_mode: BfvCiphertextBoundModeV1,
    /// Canonical statement hash carried as the proof public input.
    pub statement_hash: Hash,
    /// Verifier-backed proof attachment for the input-admission statement.
    pub proof: ProofAttachment,
}
impl SoracloudFheInputAdmissionProofV1 {
    /// Validate proof-envelope structure before verifier execution.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the envelope version is unsupported
    /// or the nested proof attachment is malformed.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != SORACLOUD_FHE_INPUT_ADMISSION_PROOF_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "soracloud fhe input admission proof",
                expected: SORACLOUD_FHE_INPUT_ADMISSION_PROOF_VERSION_V1,
                found: self.schema_version,
            });
        }
        validate_soracloud_fhe_statement_hash(
            "soracloud fhe input admission proof",
            "statement_hash",
            self.statement_hash,
        )?;
        let public_key =
            self.public_key
                .as_ref()
                .ok_or_else(|| SoracloudManifestError::InvalidField {
                    manifest: "soracloud fhe input admission proof",
                    field: "public_key",
                    reason: "must be present for ciphertext statement binding".to_string(),
                })?;
        let params = ram_lfe_bfv_parameters_v1();
        validate_bfv_public_key(&params, public_key).map_err(|err| {
            SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe input admission proof",
                field: "public_key",
                reason: err.to_string(),
            }
        })?;
        bfv_public_key_digest(&params, public_key).map_err(|err| {
            SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe input admission proof",
                field: "public_key",
                reason: err.to_string(),
            }
        })?;
        if self.ciphertext_proof_statement_digests.is_empty() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe input admission proof",
                field: "ciphertext_proof_statement_digests",
                reason: "must include at least one ciphertext statement digest".to_string(),
            });
        }
        if self.ciphertext_proof_statement_digests.len() > RAM_LFE_BFV_IDENTIFIER_SLOT_COUNT {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe input admission proof",
                field: "ciphertext_proof_statement_digests",
                reason: format!(
                    "contains {} digests but the registered identifier profile allows at most {RAM_LFE_BFV_IDENTIFIER_SLOT_COUNT}",
                    self.ciphertext_proof_statement_digests.len()
                ),
            });
        }
        for digest in &self.ciphertext_proof_statement_digests {
            validate_soracloud_fhe_statement_hash(
                "soracloud fhe input admission proof",
                "ciphertext_proof_statement_digests",
                *digest,
            )?;
        }
        if self.proof.backend.as_str().trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "soracloud fhe input admission proof",
                field: "proof.backend",
            });
        }
        validate_soracloud_fhe_input_admission_backend(self.proof.backend.as_str())?;
        if self.proof.proof.backend != self.proof.backend {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe input admission proof",
                field: "proof.proof.backend",
                reason: "must match proof.backend".to_string(),
            });
        }
        if self.proof.proof.bytes.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "soracloud fhe input admission proof",
                field: "proof.proof.bytes",
            });
        }
        if self.proof.vk_ref.backend != self.proof.backend {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe input admission proof",
                field: "proof.vk_ref.backend",
                reason: "must match proof.backend".to_string(),
            });
        }
        if self.proof.vk_ref.name.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "soracloud fhe input admission proof",
                field: "proof.vk_ref.name",
            });
        }
        if self.proof.vk_ref.name != SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1 {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe input admission proof",
                field: "proof.vk_ref.name",
                reason: "must use the canonical v1 circuit id".to_string(),
            });
        }
        if let Some((field, reason)) = self.proof.structural_error() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe input admission proof",
                field: "proof",
                reason: format!("{field} {reason}"),
            });
        }
        let vk_commitment =
            self.proof
                .vk_commitment
                .ok_or_else(|| SoracloudManifestError::InvalidField {
                    manifest: "soracloud fhe input admission proof",
                    field: "proof.vk_commitment",
                    reason: "must be present and match verifier-key hash".to_string(),
                })?;
        if self.proof.envelope_hash.is_none() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe input admission proof",
                field: "proof.envelope_hash",
                reason: "must be present and match proof bytes".to_string(),
            });
        }
        validate_soracloud_fhe_input_admission_bound_capacity(
            self.residual_multiple_bound,
            self.bound_mode,
        )?;
        validate_soracloud_fhe_input_admission_open_verify_envelope(
            &self.proof.proof.bytes,
            vk_commitment,
            self.statement_hash,
        )?;
        Ok(())
    }
}
/// Proof envelope admitting public BFV key material.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoracloudFhePublicKeyProofV1 {
    /// Schema version; must equal [`SORACLOUD_FHE_PUBLIC_KEY_PROOF_VERSION_V1`].
    pub schema_version: u16,
    /// Canonical BFV public-key statement hash carried as proof public input.
    pub statement_hash: Hash,
    /// Verifier-backed proof attachment for the public-key statement.
    pub proof: ProofAttachment,
}
impl SoracloudFhePublicKeyProofV1 {
    /// Validate proof-envelope structure before verifier execution.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the envelope version is
    /// unsupported or the nested proof attachment is malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != SORACLOUD_FHE_PUBLIC_KEY_PROOF_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "soracloud fhe public-key proof",
                expected: SORACLOUD_FHE_PUBLIC_KEY_PROOF_VERSION_V1,
                found: self.schema_version,
            });
        }
        validate_soracloud_fhe_statement_hash(
            "soracloud fhe public-key proof",
            "statement_hash",
            self.statement_hash,
        )?;
        if self.proof.backend.as_str().trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "soracloud fhe public-key proof",
                field: "proof.backend",
            });
        }
        validate_soracloud_fhe_public_key_proof_backend(self.proof.backend.as_str())?;
        if self.proof.proof.backend != self.proof.backend {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe public-key proof",
                field: "proof.proof.backend",
                reason: "must match proof.backend".to_string(),
            });
        }
        if self.proof.proof.bytes.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "soracloud fhe public-key proof",
                field: "proof.proof.bytes",
            });
        }
        if self.proof.vk_ref.backend != self.proof.backend {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe public-key proof",
                field: "proof.vk_ref.backend",
                reason: "must match proof.backend".to_string(),
            });
        }
        if self.proof.vk_ref.name.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "soracloud fhe public-key proof",
                field: "proof.vk_ref.name",
            });
        }
        if self.proof.vk_ref.name != SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1 {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe public-key proof",
                field: "proof.vk_ref.name",
                reason: "must use the canonical v1 circuit id".to_string(),
            });
        }
        if let Some((field, reason)) = self.proof.structural_error() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe public-key proof",
                field: "proof",
                reason: format!("{field} {reason}"),
            });
        }
        let vk_commitment =
            self.proof
                .vk_commitment
                .ok_or_else(|| SoracloudManifestError::InvalidField {
                    manifest: "soracloud fhe public-key proof",
                    field: "proof.vk_commitment",
                    reason: "must be present and match verifier-key hash".to_string(),
                })?;
        if self.proof.envelope_hash.is_none() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe public-key proof",
                field: "proof.envelope_hash",
                reason: "must be present and match proof bytes".to_string(),
            });
        }
        validate_soracloud_fhe_public_key_proof_open_verify_envelope(
            &self.proof.proof.bytes,
            vk_commitment,
            self.statement_hash,
        )?;
        Ok(())
    }
}
/// Proof envelope admitting public BFV bootstrap-key zero-refresh material.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoracloudFheBootstrapKeyProofV1 {
    /// Schema version; must equal [`SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_VERSION_V1`].
    pub schema_version: u16,
    /// Canonical bootstrap-key zero-refresh statement hash carried as proof public input.
    pub statement_hash: Hash,
    /// Verifier-backed proof attachment for the bootstrap-key statement.
    pub proof: ProofAttachment,
}
impl SoracloudFheBootstrapKeyProofV1 {
    /// Validate proof-envelope structure before verifier execution.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the envelope version is unsupported
    /// or the nested proof attachment is malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "soracloud fhe bootstrap key proof",
                expected: SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_VERSION_V1,
                found: self.schema_version,
            });
        }
        validate_soracloud_fhe_statement_hash(
            "soracloud fhe bootstrap key proof",
            "statement_hash",
            self.statement_hash,
        )?;
        if self.proof.backend.as_str().trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "soracloud fhe bootstrap key proof",
                field: "proof.backend",
            });
        }
        validate_soracloud_fhe_bootstrap_key_proof_backend(self.proof.backend.as_str())?;
        if self.proof.proof.backend != self.proof.backend {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe bootstrap key proof",
                field: "proof.proof.backend",
                reason: "must match proof.backend".to_string(),
            });
        }
        if self.proof.proof.bytes.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "soracloud fhe bootstrap key proof",
                field: "proof.proof.bytes",
            });
        }
        if self.proof.vk_ref.backend != self.proof.backend {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe bootstrap key proof",
                field: "proof.vk_ref.backend",
                reason: "must match proof.backend".to_string(),
            });
        }
        if self.proof.vk_ref.name.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "soracloud fhe bootstrap key proof",
                field: "proof.vk_ref.name",
            });
        }
        if self.proof.vk_ref.name != SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1 {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe bootstrap key proof",
                field: "proof.vk_ref.name",
                reason: "must use the canonical v1 circuit id".to_string(),
            });
        }
        if let Some((field, reason)) = self.proof.structural_error() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe bootstrap key proof",
                field: "proof",
                reason: format!("{field} {reason}"),
            });
        }
        let vk_commitment =
            self.proof
                .vk_commitment
                .ok_or_else(|| SoracloudManifestError::InvalidField {
                    manifest: "soracloud fhe bootstrap key proof",
                    field: "proof.vk_commitment",
                    reason: "must be present and match verifier-key hash".to_string(),
                })?;
        if self.proof.envelope_hash.is_none() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe bootstrap key proof",
                field: "proof.envelope_hash",
                reason: "must be present and match proof bytes".to_string(),
            });
        }
        validate_soracloud_fhe_bootstrap_key_proof_open_verify_envelope(
            &self.proof.proof.bytes,
            vk_commitment,
            self.statement_hash,
        )?;
        Ok(())
    }
}
/// Proof envelope admitting a governed BFV full-bootstrap execution output claim.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoracloudFheFullBootstrapExecutionProofV1 {
    /// Schema version; must equal [`SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_VERSION_V1`].
    pub schema_version: u16,
    /// Canonical full-bootstrap execution statement hash carried as proof public input.
    pub statement_hash: Hash,
    /// Verifier-backed proof attachment for the full-bootstrap execution statement.
    pub proof: ProofAttachment,
}
impl SoracloudFheFullBootstrapExecutionProofV1 {
    /// Validate proof-envelope structure before verifier execution.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the envelope version is unsupported
    /// or the nested proof attachment is malformed.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "soracloud fhe full-bootstrap execution proof",
                expected: SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_VERSION_V1,
                found: self.schema_version,
            });
        }
        validate_soracloud_fhe_statement_hash(
            "soracloud fhe full-bootstrap execution proof",
            "statement_hash",
            self.statement_hash,
        )?;
        if self.proof.backend.as_str().trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "soracloud fhe full-bootstrap execution proof",
                field: "proof.backend",
            });
        }
        validate_soracloud_fhe_full_bootstrap_execution_proof_backend(self.proof.backend.as_str())?;
        if self.proof.proof.backend != self.proof.backend {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe full-bootstrap execution proof",
                field: "proof.proof.backend",
                reason: "must match proof.backend".to_string(),
            });
        }
        if self.proof.proof.bytes.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "soracloud fhe full-bootstrap execution proof",
                field: "proof.proof.bytes",
            });
        }
        if self.proof.vk_ref.backend != self.proof.backend {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe full-bootstrap execution proof",
                field: "proof.vk_ref.backend",
                reason: "must match proof.backend".to_string(),
            });
        }
        if self.proof.vk_ref.name.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "soracloud fhe full-bootstrap execution proof",
                field: "proof.vk_ref.name",
            });
        }
        if self.proof.vk_ref.name != SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1 {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe full-bootstrap execution proof",
                field: "proof.vk_ref.name",
                reason: "must use the canonical v1 circuit id".to_string(),
            });
        }
        if let Some((field, reason)) = self.proof.structural_error() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe full-bootstrap execution proof",
                field: "proof",
                reason: format!("{field} {reason}"),
            });
        }
        let vk_commitment =
            self.proof
                .vk_commitment
                .ok_or_else(|| SoracloudManifestError::InvalidField {
                    manifest: "soracloud fhe full-bootstrap execution proof",
                    field: "proof.vk_commitment",
                    reason: "must be present and match verifier-key hash".to_string(),
                })?;
        if self.proof.envelope_hash.is_none() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "soracloud fhe full-bootstrap execution proof",
                field: "proof.envelope_hash",
                reason: "must be present and match proof bytes".to_string(),
            });
        }
        validate_soracloud_fhe_full_bootstrap_execution_proof_open_verify_envelope(
            &self.proof.proof.bytes,
            vk_commitment,
            self.statement_hash,
        )?;
        Ok(())
    }
}
fn validate_soracloud_fhe_input_admission_backend(
    backend: &str,
) -> Result<(), SoracloudManifestError> {
    if backend == BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1 {
        return Ok(());
    }
    Err(SoracloudManifestError::InvalidField {
        manifest: "soracloud fhe input admission proof",
        field: "proof.backend",
        reason: "must use the canonical BFV STARK/FRI backend".to_string(),
    })
}
fn validate_soracloud_fhe_public_key_proof_backend(
    backend: &str,
) -> Result<(), SoracloudManifestError> {
    if backend == BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1 {
        return Ok(());
    }
    Err(SoracloudManifestError::InvalidField {
        manifest: "soracloud fhe public-key proof",
        field: "proof.backend",
        reason: "must use the canonical BFV STARK/FRI backend".to_string(),
    })
}
fn validate_soracloud_fhe_bootstrap_key_proof_backend(
    backend: &str,
) -> Result<(), SoracloudManifestError> {
    if backend == BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1 {
        return Ok(());
    }
    Err(SoracloudManifestError::InvalidField {
        manifest: "soracloud fhe bootstrap key proof",
        field: "proof.backend",
        reason: "must use the canonical BFV STARK/FRI backend".to_string(),
    })
}
fn validate_soracloud_fhe_full_bootstrap_execution_proof_backend(
    backend: &str,
) -> Result<(), SoracloudManifestError> {
    if backend == BFV_FULL_BOOTSTRAP_PROOF_BACKEND_V1 {
        return Ok(());
    }
    Err(SoracloudManifestError::InvalidField {
        manifest: "soracloud fhe full-bootstrap execution proof",
        field: "proof.backend",
        reason: "must use the canonical BFV full-bootstrap STARK/FRI backend".to_string(),
    })
}
fn validate_soracloud_fhe_statement_hash(
    manifest: &'static str,
    field: &'static str,
    statement_hash: Hash,
) -> Result<(), SoracloudManifestError> {
    validate_soracloud_fhe_digest_hash(manifest, field, statement_hash)
}
fn validate_soracloud_fhe_digest_hash(
    manifest: &'static str,
    field: &'static str,
    digest_hash: Hash,
) -> Result<(), SoracloudManifestError> {
    validate_soracloud_digest_hash(manifest, field, digest_hash)
}
const SORACLOUD_FULL_BOOTSTRAP_PLACEHOLDER_DIGEST_PREIMAGES: &[&[u8]] = &[
    b"placeholder BFV full-bootstrap material",
    b"pending BFV full-bootstrap material",
    b"placeholder BFV full-bootstrap prover key",
    b"placeholder BFV full-bootstrap verifier key",
    b"placeholder BFV full-bootstrap native prover payload",
    b"placeholder BFV full-bootstrap native verifier payload",
    b"placeholder BFV full-bootstrap native proof key payload",
    b"pending BFV full-bootstrap native proof key payload",
    b"TODO pending BFV full-bootstrap native proof key payload",
    b"replace-before-production",
    b"replace before production",
    b"replace_before_production",
    b"not for production",
    b"not-for-production",
    b"not_for_production",
    b"not production ready",
    b"not-production-ready",
    b"not_production_ready",
    b"draft",
    b"draft-only",
    b"draft only",
    b"draft_only",
    b"todo",
    b"dummy",
    b"fake",
    b"mock",
    b"fixture",
    b"sample",
    b"template",
    b"example",
];
const SORACLOUD_FULL_BOOTSTRAP_PLACEHOLDER_DIGEST_DELAY_PREFIXES: &[&[u8]] = &[
    b"full-bootstrap material before placeholder: ",
    b"governed material digest before placeholder: ",
];
const SORACLOUD_FULL_BOOTSTRAP_PLACEHOLDER_DIGEST_LEADING_PREFIXES: &[&[u8]] =
    &[b" ", b"\n", b"\r\n", b"\t", b" \n\t"];
const SORACLOUD_FULL_BOOTSTRAP_SEPARATOR_SPELLED_PLACEHOLDER_TOKENS: &[&[u8]] = &[
    b"placeholder",
    b"pending",
    b"todo",
    b"fake",
    b"stub",
    b"mock",
    b"draft",
    b"dummy",
    b"fixture",
    b"sample",
    b"template",
    b"example",
];
const SORACLOUD_FULL_BOOTSTRAP_SEPARATOR_BYTES: &[u8] = b"-._";
const SORACLOUD_FULL_BOOTSTRAP_NATIVE_PROOF_KEY_SUFFIX: &[u8] =
    b" BFV full-bootstrap native proof key payload";
const SORACLOUD_FULL_BOOTSTRAP_PENDING_NATIVE_PROOF_KEY_SUFFIX: &[u8] =
    b" pending BFV full-bootstrap native proof key payload";
const SORACLOUD_STARK_NATIVE_ENVELOPE_PLACEHOLDER_MARKERS: &[&[u8]] = &[
    b"placeholder",
    b"not production ready",
    b"not-production-ready",
    b"not for production",
    b"not-for-production",
    b"replace before production",
    b"replace-before-production",
    b"replace_before_production",
    b"replace-me",
    b"replace me",
    b"replace_me",
    b"changeme",
    b"change-me",
    b"change me",
    b"change_me",
    b"test-only",
    b"test only",
    b"test_only",
    b"your-",
    b"your_",
    b"your-audit",
    b"your audit",
    b"your_audit",
    b"your-proof",
    b"your proof",
    b"your_proof",
    b"todo pending",
    b"todo",
    b"pending native stark",
    b"pending stark",
    b"not_for_production",
    b"not_production_ready",
    b"draft",
    b"draft-only",
    b"draft only",
    b"draft_only",
    b"dummy",
    b"fake",
    b"stub",
    b"mock",
    b"fixture",
    b"sample",
    b"template",
    b"example",
];
const SORACLOUD_COLLAPSED_PLACEHOLDER_MARKER_MIN_BYTES: usize = 5;
static SORACLOUD_COLLAPSED_PLACEHOLDER_MARKERS: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
fn validate_soracloud_no_full_bootstrap_placeholder_digest(
    manifest: &'static str,
    field: &'static str,
    digest_hash: Hash,
) -> Result<(), SoracloudManifestError> {
    for preimage in SORACLOUD_FULL_BOOTSTRAP_PLACEHOLDER_DIGEST_PREIMAGES {
        if soracloud_full_bootstrap_placeholder_digest_matches_preimage(&digest_hash, preimage) {
            return Err(SoracloudManifestError::InvalidField {
                manifest,
                field,
                reason: "must not be a placeholder full-bootstrap digest".to_string(),
            });
        }
    }
    for token in SORACLOUD_FULL_BOOTSTRAP_SEPARATOR_SPELLED_PLACEHOLDER_TOKENS {
        for separator in SORACLOUD_FULL_BOOTSTRAP_SEPARATOR_BYTES {
            let body = soracloud_separator_spell_ascii_token(token, *separator);
            if soracloud_full_bootstrap_placeholder_digest_matches_preimage(&digest_hash, &body) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest,
                    field,
                    reason: "must not be a placeholder full-bootstrap digest".to_string(),
                });
            }
            let mut suffixed_body = body;
            suffixed_body.extend_from_slice(SORACLOUD_FULL_BOOTSTRAP_NATIVE_PROOF_KEY_SUFFIX);
            if soracloud_full_bootstrap_placeholder_digest_matches_preimage(
                &digest_hash,
                &suffixed_body,
            ) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest,
                    field,
                    reason: "must not be a placeholder full-bootstrap digest".to_string(),
                });
            }
            if *token == b"todo" {
                let mut pending_body = soracloud_separator_spell_ascii_token(token, *separator);
                pending_body
                    .extend_from_slice(SORACLOUD_FULL_BOOTSTRAP_PENDING_NATIVE_PROOF_KEY_SUFFIX);
                if soracloud_full_bootstrap_placeholder_digest_matches_preimage(
                    &digest_hash,
                    &pending_body,
                ) {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest,
                        field,
                        reason: "must not be a placeholder full-bootstrap digest".to_string(),
                    });
                }
            }
        }
    }
    Ok(())
}
fn soracloud_full_bootstrap_placeholder_digest_matches_preimage(
    digest_hash: &Hash,
    preimage: &[u8],
) -> bool {
    if digest_hash == &Hash::new(preimage) {
        return true;
    }
    let mut binary_framed_preimage = Vec::with_capacity(1 + preimage.len());
    binary_framed_preimage.push(0xff);
    binary_framed_preimage.extend_from_slice(preimage);
    if digest_hash == &Hash::new(&binary_framed_preimage) {
        return true;
    }
    for prefix in SORACLOUD_FULL_BOOTSTRAP_PLACEHOLDER_DIGEST_DELAY_PREFIXES {
        let mut delayed_preimage = Vec::with_capacity(prefix.len() + preimage.len());
        delayed_preimage.extend_from_slice(prefix);
        delayed_preimage.extend_from_slice(preimage);
        if digest_hash == &Hash::new(&delayed_preimage) {
            return true;
        }
        let mut binary_framed_delayed_preimage =
            Vec::with_capacity(prefix.len() + 1 + preimage.len());
        binary_framed_delayed_preimage.extend_from_slice(prefix);
        binary_framed_delayed_preimage.push(0xff);
        binary_framed_delayed_preimage.extend_from_slice(preimage);
        if digest_hash == &Hash::new(&binary_framed_delayed_preimage) {
            return true;
        }
        for leading_prefix in SORACLOUD_FULL_BOOTSTRAP_PLACEHOLDER_DIGEST_LEADING_PREFIXES {
            let mut leading_delayed_preimage =
                Vec::with_capacity(leading_prefix.len() + prefix.len() + preimage.len());
            leading_delayed_preimage.extend_from_slice(leading_prefix);
            leading_delayed_preimage.extend_from_slice(prefix);
            leading_delayed_preimage.extend_from_slice(preimage);
            if digest_hash == &Hash::new(&leading_delayed_preimage) {
                return true;
            }
            let mut binary_framed_leading_delayed_preimage =
                Vec::with_capacity(leading_prefix.len() + prefix.len() + 1 + preimage.len());
            binary_framed_leading_delayed_preimage.extend_from_slice(leading_prefix);
            binary_framed_leading_delayed_preimage.extend_from_slice(prefix);
            binary_framed_leading_delayed_preimage.push(0xff);
            binary_framed_leading_delayed_preimage.extend_from_slice(preimage);
            if digest_hash == &Hash::new(&binary_framed_leading_delayed_preimage) {
                return true;
            }
        }
    }
    false
}
fn soracloud_separator_spell_ascii_token(token: &[u8], separator: u8) -> Vec<u8> {
    let mut spelled = Vec::with_capacity(token.len().saturating_mul(2).saturating_sub(1));
    for (index, byte) in token.iter().copied().enumerate() {
        if index > 0 {
            spelled.push(separator);
        }
        spelled.push(byte);
    }
    spelled
}
fn validate_soracloud_digest_hash(
    manifest: &'static str,
    field: &'static str,
    digest_hash: Hash,
) -> Result<(), SoracloudManifestError> {
    let mut zero_prehash_sentinel = [0u8; Hash::LENGTH];
    zero_prehash_sentinel[Hash::LENGTH - 1] = 1;
    if <[u8; Hash::LENGTH]>::from(digest_hash) == zero_prehash_sentinel {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field,
            reason: "must not be the zero prehash sentinel".to_string(),
        });
    }
    Ok(())
}
fn validate_soracloud_fhe_input_admission_bound_capacity(
    residual_multiple_bound: u128,
    bound_mode: BfvCiphertextBoundModeV1,
) -> Result<(), SoracloudManifestError> {
    validate_soracloud_bfv_ciphertext_bound_capacity(
        residual_multiple_bound,
        bound_mode,
        "soracloud fhe input admission proof",
        "residual_multiple_bound",
        "soracloud fhe input admission bound",
    )
}
fn validate_soracloud_bfv_ciphertext_bound_capacity(
    residual_multiple_bound: u128,
    bound_mode: BfvCiphertextBoundModeV1,
    manifest: &'static str,
    field: &'static str,
    label: &str,
) -> Result<(), SoracloudManifestError> {
    let params = ram_lfe_bfv_parameters_v1();
    let result = match bound_mode {
        BfvCiphertextBoundModeV1::ExactResidualMultiple => {
            validate_bfv_exact_residual_multiple_capacity(
                &params,
                residual_multiple_bound,
                &format!("{label} exact residual"),
            )
        }
        BfvCiphertextBoundModeV1::BoundedNoise => validate_bfv_bounded_noise_bound(
            &params,
            residual_multiple_bound,
            &format!("{label} bounded-noise"),
        ),
    };
    result.map_err(|err| SoracloudManifestError::InvalidField {
        manifest,
        field,
        reason: format!("exceeds registered BFV capacity: {err}"),
    })
}
fn validate_soracloud_fhe_stark_native_envelope_bytes(
    manifest: &'static str,
    envelope_bytes: &[u8],
    max_bytes: usize,
) -> Result<(), SoracloudManifestError> {
    if envelope_bytes.is_empty() {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field: "proof.proof.bytes",
            reason: "STARK native envelope bytes must be non-empty".to_string(),
        });
    }
    if envelope_bytes.iter().all(|byte| *byte == 0) {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field: "proof.proof.bytes",
            reason: "STARK native envelope bytes must not be all-zero".to_string(),
        });
    }
    if envelope_bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field: "proof.proof.bytes",
            reason: "STARK native envelope bytes must not be blank".to_string(),
        });
    }
    if envelope_bytes.len() > max_bytes {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field: "proof.proof.bytes",
            reason: format!(
                "STARK native envelope bytes length {} exceeds maximum {}",
                envelope_bytes.len(),
                max_bytes
            ),
        });
    }
    if soracloud_fhe_stark_native_envelope_bytes_are_placeholder_text(envelope_bytes) {
        return Err(SoracloudManifestError::InvalidField {
            manifest,
            field: "proof.proof.bytes",
            reason: "STARK native envelope bytes must not be placeholder or non-production text"
                .to_string(),
        });
    }
    Ok(())
}
fn soracloud_fhe_stark_native_envelope_bytes_are_placeholder_text(envelope_bytes: &[u8]) -> bool {
    let is_text_byte = |byte: &u8| byte.is_ascii_graphic() || byte.is_ascii_whitespace();
    if envelope_bytes.iter().all(is_text_byte) {
        return soracloud_fhe_stark_native_envelope_text_span_is_placeholder(envelope_bytes);
    }
    false
}
fn soracloud_fhe_stark_native_envelope_text_span_is_placeholder(text: &[u8]) -> bool {
    let mut lower = Vec::with_capacity(text.len());
    lower.extend(text.iter().map(u8::to_ascii_lowercase));
    soracloud_ascii_text_contains_placeholder_marker(
        &lower,
        SORACLOUD_STARK_NATIVE_ENVELOPE_PLACEHOLDER_MARKERS,
    )
}
fn soracloud_ascii_text_contains_placeholder_marker(normalized: &[u8], markers: &[&[u8]]) -> bool {
    if markers
        .iter()
        .any(|marker| ascii_windows_contains(normalized, marker))
    {
        return true;
    }
    let collapsed_text = ascii_alnum_collapsed(normalized);
    if collapsed_text.is_empty() {
        return false;
    }
    soracloud_collapsed_placeholder_markers(markers)
        .iter()
        .any(|marker| ascii_windows_contains(&collapsed_text, marker))
}
fn soracloud_collapsed_placeholder_markers(markers: &[&[u8]]) -> &'static [Vec<u8>] {
    debug_assert!(std::ptr::eq(
        markers,
        SORACLOUD_STARK_NATIVE_ENVELOPE_PLACEHOLDER_MARKERS,
    ));
    SORACLOUD_COLLAPSED_PLACEHOLDER_MARKERS
        .get_or_init(|| {
            markers
                .iter()
                .filter_map(|marker| {
                    let collapsed_marker = ascii_alnum_collapsed(marker);
                    (collapsed_marker.len() >= SORACLOUD_COLLAPSED_PLACEHOLDER_MARKER_MIN_BYTES)
                        .then_some(collapsed_marker)
                })
                .collect()
        })
        .as_slice()
}
fn ascii_alnum_collapsed(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .copied()
        .filter(u8::is_ascii_alphanumeric)
        .collect()
}
fn ascii_windows_contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}
fn validate_soracloud_fhe_input_admission_open_verify_envelope(
    proof_bytes: &[u8],
    vk_commitment: [u8; 32],
    statement_hash: Hash,
) -> Result<(), SoracloudManifestError> {
    if proof_bytes.len() > SORACLOUD_FHE_INPUT_ADMISSION_MAX_OPEN_VERIFY_BYTES {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe input admission proof",
            field: "proof.proof.bytes",
            reason: format!(
                "OpenVerifyEnvelope length {} exceeds maximum {}",
                proof_bytes.len(),
                SORACLOUD_FHE_INPUT_ADMISSION_MAX_OPEN_VERIFY_BYTES
            ),
        });
    }
    let envelope = norito::decode_canonical::<OpenVerifyEnvelope>(proof_bytes).map_err(|err| {
        SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe input admission proof",
            field: "proof.proof.bytes",
            reason: format!("must encode a Soracloud FHE OpenVerifyEnvelope: {err}"),
        }
    })?;
    envelope
        .validate_with_bounds(soracloud_fhe_input_admission_open_verify_bounds())
        .map_err(|err| SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe input admission proof",
            field: "proof.proof.bytes",
            reason: format!("invalid OpenVerifyEnvelope shape: {err}"),
        })?;
    if envelope.backend != BackendTag::Stark {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe input admission proof",
            field: "proof.proof.bytes",
            reason: "OpenVerifyEnvelope backend must be STARK".to_string(),
        });
    }
    if envelope.circuit_id != SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1 {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe input admission proof",
            field: "proof.proof.bytes",
            reason: "OpenVerifyEnvelope circuit id must be canonical v1".to_string(),
        });
    }
    if envelope.public_inputs != SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1 {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe input admission proof",
            field: "proof.proof.bytes",
            reason: "OpenVerifyEnvelope public-input schema must be canonical v1".to_string(),
        });
    }
    if vk_commitment != envelope.vk_hash {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe input admission proof",
            field: "proof.vk_commitment",
            reason: "must match OpenVerifyEnvelope.vk_hash".to_string(),
        });
    }
    let open_proof = norito::decode_canonical::<StarkFriOpenProofV1>(&envelope.proof_bytes)
        .map_err(|err| SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe input admission proof",
            field: "proof.proof.bytes",
            reason: format!(
                "OpenVerifyEnvelope proof bytes must encode STARK public inputs: {err}"
            ),
        })?;
    if open_proof.version != 1 {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe input admission proof",
            field: "proof.proof.bytes",
            reason: "STARK public-input wrapper version must be 1".to_string(),
        });
    }
    let expected_public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]];
    if open_proof.public_inputs != expected_public_inputs {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe input admission proof",
            field: "proof.proof.bytes",
            reason: "STARK public inputs must match statement_hash".to_string(),
        });
    }
    validate_soracloud_fhe_stark_native_envelope_bytes(
        "soracloud fhe input admission proof",
        &open_proof.envelope_bytes,
        SORACLOUD_FHE_INPUT_ADMISSION_MAX_NATIVE_ENVELOPE_BYTES,
    )?;
    Ok(())
}
fn validate_soracloud_fhe_public_key_proof_open_verify_envelope(
    proof_bytes: &[u8],
    vk_commitment: [u8; 32],
    statement_hash: Hash,
) -> Result<(), SoracloudManifestError> {
    if proof_bytes.len() > SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_OPEN_VERIFY_BYTES {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe public-key proof",
            field: "proof.proof.bytes",
            reason: format!(
                "OpenVerifyEnvelope length {} exceeds maximum {}",
                proof_bytes.len(),
                SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_OPEN_VERIFY_BYTES
            ),
        });
    }
    let envelope = norito::decode_canonical::<OpenVerifyEnvelope>(proof_bytes).map_err(|err| {
        SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe public-key proof",
            field: "proof.proof.bytes",
            reason: format!("must encode a Soracloud FHE OpenVerifyEnvelope: {err}"),
        }
    })?;
    envelope
        .validate_with_bounds(soracloud_fhe_public_key_proof_open_verify_bounds())
        .map_err(|err| SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe public-key proof",
            field: "proof.proof.bytes",
            reason: format!("invalid OpenVerifyEnvelope shape: {err}"),
        })?;
    if envelope.backend != BackendTag::Stark {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe public-key proof",
            field: "proof.proof.bytes",
            reason: "OpenVerifyEnvelope backend must be STARK".to_string(),
        });
    }
    if envelope.circuit_id != SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1 {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe public-key proof",
            field: "proof.proof.bytes",
            reason: "OpenVerifyEnvelope circuit id must be canonical v1".to_string(),
        });
    }
    if envelope.public_inputs != SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1 {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe public-key proof",
            field: "proof.proof.bytes",
            reason: "OpenVerifyEnvelope public-input schema must be canonical v1".to_string(),
        });
    }
    if vk_commitment != envelope.vk_hash {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe public-key proof",
            field: "proof.vk_commitment",
            reason: "must match OpenVerifyEnvelope.vk_hash".to_string(),
        });
    }
    let open_proof = norito::decode_canonical::<StarkFriOpenProofV1>(&envelope.proof_bytes)
        .map_err(|err| SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe public-key proof",
            field: "proof.proof.bytes",
            reason: format!(
                "OpenVerifyEnvelope proof bytes must encode STARK public inputs: {err}"
            ),
        })?;
    if open_proof.version != 1 {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe public-key proof",
            field: "proof.proof.bytes",
            reason: "STARK public-input wrapper version must be 1".to_string(),
        });
    }
    let expected_public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]];
    if open_proof.public_inputs != expected_public_inputs {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe public-key proof",
            field: "proof.proof.bytes",
            reason: "STARK public inputs must match statement_hash".to_string(),
        });
    }
    validate_soracloud_fhe_stark_native_envelope_bytes(
        "soracloud fhe public-key proof",
        &open_proof.envelope_bytes,
        SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES,
    )?;
    Ok(())
}
fn validate_soracloud_fhe_bootstrap_key_proof_open_verify_envelope(
    proof_bytes: &[u8],
    vk_commitment: [u8; 32],
    statement_hash: Hash,
) -> Result<(), SoracloudManifestError> {
    if proof_bytes.len() > SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_OPEN_VERIFY_BYTES {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe bootstrap key proof",
            field: "proof.proof.bytes",
            reason: format!(
                "OpenVerifyEnvelope length {} exceeds maximum {}",
                proof_bytes.len(),
                SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_OPEN_VERIFY_BYTES
            ),
        });
    }
    let envelope = norito::decode_canonical::<OpenVerifyEnvelope>(proof_bytes).map_err(|err| {
        SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe bootstrap key proof",
            field: "proof.proof.bytes",
            reason: format!("must encode a Soracloud FHE OpenVerifyEnvelope: {err}"),
        }
    })?;
    envelope
        .validate_with_bounds(soracloud_fhe_bootstrap_key_proof_open_verify_bounds())
        .map_err(|err| SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe bootstrap key proof",
            field: "proof.proof.bytes",
            reason: format!("invalid OpenVerifyEnvelope shape: {err}"),
        })?;
    if envelope.backend != BackendTag::Stark {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe bootstrap key proof",
            field: "proof.proof.bytes",
            reason: "OpenVerifyEnvelope backend must be STARK".to_string(),
        });
    }
    if envelope.circuit_id != SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1 {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe bootstrap key proof",
            field: "proof.proof.bytes",
            reason: "OpenVerifyEnvelope circuit id must be canonical v1".to_string(),
        });
    }
    if envelope.public_inputs != SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1 {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe bootstrap key proof",
            field: "proof.proof.bytes",
            reason: "OpenVerifyEnvelope public-input schema must be canonical v1".to_string(),
        });
    }
    if vk_commitment != envelope.vk_hash {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe bootstrap key proof",
            field: "proof.vk_commitment",
            reason: "must match OpenVerifyEnvelope.vk_hash".to_string(),
        });
    }
    let open_proof = norito::decode_canonical::<StarkFriOpenProofV1>(&envelope.proof_bytes)
        .map_err(|err| SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe bootstrap key proof",
            field: "proof.proof.bytes",
            reason: format!(
                "OpenVerifyEnvelope proof bytes must encode STARK public inputs: {err}"
            ),
        })?;
    if open_proof.version != 1 {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe bootstrap key proof",
            field: "proof.proof.bytes",
            reason: "STARK public-input wrapper version must be 1".to_string(),
        });
    }
    let expected_public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]];
    if open_proof.public_inputs != expected_public_inputs {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe bootstrap key proof",
            field: "proof.proof.bytes",
            reason: "STARK public inputs must match statement_hash".to_string(),
        });
    }
    validate_soracloud_fhe_stark_native_envelope_bytes(
        "soracloud fhe bootstrap key proof",
        &open_proof.envelope_bytes,
        SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_NATIVE_ENVELOPE_BYTES,
    )?;
    Ok(())
}
fn validate_soracloud_fhe_full_bootstrap_execution_proof_open_verify_envelope(
    proof_bytes: &[u8],
    vk_commitment: [u8; 32],
    statement_hash: Hash,
) -> Result<(), SoracloudManifestError> {
    if proof_bytes.len() > SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_OPEN_VERIFY_BYTES {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe full-bootstrap execution proof",
            field: "proof.proof.bytes",
            reason: format!(
                "OpenVerifyEnvelope length {} exceeds maximum {}",
                proof_bytes.len(),
                SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_OPEN_VERIFY_BYTES
            ),
        });
    }
    let envelope = norito::decode_canonical::<OpenVerifyEnvelope>(proof_bytes).map_err(|err| {
        SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe full-bootstrap execution proof",
            field: "proof.proof.bytes",
            reason: format!("must encode a Soracloud FHE OpenVerifyEnvelope: {err}"),
        }
    })?;
    envelope
        .validate_with_bounds(soracloud_fhe_full_bootstrap_execution_proof_open_verify_bounds())
        .map_err(|err| SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe full-bootstrap execution proof",
            field: "proof.proof.bytes",
            reason: format!("invalid OpenVerifyEnvelope shape: {err}"),
        })?;
    if envelope.backend != BackendTag::Stark {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe full-bootstrap execution proof",
            field: "proof.proof.bytes",
            reason: "OpenVerifyEnvelope backend must be STARK".to_string(),
        });
    }
    if envelope.circuit_id != SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1 {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe full-bootstrap execution proof",
            field: "proof.proof.bytes",
            reason: "OpenVerifyEnvelope circuit id must be canonical v1".to_string(),
        });
    }
    if envelope.public_inputs
        != SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1
    {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe full-bootstrap execution proof",
            field: "proof.proof.bytes",
            reason: "OpenVerifyEnvelope public-input schema must be canonical v1".to_string(),
        });
    }
    if vk_commitment != envelope.vk_hash {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe full-bootstrap execution proof",
            field: "proof.vk_commitment",
            reason: "must match OpenVerifyEnvelope.vk_hash".to_string(),
        });
    }
    let open_proof = norito::decode_canonical::<StarkFriOpenProofV1>(&envelope.proof_bytes)
        .map_err(|err| SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe full-bootstrap execution proof",
            field: "proof.proof.bytes",
            reason: format!(
                "OpenVerifyEnvelope proof bytes must encode STARK public inputs: {err}"
            ),
        })?;
    if open_proof.version != 1 {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe full-bootstrap execution proof",
            field: "proof.proof.bytes",
            reason: "STARK public-input wrapper version must be 1".to_string(),
        });
    }
    let expected_public_inputs = vec![vec![<[u8; Hash::LENGTH]>::from(statement_hash)]];
    if open_proof.public_inputs != expected_public_inputs {
        return Err(SoracloudManifestError::InvalidField {
            manifest: "soracloud fhe full-bootstrap execution proof",
            field: "proof.proof.bytes",
            reason: "STARK public inputs must match statement_hash".to_string(),
        });
    }
    validate_soracloud_fhe_stark_native_envelope_bytes(
        "soracloud fhe full-bootstrap execution proof",
        &open_proof.envelope_bytes,
        SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_NATIVE_ENVELOPE_BYTES,
    )?;
    Ok(())
}
/// Return shared `OpenVerifyEnvelope` bounds for Soracloud FHE input admission.
///
/// Data-model validation and Core runtime admission both use these limits so outer envelope, STARK
/// wrapper, canonical metadata, and auxiliary-byte policy cannot drift.
#[must_use]
pub fn soracloud_fhe_input_admission_open_verify_bounds() -> OpenVerifyEnvelopeBounds {
    OpenVerifyEnvelopeBounds {
        max_circuit_id_bytes: SORACLOUD_FHE_INPUT_ADMISSION_CIRCUIT_ID_V1.len(),
        max_public_input_bytes: SORACLOUD_FHE_INPUT_ADMISSION_PUBLIC_INPUTS_SCHEMA_V1.len(),
        max_proof_bytes: SORACLOUD_FHE_INPUT_ADMISSION_MAX_STARK_WRAPPER_BYTES,
        max_aux_bytes: 0,
        allow_aux: false,
        ..OpenVerifyEnvelopeBounds::default()
    }
}
/// Return shared `OpenVerifyEnvelope` bounds for Soracloud FHE public-key proofs.
///
/// Data-model validation and Core runtime admission both use these limits so outer envelope, STARK
/// wrapper, canonical metadata, and auxiliary-byte policy cannot drift.
#[must_use]
pub fn soracloud_fhe_public_key_proof_open_verify_bounds() -> OpenVerifyEnvelopeBounds {
    OpenVerifyEnvelopeBounds {
        max_circuit_id_bytes: SORACLOUD_FHE_PUBLIC_KEY_PROOF_CIRCUIT_ID_V1.len(),
        max_public_input_bytes: SORACLOUD_FHE_PUBLIC_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1.len(),
        max_proof_bytes: SORACLOUD_FHE_PUBLIC_KEY_PROOF_MAX_STARK_WRAPPER_BYTES,
        max_aux_bytes: 0,
        allow_aux: false,
        ..OpenVerifyEnvelopeBounds::default()
    }
}
/// Return shared `OpenVerifyEnvelope` bounds for Soracloud FHE bootstrap-key proofs.
///
/// Data-model validation and Core runtime admission both use these limits so outer envelope, STARK
/// wrapper, canonical metadata, and auxiliary-byte policy cannot drift.
#[must_use]
pub fn soracloud_fhe_bootstrap_key_proof_open_verify_bounds() -> OpenVerifyEnvelopeBounds {
    OpenVerifyEnvelopeBounds {
        max_circuit_id_bytes: SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_CIRCUIT_ID_V1.len(),
        max_public_input_bytes: SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_PUBLIC_INPUTS_SCHEMA_V1.len(),
        max_proof_bytes: SORACLOUD_FHE_BOOTSTRAP_KEY_PROOF_MAX_STARK_WRAPPER_BYTES,
        max_aux_bytes: 0,
        allow_aux: false,
        ..OpenVerifyEnvelopeBounds::default()
    }
}
/// Return shared `OpenVerifyEnvelope` bounds for full-bootstrap execution proofs.
///
/// Data-model validation and Core runtime admission both use these limits so outer envelope, STARK
/// wrapper, canonical metadata, and auxiliary-byte policy cannot drift.
#[must_use]
pub fn soracloud_fhe_full_bootstrap_execution_proof_open_verify_bounds() -> OpenVerifyEnvelopeBounds
{
    OpenVerifyEnvelopeBounds {
        max_circuit_id_bytes: SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_CIRCUIT_ID_V1.len(),
        max_public_input_bytes:
            SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_PUBLIC_INPUTS_SCHEMA_V1.len(),
        max_proof_bytes: SORACLOUD_FHE_FULL_BOOTSTRAP_EXECUTION_PROOF_MAX_STARK_WRAPPER_BYTES,
        max_aux_bytes: 0,
        allow_aux: false,
        ..OpenVerifyEnvelopeBounds::default()
    }
}
/// Encryption class for an opaque secret envelope payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "encryption", content = "value"))]
pub enum SecretEnvelopeEncryptionV1 {
    /// Payload is client-encrypted and opaque to validators.
    ClientCiphertext,
    /// Payload is FHE ciphertext and may be operated on homomorphically.
    FheCiphertext,
}
/// Opaque encrypted payload with commitment used by ciphertext-native state.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SecretEnvelopeV1 {
    /// Schema version; must equal [`SECRET_ENVELOPE_VERSION_V1`].
    pub schema_version: u16,
    /// Encryption class used by the payload.
    pub encryption: SecretEnvelopeEncryptionV1,
    /// Key material identifier (KMS alias / threshold key id / FHE key tag).
    pub key_id: String,
    /// Key version under the same `key_id`.
    pub key_version: NonZeroU32,
    /// Deterministic nonce/IV bytes supplied by the producer.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub nonce: Vec<u8>,
    /// Opaque encrypted payload bytes.
    #[cfg_attr(feature = "json", norito(json = "crate::json_helpers::base64_vec"))]
    pub ciphertext: Vec<u8>,
    /// Commitment hash for verifying payload integrity against metadata.
    pub commitment: Hash,
    /// Optional digest over associated public metadata.
    #[norito(default)]
    pub aad_digest: Option<Hash>,
}
impl SecretEnvelopeV1 {
    const MAX_NONCE_BYTES: usize = 256;
    const MAX_CIPHERTEXT_BYTES: usize = 33_554_432;
    /// Validate schema version and ciphertext envelope constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// encrypted payload fields violate deterministic bounds.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != SECRET_ENVELOPE_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "secret envelope",
                expected: SECRET_ENVELOPE_VERSION_V1,
                found: self.schema_version,
            });
        }
        if self.key_id.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "secret envelope",
                field: "key_id",
            });
        }
        if self.nonce.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "secret envelope",
                field: "nonce",
            });
        }
        if self.nonce.len() > Self::MAX_NONCE_BYTES {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "secret envelope",
                field: "nonce",
                reason: format!(
                    "length {} exceeds max {} bytes",
                    self.nonce.len(),
                    Self::MAX_NONCE_BYTES
                ),
            });
        }
        if self.ciphertext.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "secret envelope",
                field: "ciphertext",
            });
        }
        if self.ciphertext.len() > Self::MAX_CIPHERTEXT_BYTES {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "secret envelope",
                field: "ciphertext",
                reason: format!(
                    "length {} exceeds max {} bytes",
                    self.ciphertext.len(),
                    Self::MAX_CIPHERTEXT_BYTES
                ),
            });
        }
        validate_soracloud_digest_hash("secret envelope", "commitment", self.commitment)?;
        if let Some(aad_digest) = self.aad_digest {
            validate_soracloud_digest_hash("secret envelope", "aad_digest", aad_digest)?;
        }
        Ok(())
    }
}
/// Public metadata attached to ciphertext-native state records.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct CiphertextStateMetadataV1 {
    /// MIME-style content hint for encrypted payload decoding.
    pub content_type: String,
    /// Ciphertext payload size in bytes.
    pub payload_bytes: NonZeroU64,
    /// Commitment hash mirrored from the secret envelope.
    pub commitment: Hash,
    /// Optional governance policy tag for access/disclosure controls.
    #[norito(default)]
    pub policy_tag: Option<String>,
    /// Optional deterministic labels for index/query routing.
    #[norito(default)]
    pub tags: Vec<String>,
}
impl CiphertextStateMetadataV1 {
    /// Validate metadata fields for deterministic ciphertext state indexing.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when metadata fields are empty or
    /// include duplicate tag entries.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.content_type.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "ciphertext state metadata",
                field: "content_type",
            });
        }
        validate_soracloud_digest_hash("ciphertext state metadata", "commitment", self.commitment)?;
        if let Some(policy_tag) = self.policy_tag.as_ref()
            && policy_tag.trim().is_empty()
        {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "ciphertext state metadata",
                field: "policy_tag",
                reason: "must not be empty when provided".to_string(),
            });
        }
        let mut seen = BTreeSet::new();
        for tag in &self.tags {
            let normalized = tag.trim();
            if normalized.is_empty() {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "ciphertext state metadata",
                    field: "tags",
                    reason: "tag entries must be non-empty".to_string(),
                });
            }
            if !seen.insert(normalized.to_owned()) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "ciphertext state metadata",
                    field: "tags",
                    reason: format!("duplicate tag `{normalized}`"),
                });
            }
        }
        Ok(())
    }
}
/// Ciphertext-native key-value record with public metadata and secret payload.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct CiphertextStateRecordV1 {
    /// Schema version; must equal [`CIPHERTEXT_STATE_RECORD_VERSION_V1`].
    pub schema_version: u16,
    /// Binding that governs this encrypted state key.
    pub binding_name: Name,
    /// Canonical key path scoped under a state binding prefix.
    pub state_key: String,
    /// Publicly visible metadata used for indexing, policy, and audit.
    pub metadata: CiphertextStateMetadataV1,
    /// Encrypted payload envelope.
    pub secret: SecretEnvelopeV1,
}
impl CiphertextStateRecordV1 {
    /// Validate schema version and metadata/secret consistency constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch,
    /// key paths are invalid, or metadata does not match secret payload state.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != CIPHERTEXT_STATE_RECORD_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "ciphertext state record",
                expected: CIPHERTEXT_STATE_RECORD_VERSION_V1,
                found: self.schema_version,
            });
        }
        if self.state_key.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "ciphertext state record",
                field: "state_key",
            });
        }
        if !self.state_key.starts_with('/') {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "ciphertext state record",
                field: "state_key",
                reason: "must start with '/'".to_string(),
            });
        }
        self.metadata.validate()?;
        self.secret.validate()?;
        let ciphertext_len = u64::try_from(self.secret.ciphertext.len())
            .expect("ciphertext length always fits in u64");
        if self.metadata.payload_bytes.get() != ciphertext_len {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "ciphertext state record",
                field: "metadata.payload_bytes",
                reason: format!(
                    "metadata declares {} bytes but envelope has {} bytes",
                    self.metadata.payload_bytes, ciphertext_len
                ),
            });
        }
        if self.metadata.commitment != self.secret.commitment {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "ciphertext state record",
                field: "metadata.commitment",
                reason: "must match secret.commitment".to_string(),
            });
        }
        Ok(())
    }
}
/// Deterministic FHE operation class admitted for Soracloud ciphertext jobs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "operation", content = "value"))]
pub enum FheJobOperationV1 {
    /// Element-wise homomorphic addition over two or more inputs.
    Add,
    /// Element-wise homomorphic multiplication over two or more inputs.
    Multiply,
    /// Deterministic left-rotation over one ciphertext input.
    ///
    /// Single-ciphertext packed BFV envelopes use public Galois key switching, including masked key
    /// schedules for rotations that are not one automorphism. Multi-slot identifier envelopes use
    /// the outer ciphertext-slot rotation path.
    RotateLeft,
    /// Deterministic bootstrap/relinearization refresh over one input.
    Bootstrap,
}
/// Input ciphertext reference for deterministic FHE job admission.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct FheJobInputRefV1 {
    /// Canonical state key of the ciphertext input.
    pub state_key: String,
    /// Input payload size in bytes.
    pub payload_bytes: NonZeroU64,
    /// Input commitment hash bound to the ciphertext payload.
    pub commitment: Hash,
}
impl FheJobInputRefV1 {
    /// Validate deterministic input reference constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when state keys are empty or outside
    /// canonical path formatting rules.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.state_key.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "fhe job spec",
                field: "inputs.state_key",
            });
        }
        if !self.state_key.starts_with('/') {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "inputs.state_key",
                reason: "must start with '/'".to_string(),
            });
        }
        validate_soracloud_fhe_digest_hash("fhe job spec", "inputs.commitment", self.commitment)?;
        Ok(())
    }
}
/// Deterministic FHE admission/execution job descriptor.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct FheJobSpecV1 {
    /// Schema version; must equal [`FHE_JOB_SPEC_VERSION_V1`].
    pub schema_version: u16,
    /// Stable deterministic job identifier.
    pub job_id: String,
    /// Referenced deterministic execution policy identifier.
    pub policy_name: Name,
    /// Referenced parameter-set identifier.
    pub param_set: Name,
    /// Referenced parameter-set version.
    pub param_set_version: NonZeroU32,
    /// Homomorphic operation class.
    pub operation: FheJobOperationV1,
    /// Ordered ciphertext inputs for deterministic replay.
    #[norito(default)]
    pub inputs: Vec<FheJobInputRefV1>,
    /// Output ciphertext state key.
    pub output_state_key: String,
    /// Requested multiplicative depth consumed by this job.
    pub requested_multiplication_depth: u16,
    /// Left-rotation steps requested for slot-rotation jobs.
    pub rotation_steps: u32,
    /// Number of bootstrap/refresh operations requested.
    pub bootstrap_count: u16,
}
impl FheJobSpecV1 {
    /// Validate schema version and deterministic FHE job constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when job identifiers, key paths,
    /// inputs, or operation-specific constraints are invalid.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != FHE_JOB_SPEC_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "fhe job spec",
                expected: FHE_JOB_SPEC_VERSION_V1,
                found: self.schema_version,
            });
        }
        if self.job_id.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "fhe job spec",
                field: "job_id",
            });
        }
        if self.output_state_key.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "fhe job spec",
                field: "output_state_key",
            });
        }
        if !self.output_state_key.starts_with('/') {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "output_state_key",
                reason: "must start with '/'".to_string(),
            });
        }
        if self.inputs.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "fhe job spec",
                field: "inputs",
            });
        }
        let mut seen_inputs = BTreeSet::new();
        for input in &self.inputs {
            input.validate()?;
            if !seen_inputs.insert(input.state_key.clone()) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "fhe job spec",
                    field: "inputs.state_key",
                    reason: format!("duplicate input key `{}`", input.state_key),
                });
            }
        }
        match self.operation {
            FheJobOperationV1::Add => {
                if self.requested_multiplication_depth != 0 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "requested_multiplication_depth",
                        reason: "add operation must use depth 0".to_string(),
                    });
                }
                if self.rotation_steps != 0 || self.bootstrap_count != 0 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "operation",
                        reason: "add operation cannot request rotation/bootstrap".to_string(),
                    });
                }
                if self.inputs.len() < 2 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "inputs",
                        reason: "add operation requires at least two inputs".to_string(),
                    });
                }
            }
            FheJobOperationV1::Multiply => {
                if self.requested_multiplication_depth == 0 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "requested_multiplication_depth",
                        reason: "multiply operation requires non-zero depth".to_string(),
                    });
                }
                if self.rotation_steps != 0 || self.bootstrap_count != 0 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "operation",
                        reason: "multiply operation cannot request rotation/bootstrap".to_string(),
                    });
                }
                if self.inputs.len() < 2 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "inputs",
                        reason: "multiply operation requires at least two inputs".to_string(),
                    });
                }
                let balanced_depth =
                    bfv_balanced_multiplication_depth(self.inputs.len()).map_err(|err| {
                        SoracloudManifestError::InvalidField {
                            manifest: "fhe job spec",
                            field: "inputs",
                            reason: err.to_string(),
                        }
                    })?;
                if self.requested_multiplication_depth < balanced_depth {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "requested_multiplication_depth",
                        reason: format!(
                            "requested depth {} under-declares balanced multiply depth {balanced_depth}",
                            self.requested_multiplication_depth
                        ),
                    });
                }
            }
            FheJobOperationV1::RotateLeft => {
                if self.rotation_steps == 0 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "rotation_steps",
                        reason: "rotate operation requires non-zero rotation_steps".to_string(),
                    });
                }
                if self.requested_multiplication_depth != 0 || self.bootstrap_count != 0 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "operation",
                        reason: "rotate operation cannot request depth/bootstrap".to_string(),
                    });
                }
                if self.inputs.len() != 1 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "inputs",
                        reason: "rotate operation requires exactly one input".to_string(),
                    });
                }
            }
            FheJobOperationV1::Bootstrap => {
                if self.bootstrap_count == 0 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "bootstrap_count",
                        reason: "bootstrap operation requires non-zero bootstrap_count".to_string(),
                    });
                }
                if self.requested_multiplication_depth != 0 || self.rotation_steps != 0 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "operation",
                        reason: "bootstrap operation cannot request depth/rotation".to_string(),
                    });
                }
                if self.inputs.len() != 1 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "fhe job spec",
                        field: "inputs",
                        reason: "bootstrap operation requires exactly one input".to_string(),
                    });
                }
            }
        }
        Ok(())
    }
    /// Validate job admission against deterministic policy + parameter constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when policy linkage mismatches, input
    /// bounds exceed policy limits, or deterministic output bounds are violated.
    #[allow(clippy::too_many_lines)]
    pub fn validate_for_execution(
        &self,
        policy: &FheExecutionPolicyV1,
        param_set: &FheParamSetV1,
    ) -> Result<(), SoracloudManifestError> {
        self.validate()?;
        policy.validate_for_param_set(param_set)?;
        if self.policy_name != policy.policy_name {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "policy_name",
                reason: format!(
                    "job references `{}` but policy is `{}`",
                    self.policy_name, policy.policy_name
                ),
            });
        }
        if self.param_set != policy.param_set {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "param_set",
                reason: format!(
                    "job references `{}` but policy is `{}`",
                    self.param_set, policy.param_set
                ),
            });
        }
        if self.param_set_version != policy.param_set_version {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "param_set_version",
                reason: format!(
                    "job references version {} but policy is version {}",
                    self.param_set_version, policy.param_set_version
                ),
            });
        }
        let input_count =
            u16::try_from(self.inputs.len()).map_err(|_| SoracloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "inputs",
                reason: "input count exceeds supported u16 range".to_string(),
            })?;
        if input_count > policy.max_input_ciphertexts.get() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "inputs",
                reason: format!(
                    "input count {} exceeds policy max_input_ciphertexts {}",
                    input_count, policy.max_input_ciphertexts
                ),
            });
        }
        if self.requested_multiplication_depth > policy.max_multiplication_depth.get() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "requested_multiplication_depth",
                reason: format!(
                    "requested depth {} exceeds policy max_multiplication_depth {}",
                    self.requested_multiplication_depth, policy.max_multiplication_depth
                ),
            });
        }
        if self.rotation_steps > policy.max_rotation_count.get() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "rotation_steps",
                reason: format!(
                    "rotation_steps {} exceeds policy max_rotation_count {}",
                    self.rotation_steps, policy.max_rotation_count
                ),
            });
        }
        if self.bootstrap_count > policy.max_bootstrap_count {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "bootstrap_count",
                reason: format!(
                    "bootstrap_count {} exceeds policy max_bootstrap_count {}",
                    self.bootstrap_count, policy.max_bootstrap_count
                ),
            });
        }
        for input in &self.inputs {
            if input.payload_bytes > policy.max_ciphertext_bytes {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "fhe job spec",
                    field: "inputs.payload_bytes",
                    reason: format!(
                        "input payload {} exceeds policy max_ciphertext_bytes {}",
                        input.payload_bytes, policy.max_ciphertext_bytes
                    ),
                });
            }
        }
        let output_bytes = self.try_deterministic_output_payload_bytes()?;
        if output_bytes > policy.max_ciphertext_bytes.get() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "fhe job spec",
                field: "output_state_key",
                reason: format!(
                    "deterministic output size {} exceeds policy max_ciphertext_bytes {}",
                    output_bytes, policy.max_ciphertext_bytes
                ),
            });
        }
        Ok(())
    }
    /// Try to compute the deterministic projected output payload size in bytes.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when the operation overhead or final
    /// projected output size cannot be represented as a `u64`.
    pub fn try_deterministic_output_payload_bytes(&self) -> Result<u64, SoracloudManifestError> {
        let max_input = self
            .inputs
            .iter()
            .map(|input| input.payload_bytes.get())
            .max()
            .unwrap_or(0);
        let overflow = |reason: String| SoracloudManifestError::InvalidField {
            manifest: "fhe job spec",
            field: "output_state_key",
            reason,
        };
        let op_overhead = match self.operation {
            FheJobOperationV1::Add => 16,
            FheJobOperationV1::Multiply => u64::from(self.requested_multiplication_depth)
                .checked_mul(64)
                .ok_or_else(|| overflow("multiply output overhead exceeds u64".to_string()))?,
            FheJobOperationV1::RotateLeft => u64::from(self.rotation_steps).min(1_024),
            FheJobOperationV1::Bootstrap => u64::from(self.bootstrap_count)
                .checked_mul(128)
                .ok_or_else(|| overflow("bootstrap output overhead exceeds u64".to_string()))?,
        };
        max_input
            .checked_add(op_overhead)
            .map(|output_bytes| output_bytes.max(1))
            .ok_or_else(|| {
                overflow(format!(
                    "deterministic output size overflows u64: max input {max_input} plus overhead {op_overhead}"
                ))
            })
    }
    /// Deterministic projected output payload size in bytes for admission checks.
    #[must_use]
    pub fn deterministic_output_payload_bytes(&self) -> u64 {
        self.try_deterministic_output_payload_bytes()
            .unwrap_or(u64::MAX)
    }
    /// Deterministic output commitment derived from operation + input commitments.
    #[must_use]
    pub fn deterministic_output_commitment(&self) -> Hash {
        let input_commitments = self
            .inputs
            .iter()
            .map(|input| input.commitment)
            .collect::<Vec<_>>();
        Hash::new(Encode::encode(&(
            self.job_id.clone(),
            self.policy_name.clone(),
            self.param_set.clone(),
            self.param_set_version,
            self.operation,
            self.requested_multiplication_depth,
            self.rotation_steps,
            self.bootstrap_count,
            input_commitments,
        )))
    }
}
/// Decryption authority mode enforced for private-state disclosure requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "mode", content = "value"))]
pub enum DecryptionAuthorityModeV1 {
    /// Ciphertext keys are client-held; network records request/audit only.
    ClientHeld,
    /// Decryption requires threshold service approvals from policy members.
    ThresholdService,
}
/// Governance-managed policy for decryption authority and request gating.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DecryptionAuthorityPolicyV1 {
    /// Schema version; must equal [`DECRYPTION_AUTHORITY_POLICY_VERSION_V1`].
    pub schema_version: u16,
    /// Stable decryption policy identifier.
    pub policy_name: Name,
    /// Decryption authority mode.
    pub mode: DecryptionAuthorityModeV1,
    /// Required approvals for threshold-mode decryption.
    pub approver_quorum: NonZeroU16,
    /// Ordered unique approver identities allowed by the policy.
    #[norito(default)]
    pub approver_ids: Vec<Name>,
    /// Whether emergency break-glass requests are allowed.
    pub allow_break_glass: bool,
    /// Canonical jurisdiction/compliance tag enforced for requests.
    pub jurisdiction_tag: String,
    /// Whether non-break-glass requests must include consent evidence.
    pub require_consent_evidence: bool,
    /// Maximum request TTL in blocks.
    pub max_ttl_blocks: NonZeroU32,
    /// Canonical audit tag attached to request records.
    pub audit_tag: String,
}
impl DecryptionAuthorityPolicyV1 {
    /// Validate schema version and deterministic decryption-policy constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when quorum, approver ordering,
    /// mode constraints, or audit-tag rules are violated.
    #[allow(clippy::too_many_lines)]
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != DECRYPTION_AUTHORITY_POLICY_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "decryption authority policy",
                expected: DECRYPTION_AUTHORITY_POLICY_VERSION_V1,
                found: self.schema_version,
            });
        }
        if self.approver_ids.is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "decryption authority policy",
                field: "approver_ids",
            });
        }
        let mut seen = BTreeSet::new();
        for approver in &self.approver_ids {
            if !seen.insert(approver.clone()) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "decryption authority policy",
                    field: "approver_ids",
                    reason: format!("duplicate approver `{approver}`"),
                });
            }
        }
        if self
            .approver_ids
            .windows(2)
            .any(|pair| pair[0].as_ref() >= pair[1].as_ref())
        {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "decryption authority policy",
                field: "approver_ids",
                reason: "must be strictly sorted in ascending lexical order".to_string(),
            });
        }
        let approver_count =
            u16::try_from(self.approver_ids.len()).expect("approver count always fits into u16");
        match self.mode {
            DecryptionAuthorityModeV1::ClientHeld => {
                if approver_count != 1 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "decryption authority policy",
                        field: "approver_ids",
                        reason: "client-held mode requires exactly one approver".to_string(),
                    });
                }
                if self.approver_quorum.get() != 1 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "decryption authority policy",
                        field: "approver_quorum",
                        reason: "client-held mode requires approver_quorum=1".to_string(),
                    });
                }
                if self.allow_break_glass {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "decryption authority policy",
                        field: "allow_break_glass",
                        reason: "client-held mode must not enable break-glass".to_string(),
                    });
                }
            }
            DecryptionAuthorityModeV1::ThresholdService => {
                if approver_count < 2 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "decryption authority policy",
                        field: "approver_ids",
                        reason: "threshold mode requires at least two approvers".to_string(),
                    });
                }
                if self.approver_quorum.get() > approver_count {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "decryption authority policy",
                        field: "approver_quorum",
                        reason: format!(
                            "approver_quorum {} exceeds approver count {}",
                            self.approver_quorum, approver_count
                        ),
                    });
                }
            }
        }
        if self.jurisdiction_tag.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "decryption authority policy",
                field: "jurisdiction_tag",
            });
        }
        if self.jurisdiction_tag.chars().any(char::is_control) {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "decryption authority policy",
                field: "jurisdiction_tag",
                reason: "must not contain control characters".to_string(),
            });
        }
        if self.audit_tag.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "decryption authority policy",
                field: "audit_tag",
            });
        }
        if self.audit_tag.chars().any(char::is_control) {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "decryption authority policy",
                field: "audit_tag",
                reason: "must not contain control characters".to_string(),
            });
        }
        Ok(())
    }
}
/// Decryption request envelope gated by a [`DecryptionAuthorityPolicyV1`].
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct DecryptionRequestV1 {
    /// Schema version; must equal [`DECRYPTION_REQUEST_VERSION_V1`].
    pub schema_version: u16,
    /// Stable request identifier.
    pub request_id: String,
    /// Referenced decryption policy identifier.
    pub policy_name: Name,
    /// Binding owning the ciphertext state.
    pub binding_name: Name,
    /// Canonical state key of requested ciphertext material.
    pub state_key: String,
    /// Commitment of the ciphertext payload being disclosed.
    pub ciphertext_commitment: Hash,
    /// Human-readable justification captured for immutable audit.
    pub justification: String,
    /// Jurisdiction/compliance tag for this disclosure request.
    pub jurisdiction_tag: String,
    /// Optional consent evidence commitment hash.
    #[norito(default)]
    pub consent_evidence_hash: Option<Hash>,
    /// Requested TTL in blocks before request expiry.
    pub requested_ttl_blocks: NonZeroU32,
    /// Break-glass flag for emergency disclosure attempts.
    pub break_glass: bool,
    /// Optional break-glass reason, required when `break_glass=true`.
    #[norito(default)]
    pub break_glass_reason: Option<String>,
    /// Governance linkage hash for policy-driven auditability.
    pub governance_tx_hash: Hash,
}
impl DecryptionRequestV1 {
    /// Validate schema version and base request integrity constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when request identifiers, key paths,
    /// or justification fields violate deterministic requirements.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != DECRYPTION_REQUEST_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "decryption request",
                expected: DECRYPTION_REQUEST_VERSION_V1,
                found: self.schema_version,
            });
        }
        if self.request_id.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "decryption request",
                field: "request_id",
            });
        }
        if self.state_key.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "decryption request",
                field: "state_key",
            });
        }
        if !self.state_key.starts_with('/') {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "decryption request",
                field: "state_key",
                reason: "must start with '/'".to_string(),
            });
        }
        validate_soracloud_digest_hash(
            "decryption request",
            "ciphertext_commitment",
            self.ciphertext_commitment,
        )?;
        if self.justification.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "decryption request",
                field: "justification",
            });
        }
        if self.jurisdiction_tag.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "decryption request",
                field: "jurisdiction_tag",
            });
        }
        if self.jurisdiction_tag.chars().any(char::is_control) {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "decryption request",
                field: "jurisdiction_tag",
                reason: "must not contain control characters".to_string(),
            });
        }
        if let Some(consent_evidence_hash) = self.consent_evidence_hash {
            validate_soracloud_digest_hash(
                "decryption request",
                "consent_evidence_hash",
                consent_evidence_hash,
            )?;
        }
        validate_soracloud_digest_hash(
            "decryption request",
            "governance_tx_hash",
            self.governance_tx_hash,
        )?;
        if self.break_glass {
            let has_reason = self
                .break_glass_reason
                .as_deref()
                .is_some_and(|reason| !reason.trim().is_empty());
            if !has_reason {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "decryption request",
                    field: "break_glass_reason",
                    reason: "must be provided when break_glass=true".to_string(),
                });
            }
        } else if self.break_glass_reason.is_some() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "decryption request",
                field: "break_glass_reason",
                reason: "must be omitted when break_glass=false".to_string(),
            });
        }
        Ok(())
    }
    /// Validate request admission against decryption authority policy rules.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when policy linkage mismatches, TTL
    /// exceeds policy limits, or consent/break-glass policy gates are violated.
    pub fn validate_for_policy(
        &self,
        policy: &DecryptionAuthorityPolicyV1,
    ) -> Result<(), SoracloudManifestError> {
        self.validate()?;
        policy.validate()?;
        if self.policy_name != policy.policy_name {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "decryption request",
                field: "policy_name",
                reason: format!(
                    "request references `{}` but policy is `{}`",
                    self.policy_name, policy.policy_name
                ),
            });
        }
        if self.requested_ttl_blocks > policy.max_ttl_blocks {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "decryption request",
                field: "requested_ttl_blocks",
                reason: format!(
                    "requested TTL {} exceeds policy max_ttl_blocks {}",
                    self.requested_ttl_blocks, policy.max_ttl_blocks
                ),
            });
        }
        if self.jurisdiction_tag != policy.jurisdiction_tag {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "decryption request",
                field: "jurisdiction_tag",
                reason: format!(
                    "request jurisdiction `{}` does not match policy jurisdiction `{}`",
                    self.jurisdiction_tag, policy.jurisdiction_tag
                ),
            });
        }
        if policy.require_consent_evidence
            && !self.break_glass
            && self.consent_evidence_hash.is_none()
        {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "decryption request",
                field: "consent_evidence_hash",
                reason: "policy requires consent evidence for non-break-glass requests".to_string(),
            });
        }
        if self.break_glass && !policy.allow_break_glass {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "decryption request",
                field: "break_glass",
                reason: "policy does not allow break-glass disclosure".to_string(),
            });
        }
        Ok(())
    }
}
/// Metadata projection level for ciphertext query responses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
#[cfg_attr(feature = "json", norito(tag = "metadata_level", content = "value"))]
pub enum CiphertextQueryMetadataLevelV1 {
    /// Return only digest-level key references.
    Minimal,
    /// Return canonical state keys in addition to digest references.
    Standard,
}
/// Deterministic query specification for ciphertext-only state lookups.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct CiphertextQuerySpecV1 {
    /// Schema version; must equal [`CIPHERTEXT_QUERY_SPEC_VERSION_V1`].
    pub schema_version: u16,
    /// Service name to query.
    pub service_name: Name,
    /// Binding constrained to ciphertext-capable state.
    pub binding_name: Name,
    /// Canonical key-prefix filter scoped under binding policy.
    pub state_key_prefix: String,
    /// Maximum result count for deterministic bounded scans.
    pub max_results: NonZeroU16,
    /// Metadata projection level for non-disclosure behavior.
    pub metadata_level: CiphertextQueryMetadataLevelV1,
    /// Whether inclusion proofs should be attached to each result row.
    pub include_proof: bool,
}
impl CiphertextQuerySpecV1 {
    const MAX_RESULTS_LIMIT: u16 = 256;
    /// Validate deterministic ciphertext query constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch, key
    /// prefixes are invalid, or result limits exceed deterministic bounds.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != CIPHERTEXT_QUERY_SPEC_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "ciphertext query spec",
                expected: CIPHERTEXT_QUERY_SPEC_VERSION_V1,
                found: self.schema_version,
            });
        }
        if self.state_key_prefix.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "ciphertext query spec",
                field: "state_key_prefix",
            });
        }
        if !self.state_key_prefix.starts_with('/') {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "ciphertext query spec",
                field: "state_key_prefix",
                reason: "must start with '/'".to_string(),
            });
        }
        if self.max_results.get() > Self::MAX_RESULTS_LIMIT {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "ciphertext query spec",
                field: "max_results",
                reason: format!(
                    "max_results {} exceeds deterministic limit {}",
                    self.max_results,
                    Self::MAX_RESULTS_LIMIT
                ),
            });
        }
        Ok(())
    }
}
/// Inclusion proof attached to ciphertext query results.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct CiphertextInclusionProofV1 {
    /// Schema version; must equal [`CIPHERTEXT_QUERY_PROOF_VERSION_V1`].
    pub schema_version: u16,
    /// Proof algorithm identifier.
    pub proof_scheme: String,
    /// Hash of the referenced audit leaf payload.
    pub leaf_hash: Hash,
    /// Hash anchor over audit history up to `anchor_sequence`.
    pub anchor_hash: Hash,
    /// Sequence for the anchor checkpoint.
    pub anchor_sequence: u64,
    /// Sequence of the leaf event this proof attests to.
    pub event_sequence: u64,
}
impl CiphertextInclusionProofV1 {
    /// Validate inclusion-proof envelope constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch or
    /// proof metadata is empty/inconsistent.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != CIPHERTEXT_QUERY_PROOF_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "ciphertext inclusion proof",
                expected: CIPHERTEXT_QUERY_PROOF_VERSION_V1,
                found: self.schema_version,
            });
        }
        if self.proof_scheme.trim().is_empty() {
            return Err(SoracloudManifestError::EmptyField {
                manifest: "ciphertext inclusion proof",
                field: "proof_scheme",
            });
        }
        validate_soracloud_digest_hash("ciphertext inclusion proof", "leaf_hash", self.leaf_hash)?;
        validate_soracloud_digest_hash(
            "ciphertext inclusion proof",
            "anchor_hash",
            self.anchor_hash,
        )?;
        if self.anchor_sequence < self.event_sequence {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "ciphertext inclusion proof",
                field: "anchor_sequence",
                reason: format!(
                    "anchor_sequence {} must be >= event_sequence {}",
                    self.anchor_sequence, self.event_sequence
                ),
            });
        }
        Ok(())
    }
}
/// A single query result row for ciphertext metadata lookups.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct CiphertextQueryResultItemV1 {
    /// Binding owning the ciphertext state row.
    pub binding_name: Name,
    /// Canonical state key when `Standard` metadata projection is used.
    #[norito(default)]
    pub state_key: Option<String>,
    /// Digest reference for the state key.
    pub state_key_digest: Hash,
    /// Ciphertext payload size in bytes.
    pub payload_bytes: NonZeroU64,
    /// Ciphertext commitment hash.
    pub ciphertext_commitment: Hash,
    /// Encryption mode for the stored ciphertext.
    pub encryption: SoraStateEncryptionV1,
    /// Latest update sequence observed for this state key.
    pub last_update_sequence: u64,
    /// Governance linkage hash associated with the ciphertext mutation.
    pub governance_tx_hash: Hash,
    /// Optional inclusion proof for this row.
    #[norito(default)]
    pub proof: Option<CiphertextInclusionProofV1>,
}
impl CiphertextQueryResultItemV1 {
    /// Validate a single ciphertext query result item.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when key/reference fields are invalid
    /// or plaintext encryption is surfaced in a ciphertext query row.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.encryption == SoraStateEncryptionV1::Plaintext {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "ciphertext query result item",
                field: "encryption",
                reason: "plaintext rows must not be returned".to_string(),
            });
        }
        if let Some(state_key) = self.state_key.as_ref() {
            if state_key.trim().is_empty() {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "ciphertext query result item",
                    field: "state_key",
                    reason: "must not be empty when provided".to_string(),
                });
            }
            if !state_key.starts_with('/') {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "ciphertext query result item",
                    field: "state_key",
                    reason: "must start with '/'".to_string(),
                });
            }
        }
        validate_soracloud_digest_hash(
            "ciphertext query result item",
            "state_key_digest",
            self.state_key_digest,
        )?;
        validate_soracloud_digest_hash(
            "ciphertext query result item",
            "ciphertext_commitment",
            self.ciphertext_commitment,
        )?;
        validate_soracloud_digest_hash(
            "ciphertext query result item",
            "governance_tx_hash",
            self.governance_tx_hash,
        )?;
        if let Some(proof) = self.proof.as_ref() {
            proof.validate()?;
        }
        Ok(())
    }
}
/// Deterministic response payload for ciphertext query execution.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct CiphertextQueryResponseV1 {
    /// Schema version; must equal [`CIPHERTEXT_QUERY_RESPONSE_VERSION_V1`].
    pub schema_version: u16,
    /// Canonical hash of the query spec used to produce this response.
    pub query_hash: Hash,
    /// Service queried by this response.
    pub service_name: Name,
    /// Binding queried by this response.
    pub binding_name: Name,
    /// Metadata projection level applied in this response.
    pub metadata_level: CiphertextQueryMetadataLevelV1,
    /// Registry sequence at which this response was materialized.
    pub served_sequence: u64,
    /// Number of results serialized in this response.
    pub result_count: u16,
    /// Whether additional rows existed beyond `result_count`.
    pub truncated: bool,
    /// Result rows.
    #[norito(default)]
    pub results: Vec<CiphertextQueryResultItemV1>,
}
impl CiphertextQueryResponseV1 {
    /// Validate ciphertext query response constraints.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch, result counts diverge,
    /// projection constraints are violated, or any nested result/proof item fails validation.
    pub fn validate(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != CIPHERTEXT_QUERY_RESPONSE_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "ciphertext query response",
                expected: CIPHERTEXT_QUERY_RESPONSE_VERSION_V1,
                found: self.schema_version,
            });
        }
        if usize::from(self.result_count) != self.results.len() {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "ciphertext query response",
                field: "result_count",
                reason: format!(
                    "declared {} results but payload has {} rows",
                    self.result_count,
                    self.results.len()
                ),
            });
        }
        validate_soracloud_digest_hash("ciphertext query response", "query_hash", self.query_hash)?;
        for row in &self.results {
            row.validate()?;
            match self.metadata_level {
                CiphertextQueryMetadataLevelV1::Minimal => {
                    if row.state_key.is_some() {
                        return Err(SoracloudManifestError::InvalidField {
                            manifest: "ciphertext query response",
                            field: "results.state_key",
                            reason: "minimal metadata level must not expose state_key".to_string(),
                        });
                    }
                }
                CiphertextQueryMetadataLevelV1::Standard => {
                    if row.state_key.is_none() {
                        return Err(SoracloudManifestError::InvalidField {
                            manifest: "ciphertext query response",
                            field: "results.state_key",
                            reason: "standard metadata level requires state_key".to_string(),
                        });
                    }
                }
            }
        }
        Ok(())
    }
}
/// Admission bundle coupling container + service manifests for deterministic checks.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(feature = "json", derive(DeriveJsonSerialize, DeriveJsonDeserialize))]
pub struct SoraDeploymentBundleV1 {
    /// Schema version; must equal [`SORA_DEPLOYMENT_BUNDLE_VERSION_V1`].
    pub schema_version: u16,
    /// Container manifest referenced by the service.
    pub container: SoraContainerManifestV1,
    /// Routable service manifest.
    pub service: SoraServiceManifestV1,
}
impl SoraDeploymentBundleV1 {
    /// Compute the canonical hash of the container manifest.
    #[must_use]
    pub fn container_manifest_hash(&self) -> Hash {
        Hash::new(Encode::encode(&self.container))
    }
    /// Compute the canonical hash of the service manifest.
    #[must_use]
    pub fn service_manifest_hash(&self) -> Hash {
        Hash::new(Encode::encode(&self.service))
    }
    /// Validate deterministic admission constraints across container + service manifests.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when schema versions mismatch, internal
    /// manifest validation fails, manifest references are inconsistent, or
    /// capability/binding combinations are invalid.
    pub fn validate_for_admission(&self) -> Result<(), SoracloudManifestError> {
        if self.schema_version != SORA_DEPLOYMENT_BUNDLE_VERSION_V1 {
            return Err(SoracloudManifestError::UnsupportedVersion {
                manifest: "sora deployment bundle",
                expected: SORA_DEPLOYMENT_BUNDLE_VERSION_V1,
                found: self.schema_version,
            });
        }
        self.container.validate()?;
        self.service.validate()?;
        self.validate_container_reference()?;
        self.validate_state_write_requirements()?;
        self.validate_runtime_requirements()?;
        self.validate_public_route_healthcheck_requirement()?;
        self.validate_http_service_quota_class()?;
        Ok(())
    }
    fn validate_container_reference(&self) -> Result<(), SoracloudManifestError> {
        if self.service.container.expected_schema_version != self.container.schema_version {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "sora deployment bundle",
                field: "service.container.expected_schema_version",
                reason: format!(
                    "expected service container schema {} but container manifest declares {}",
                    self.service.container.expected_schema_version, self.container.schema_version
                ),
            });
        }
        let computed_hash = self.container_manifest_hash();
        if self.service.container.manifest_hash != computed_hash {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "sora deployment bundle",
                field: "service.container.manifest_hash",
                reason: format!(
                    "expected {}, found {}",
                    computed_hash, self.service.container.manifest_hash
                ),
            });
        }
        Ok(())
    }
    fn validate_state_write_requirements(&self) -> Result<(), SoracloudManifestError> {
        if self.container.capabilities.allow_state_writes {
            return Ok(());
        }
        for binding in &self.service.state_bindings {
            if binding.mutability != SoraStateMutabilityV1::ReadOnly {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora deployment bundle",
                    field: "container.capabilities.allow_state_writes",
                    reason: format!(
                        "binding `{}` requires mutable writes (`{:?}`)",
                        binding.binding_name, binding.mutability
                    ),
                });
            }
        }
        for handler in &self.service.handlers {
            if matches!(
                handler.class,
                SoraServiceHandlerClassV1::Update | SoraServiceHandlerClassV1::PrivateUpdate
            ) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora deployment bundle",
                    field: "container.capabilities.allow_state_writes",
                    reason: format!(
                        "handler `{}` requires ordered replicated writes",
                        handler.handler_name
                    ),
                });
            }
        }
        Ok(())
    }
    fn validate_runtime_requirements(&self) -> Result<(), SoracloudManifestError> {
        match self.service.execution_plane {
            SoraServiceExecutionPlaneV1::DeterministicService => {
                if !self.container.runtime.is_deterministic() {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "sora deployment bundle",
                        field: "container.runtime",
                        reason: format!(
                            "deterministic services require `Ivm`, found `{:?}`",
                            self.container.runtime
                        ),
                    });
                }
            }
            SoraServiceExecutionPlaneV1::HttpService => {
                if self.container.runtime != SoraContainerRuntimeV1::Inrou {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "sora deployment bundle",
                        field: "container.runtime",
                        reason: format!(
                            "http services require `Inrou`, found `{:?}`",
                            self.container.runtime
                        ),
                    });
                }
                if self.container.lifecycle.healthcheck_path.is_none() {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "sora deployment bundle",
                        field: "container.lifecycle.healthcheck_path",
                        reason: "http services require an explicit healthcheck path".to_string(),
                    });
                }
                let root_volume_count = self
                    .service
                    .lease_volumes
                    .iter()
                    .filter(|volume| volume.attaches_per_replica())
                    .count();
                if root_volume_count != 1 {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "sora deployment bundle",
                        field: "service.lease_volumes",
                        reason:
                            "Inrou runtimes require exactly one `PersistentRootLeaseVolume` binding"
                                .to_string(),
                    });
                }
                if !self
                    .service
                    .lease_volumes
                    .iter()
                    .any(SoraLeaseVolumeBindingV1::attaches_shared_across_replicas)
                {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "sora deployment bundle",
                        field: "service.lease_volumes",
                        reason:
                            "Inrou runtimes require at least one shared `ServiceLeaseVolume` or `ConfidentialLeaseVolume` binding"
                                .to_string(),
                    });
                }
                if self
                    .container
                    .inrou
                    .as_ref()
                    .is_some_and(|inrou| inrou.ssh_authorized_keys.is_empty())
                {
                    return Err(SoracloudManifestError::InvalidField {
                        manifest: "sora deployment bundle",
                        field: "container.inrou.ssh_authorized_keys",
                        reason: "Inrou runtimes require at least one SSH authorized key"
                            .to_string(),
                    });
                }
            }
        }
        Ok(())
    }
    fn validate_public_route_healthcheck_requirement(&self) -> Result<(), SoracloudManifestError> {
        if matches!(
            self.service.route.as_ref().map(|route| route.visibility),
            Some(SoraRouteVisibilityV1::Public)
        ) && self.container.lifecycle.healthcheck_path.is_none()
        {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "sora deployment bundle",
                field: "container.lifecycle.healthcheck_path",
                reason: "public routes require an explicit healthcheck path".to_string(),
            });
        }
        Ok(())
    }
    fn validate_http_service_quota_class(&self) -> Result<(), SoracloudManifestError> {
        if self.service.execution_plane != SoraServiceExecutionPlaneV1::HttpService {
            return Ok(());
        }
        let quota_class = self.service.economics.quota_class.as_str();
        let Some(policy) = http_service_quota_class_policy(quota_class) else {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "sora deployment bundle",
                field: "service.economics.quota_class",
                reason: format!(
                    "unknown hosted-service quota class `{quota_class}`; supported classes: `{SORA_HTTP_SERVICE_QUOTA_CLASS_TAIRA_OPEN}`"
                ),
            });
        };
        if self.service.replicas.get() > policy.max_replicas {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "sora deployment bundle",
                field: "service.replicas",
                reason: format!(
                    "quota class `{quota_class}` allows at most {} replicas",
                    policy.max_replicas
                ),
            });
        }
        let resources = self.container.resources;
        for (field, value, max_value) in [
            (
                "container.resources.cpu_millis",
                u64::from(resources.cpu_millis.get()),
                u64::from(policy.max_cpu_millis),
            ),
            (
                "container.resources.memory_bytes",
                resources.memory_bytes.get(),
                policy.max_memory_bytes,
            ),
            (
                "container.resources.ephemeral_storage_bytes",
                resources.ephemeral_storage_bytes.get(),
                policy.max_ephemeral_storage_bytes,
            ),
            (
                "container.resources.max_open_files",
                u64::from(resources.max_open_files.get()),
                u64::from(policy.max_open_files),
            ),
            (
                "container.resources.max_tasks",
                u64::from(resources.max_tasks.get()),
                u64::from(policy.max_tasks),
            ),
        ] {
            if value > max_value {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora deployment bundle",
                    field,
                    reason: format!(
                        "quota class `{quota_class}` allows at most {max_value}, found {value}"
                    ),
                });
            }
        }
        let total_lease_volume_bytes = self
            .service
            .lease_volumes
            .iter()
            .fold(0_u64, |acc, volume| {
                acc.saturating_add(volume.max_total_bytes.get())
            });
        if total_lease_volume_bytes > policy.max_total_lease_volume_bytes {
            return Err(SoracloudManifestError::InvalidField {
                manifest: "sora deployment bundle",
                field: "service.lease_volumes",
                reason: format!(
                    "quota class `{quota_class}` allows at most {} total lease-backed bytes, found {total_lease_volume_bytes}",
                    policy.max_total_lease_volume_bytes
                ),
            });
        }
        Ok(())
    }
    /// Validate that the effective service-scoped config and secret maps satisfy this revision.
    ///
    /// # Errors
    /// Returns [`SoracloudManifestError`] when a required config or secret is absent.
    pub fn validate_required_service_materials(
        &self,
        service_configs: &BTreeMap<String, SoraServiceConfigEntryV1>,
        service_secrets: &BTreeMap<String, SoraServiceSecretEntryV1>,
    ) -> Result<(), SoracloudManifestError> {
        for config_name in &self.container.required_config_names {
            if !service_configs.contains_key(config_name) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora deployment bundle",
                    field: "container.required_config_names",
                    reason: format!(
                        "required service config `{config_name}` is missing from the effective deployment materials"
                    ),
                });
            }
        }
        for secret_name in &self.container.required_secret_names {
            if !service_secrets.contains_key(secret_name) {
                return Err(SoracloudManifestError::InvalidField {
                    manifest: "sora deployment bundle",
                    field: "container.required_secret_names",
                    reason: format!(
                        "required service secret `{secret_name}` is missing from the effective deployment materials"
                    ),
                });
            }
        }
        Ok(())
    }
}
