//! Canonical MAIN aggregate prover and verifier.
//!
//! The implementation shares the verifier-fixed layouts, transcript helpers,
//! and concrete trace providers owned by the parent STARK module.
// This is a private continuation of the parent module's fixed protocol
// vocabulary; it does not define an independent extension surface.
use super::*;
#[cfg(any(test, feature = "privacy-release-evidence"))]
use rayon::prelude::*;
#[derive(Clone, Copy)]
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) enum MainTraceColumnKindV1 {
    Base,
    Aux,
}
/// Exact ordered ownership of the six authenticated masked polynomial sets.
///
/// Each group retains only the coefficients of the polynomials that were streamed into its
/// commitment. The native witness and a common-domain LDE matrix are neither retained nor
/// reconstructed after commitment. This keeps the phase transition binding while avoiding
/// terabyte-scale encrypted scratch for the full MAIN profile.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) struct MainTracePolynomialSetV1 {
    groups: [aggregate::MaskedTracePolynomialSetV1; FULL_PROFILE_TRACE_GROUPS_V1],
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
impl MainTracePolynomialSetV1 {
    pub(super) fn from_ordered_v1(
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
        groups: Vec<aggregate::MaskedTracePolynomialSetV1>,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        let groups = groups
            .try_into()
            .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?;
        let set = Self { groups };
        set.validate_v1(layout, kind)?;
        Ok(set)
    }
    fn validate_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
    ) -> Result<(), ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        for (polynomials, group) in self.groups.iter().zip(&layout.trace_groups) {
            let width = match kind {
                MainTraceColumnKindV1::Base => group.base_width,
                MainTraceColumnKindV1::Aux => group.aux_width,
            };
            if polynomials.width() != width
                || polynomials.native_trace_log2() != group.native_trace_log2
                || polynomials.commitment_lde_log2() != layout.common_lde_log2
            {
                return Err(ZkX509StarkErrorV1::TranscriptMismatch);
            }
        }
        Ok(())
    }
    fn group_v1(
        &self,
        layout: &AggregateProofLayoutV1,
        kind: MainTraceColumnKindV1,
        group_index: usize,
    ) -> Result<&aggregate::MaskedTracePolynomialSetV1, ZkX509StarkErrorV1> {
        self.validate_v1(layout, kind)?;
        self.groups
            .get(group_index)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
    }
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn registered_main_group_column_v1(
    layout: &AggregateProofLayoutV1,
    group_index: usize,
    kind: MainTraceColumnKindV1,
    column_index: usize,
) -> Result<(RegisteredSegmentLayoutV1, usize), ZkX509StarkErrorV1> {
    layout.validate_exact_full_profile_registration_v1()?;
    let group = layout
        .trace_groups
        .get(group_index)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let width = match kind {
        MainTraceColumnKindV1::Base => group.base_width,
        MainTraceColumnKindV1::Aux => group.aux_width,
    };
    if column_index >= width {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let mut matched = None;
    for registration in layout
        .registered_segments
        .iter()
        .copied()
        .filter(|registration| registration.trace_group == group_index)
    {
        let (start, end) = match kind {
            MainTraceColumnKindV1::Base => (registration.base_start, registration.base_end()?),
            MainTraceColumnKindV1::Aux => (registration.aux_start, registration.aux_end()?),
        };
        if (start..end).contains(&column_index)
            && matched
                .replace((registration, column_index - start))
                .is_some()
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
    }
    matched.ok_or(ZkX509StarkErrorV1::ProfileMismatch)
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn commit_main_trace_group_v1<R: TryRngCore>(
    layout: &AggregateProofLayoutV1,
    group_index: usize,
    kind: MainTraceColumnKindV1,
    source: &mut dyn MainTraceGroupSourceV1,
    rng: &mut R,
) -> Result<
    (
        aggregate::StreamingRowCommitmentResultV1,
        aggregate::MaskedTracePolynomialSetV1,
    ),
    ZkX509StarkErrorV1,
> {
    layout.validate_exact_full_profile_registration_v1()?;
    let group = *layout
        .trace_groups
        .get(group_index)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if MAIN_BASE_COMMITMENT_NATIVE_LOGS_V1
        .get(group_index)
        .copied()
        != Some(group.native_trace_log2)
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let (leaf_domain, node_domain, width) = match kind {
        MainTraceColumnKindV1::Base => (BASE_LEAF_DOMAIN, BASE_NODE_DOMAIN, group.base_width),
        MainTraceColumnKindV1::Aux => (AUX_LEAF_DOMAIN, AUX_NODE_DOMAIN, group.aux_width),
    };
    let mut source_error = None;
    let result = aggregate::commit_masked_trace_polynomial_columns_v1(
        leaf_domain,
        node_domain,
        group_index,
        group.native_trace_log2,
        layout.common_lde_log2,
        width,
        MASK_DEGREE,
        &[],
        rng,
        |column_index| {
            let (registration, local_column) =
                registered_main_group_column_v1(layout, group_index, kind, column_index)
                    .map_err(|_| AggregateStarkErrorV1::InvalidLayout)?;
            let column = match kind {
                MainTraceColumnKindV1::Base => {
                    source.native_base_column_v1(registration, local_column)
                }
                MainTraceColumnKindV1::Aux => {
                    source.native_aux_column_v1(registration, local_column)
                }
            };
            let column = match column {
                Ok(column) => column,
                Err(error) => {
                    let aggregate_error = if matches!(&error, ZkX509StarkErrorV1::AllocationFailure)
                    {
                        AggregateStarkErrorV1::AllocationFailure
                    } else {
                        AggregateStarkErrorV1::InvalidLayout
                    };
                    source_error = Some(error);
                    return Err(aggregate_error);
                }
            };
            if column.len() != registration.segment.trace_size()
                || column.iter().any(|value| F::canonical(value.0).is_none())
            {
                return Err(AggregateStarkErrorV1::InvalidLayout);
            }
            Ok(column.into_vec_v1())
        },
    );
    match (result, source_error) {
        (Ok(committed), None) => Ok(committed),
        (Err(_), Some(error)) => Err(error),
        (Err(error), None) => Err(map_aggregate_error_v1(error)),
        (Ok(_), Some(_)) => Err(ZkX509StarkErrorV1::InternalInvariant),
    }
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn main_trace_group_root_v1(
    kind: MainTraceColumnKindV1,
    commitment: &aggregate::StreamingRowCommitmentResultV1,
) -> TraceGroupProofV1 {
    match kind {
        MainTraceColumnKindV1::Base => TraceGroupProofV1 {
            base_root: commitment.commitment.root,
            aux_root: [0_u8; 32],
            base_frontier: Vec::new(),
            aux_frontier: Vec::new(),
        },
        MainTraceColumnKindV1::Aux => TraceGroupProofV1 {
            base_root: [0_u8; 32],
            aux_root: commitment.commitment.root,
            base_frontier: Vec::new(),
            aux_frontier: Vec::new(),
        },
    }
}
fn map_credential_pre_aux_error_v1(
    error: super::super::credential_pre_aux::ZkX509CredentialPreAuxErrorV1,
) -> ZkX509StarkErrorV1 {
    use super::super::credential_pre_aux::ZkX509CredentialPreAuxErrorV1 as Error;
    match error {
        Error::Resource => ZkX509StarkErrorV1::AllocationFailure,
        Error::Transcript | Error::Challenge => ZkX509StarkErrorV1::TranscriptMismatch,
    }
}
/// MAIN state after exactly six ordered base commitments and before X5B1.
///
/// The type owns every challenge-independent child which must cross the joint credential phase. It
/// exposes only a consuming transition accepting the opaque outer credential binding; raw challenge
/// families and auxiliary commitment APIs are intentionally absent.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) struct ZkX509MainAwaitingCredentialBindingV1<'a> {
    layout: AggregateProofLayoutV1,
    statement: &'a IrohaZkX509StarkP256StatementV1,
    assembly: &'a ZkX509MainTraceAssemblyV1,
    public: ZkX509CredentialPublicBindingV1,
    p256: P256MainBaseSourceV1,
    sha: [ZkX509ShaBatchSegmentBaseSourceV1<'a>; ZK_X509_SHA_SEGMENT_COUNT_V1],
    projection: MainProjectionTraceGroupSourceV1<'a>,
    io: MainIoTraceGroupSourceV1<'a>,
    trace_groups: Vec<TraceGroupProofV1>,
    base_polynomials: MainTracePolynomialSetV1,
    transcript: TransparentTranscriptV1,
    base_transcript_state: [u8; 32],
    pre_aux: ZkX509CredentialMainPreAuxV1,
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
impl ZkX509MainAwaitingCredentialBindingV1<'_> {
    fn validate_v1(&self) -> Result<(), ZkX509StarkErrorV1> {
        self.layout.validate_exact_full_profile_registration_v1()?;
        validate_zk_x509_main_verifier_profile_v1(self.assembly.verifier_profile)?;
        self.base_polynomials
            .validate_v1(&self.layout, MainTraceColumnKindV1::Base)?;
        if self.public.consensus_context_digest == [0_u8; 32]
            || self.trace_groups.len() != FULL_PROFILE_TRACE_GROUPS_V1
            || self.transcript.state() != self.base_transcript_state
            || self
                .trace_groups
                .iter()
                .any(|group| group.base_root == [0_u8; 32] || group.aux_root != [0_u8; 32])
            || self.projection.aux.is_some()
            || self.io.bind_attempted
            || self.io.aux_columns.is_some()
            || self.io.post_base.is_some()
        {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }
        validate_p256_main_registration_order_v1(&self.p256.canonical_registrations_v1()?)
            .map_err(|_| ZkX509StarkErrorV1::P256Witness)
    }
}
/// Composition-ready MAIN state after the sole X5B1 transition.
///
/// Both trace-mask sets, all six base/aux roots, the exact terminal claims, and per-registration
/// composition coefficients are retained together. A future composition/DEEP/FRI continuation
/// cannot resample challenges or return to either earlier phase.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) struct ZkX509MainCompositionPhaseV1<'a> {
    layout: AggregateProofLayoutV1,
    statement: &'a IrohaZkX509StarkP256StatementV1,
    assembly: &'a ZkX509MainTraceAssemblyV1,
    public: ZkX509CredentialPublicBindingV1,
    log19: MainLog19BoundTraceGroupSourceV1<'a>,
    projection: MainProjectionTraceGroupSourceV1<'a>,
    io: MainIoTraceGroupSourceV1<'a>,
    trace_groups: Vec<TraceGroupProofV1>,
    base_polynomials: MainTracePolynomialSetV1,
    aux_polynomials: MainTracePolynomialSetV1,
    terminal_claims: ZkX509MainTerminalClaimsV1,
    alphas: Vec<Vec<Vec<E>>>,
    transcript: TransparentTranscriptV1,
    composition_transcript_state: [u8; 32],
    binding: ZkX509CredentialPreAuxBindingV1,
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
impl ZkX509MainCompositionPhaseV1<'_> {
    fn validate_v1(&self) -> Result<(), ZkX509StarkErrorV1> {
        self.layout.validate_exact_full_profile_registration_v1()?;
        validate_zk_x509_main_verifier_profile_v1(self.assembly.verifier_profile)?;
        self.base_polynomials
            .validate_v1(&self.layout, MainTraceColumnKindV1::Base)?;
        self.aux_polynomials
            .validate_v1(&self.layout, MainTraceColumnKindV1::Aux)?;
        if self.public.consensus_context_digest == [0_u8; 32]
            || self.trace_groups.len() != FULL_PROFILE_TRACE_GROUPS_V1
            || self.terminal_claims != self.log19.terminal_claims_v1()
            || self.log19.post_base != self.binding.main_post_base()
            || self.transcript.state() != self.composition_transcript_state
            || self
                .trace_groups
                .iter()
                .any(|group| group.base_root == [0_u8; 32] || group.aux_root == [0_u8; 32])
            || self.alphas.len() != self.layout.registered_segments.len()
            || self
                .alphas
                .iter()
                .zip(&self.layout.registered_segments)
                .any(|(lanes, registration)| {
                    lanes.len() != SECURITY_LANES
                        || lanes
                            .iter()
                            .any(|lane| lane.len() != registration.segment.constraint_count)
                })
        {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }
        self.io.validate_bound_phase_v1()?;
        if self.projection.aux.is_none() {
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }
        Ok(())
    }
    fn prover_constraint_providers_v1(
        &self,
    ) -> Result<Vec<MainProverConstraintProviderV1<'_, '_>>, ZkX509StarkErrorV1> {
        self.validate_v1()?;
        let post_base = self.binding.main_post_base();
        let providers = vec![
            MainProverConstraintProviderV1::Log5(
                MainP256Log5ProverConstraintSourceV1::for_main_v1(&self.layout, &self.log19.p256)?,
            ),
            MainProverConstraintProviderV1::P256Scalar(
                MainP256ScalarProverConstraintSourceV1::for_main_v1(
                    &self.layout,
                    &self.log19.p256,
                )?,
            ),
            MainProverConstraintProviderV1::Projection(
                MainProjectionProverConstraintSourceV1::for_main_v1(
                    &self.layout,
                    self.statement,
                    post_base,
                )?,
            ),
            MainProverConstraintProviderV1::Log16(
                MainP256Log16ProverConstraintSourceV1::for_main_v1(&self.layout, &self.log19.p256)?,
            ),
            MainProverConstraintProviderV1::Io(MainIoProverConstraintSourceV1::for_main_v1(
                &self.layout,
                self.statement,
                &self.assembly.io,
                post_base,
            )?),
            MainProverConstraintProviderV1::Log19(MainLog19ProverConstraintSourceV1::for_main_v1(
                &self.layout,
                &self.log19,
            )?),
        ];
        if providers.len() != FULL_PROFILE_TRACE_GROUPS_V1
            || providers
                .iter()
                .zip(&self.layout.trace_groups)
                .any(|(provider, group)| provider.native_trace_log2_v1() != group.native_trace_log2)
        {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        Ok(providers)
    }
    fn composition_material_v1(&self) -> Result<RetainedCompositionMaterialV1, ZkX509StarkErrorV1> {
        let providers = self.prover_constraint_providers_v1()?;
        main_composition_material_from_polynomials_v1(
            &self.layout,
            &self.base_polynomials,
            &self.aux_polynomials,
            &providers,
            &self.alphas,
        )
    }
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn record_main_group_commitment_v1(
    group_index: usize,
    kind: MainTraceColumnKindV1,
    commitment: &aggregate::StreamingRowCommitmentResultV1,
    trace_groups: &mut Vec<TraceGroupProofV1>,
) -> Result<(), ZkX509StarkErrorV1> {
    if commitment.commitment.root == [0_u8; 32] {
        return Err(ZkX509StarkErrorV1::TranscriptMismatch);
    }
    match kind {
        MainTraceColumnKindV1::Base => {
            if group_index != trace_groups.len() {
                return Err(ZkX509StarkErrorV1::TranscriptMismatch);
            }
            trace_groups.push(main_trace_group_root_v1(kind, commitment));
        }
        MainTraceColumnKindV1::Aux => {
            let expected_group = trace_groups
                .iter()
                .position(|group| group.aux_root == [0_u8; 32])
                .unwrap_or(trace_groups.len());
            if trace_groups.len() != FULL_PROFILE_TRACE_GROUPS_V1 || group_index != expected_group {
                return Err(ZkX509StarkErrorV1::TranscriptMismatch);
            }
            let group = trace_groups
                .get_mut(group_index)
                .ok_or(ZkX509StarkErrorV1::TranscriptMismatch)?;
            if group.base_root == [0_u8; 32] || group.aux_root != [0_u8; 32] {
                return Err(ZkX509StarkErrorV1::TranscriptMismatch);
            }
            group.aux_root = commitment.commitment.root;
        }
    }
    Ok(())
}
/// Commit exactly the six canonical MAIN base groups and yield the sole outer
/// credential assembly hook.
///
/// This is phase one only. The returned state cannot commit auxiliary columns
/// until the credential layer combines its fixed six roots with the compact-CA
/// root and supplies the resulting opaque 272-challenge X5B1 binding.
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(crate) fn commit_zk_x509_main_base_phase_v1_with_rng<'a, R: TryRngCore>(
    statement: &'a IrohaZkX509StarkP256StatementV1,
    assembly: &'a ZkX509MainTraceAssemblyV1,
    public: ZkX509CredentialPublicBindingV1,
    rng: &mut R,
) -> Result<
    (
        ZkX509MainAwaitingCredentialBindingV1<'a>,
        ZkX509CredentialMainPreAuxV1,
    ),
    ZkX509StarkErrorV1,
> {
    validate_zk_x509_main_verifier_profile_v1(assembly.verifier_profile)?;
    if public.consensus_context_digest == [0_u8; 32] {
        return Err(ZkX509StarkErrorV1::InvalidStatement);
    }
    if ca_accumulator_stark_public_v1(&assembly.ca_accumulator_trace, &assembly.sha_schedule)?
        != public.ca_public_v1()
    {
        return Err(ZkX509StarkErrorV1::WitnessStatementMismatch);
    }
    let layout = AggregateProofLayoutV1::for_full_profile_v1()?;
    let p256 = P256MainBaseSourceV1::new_v1(assembly)?;
    let sha = main_log19_sha_base_sources_v1(&assembly.sha_schedule, &assembly.sha_witnesses)?;
    let mut projection = MainProjectionTraceGroupSourceV1::for_main_v1(
        &layout,
        statement,
        &assembly.projection_trace,
    )?;
    let mut io = MainIoTraceGroupSourceV1::for_main_v1(&layout, statement, &assembly.io)?;
    let mut session = ZkX509MainBaseCommitmentSessionV1::new_v1(
        &layout,
        public.consensus_context_digest,
        assembly.verifier_profile,
    )?;
    let mut transcript =
        new_main_transcript_v1(&public.consensus_context_digest, assembly.verifier_profile)?;
    absorb_aggregate_layout_v1(&mut transcript, MAIN_LAYOUT_DOMAIN_V1, &layout)?;
    let mut trace_groups = Vec::new();
    let mut base_polynomials = Vec::new();
    trace_groups
        .try_reserve_exact(FULL_PROFILE_TRACE_GROUPS_V1)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    base_polynomials
        .try_reserve_exact(FULL_PROFILE_TRACE_GROUPS_V1)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    {
        let mut source = MainP256Log5TraceGroupSourceV1::for_base_v1(&layout, &p256)?;
        let (commitment, polynomials) =
            commit_main_trace_group_v1(&layout, 0, MainTraceColumnKindV1::Base, &mut source, rng)?;
        session.accept_streaming_base_commitment_v1(0, &commitment)?;
        record_main_group_commitment_v1(
            0,
            MainTraceColumnKindV1::Base,
            &commitment,
            &mut trace_groups,
        )?;
        base_polynomials.push(polynomials);
    }
    {
        let mut source = MainP256ScalarTraceGroupSourceV1::for_base_v1(&layout, &p256)?;
        let (commitment, polynomials) =
            commit_main_trace_group_v1(&layout, 1, MainTraceColumnKindV1::Base, &mut source, rng)?;
        session.accept_streaming_base_commitment_v1(1, &commitment)?;
        record_main_group_commitment_v1(
            1,
            MainTraceColumnKindV1::Base,
            &commitment,
            &mut trace_groups,
        )?;
        base_polynomials.push(polynomials);
    }
    {
        let (commitment, polynomials) = commit_main_trace_group_v1(
            &layout,
            2,
            MainTraceColumnKindV1::Base,
            &mut projection,
            rng,
        )?;
        session.accept_streaming_base_commitment_v1(2, &commitment)?;
        record_main_group_commitment_v1(
            2,
            MainTraceColumnKindV1::Base,
            &commitment,
            &mut trace_groups,
        )?;
        base_polynomials.push(polynomials);
    }
    {
        let mut source = MainP256Log16TraceGroupSourceV1::for_base_v1(&layout, &p256)?;
        let (commitment, polynomials) =
            commit_main_trace_group_v1(&layout, 3, MainTraceColumnKindV1::Base, &mut source, rng)?;
        session.accept_streaming_base_commitment_v1(3, &commitment)?;
        record_main_group_commitment_v1(
            3,
            MainTraceColumnKindV1::Base,
            &commitment,
            &mut trace_groups,
        )?;
        base_polynomials.push(polynomials);
    }
    {
        let (commitment, polynomials) =
            commit_main_trace_group_v1(&layout, 4, MainTraceColumnKindV1::Base, &mut io, rng)?;
        session.accept_streaming_base_commitment_v1(4, &commitment)?;
        record_main_group_commitment_v1(
            4,
            MainTraceColumnKindV1::Base,
            &commitment,
            &mut trace_groups,
        )?;
        base_polynomials.push(polynomials);
    }
    {
        let mut source =
            MainLog19BaseTraceGroupSourceV1::for_main_v1(&layout, assembly, &sha, &p256)?;
        let (commitment, polynomials) =
            commit_main_trace_group_v1(&layout, 5, MainTraceColumnKindV1::Base, &mut source, rng)?;
        session.accept_streaming_base_commitment_v1(5, &commitment)?;
        record_main_group_commitment_v1(
            5,
            MainTraceColumnKindV1::Base,
            &commitment,
            &mut trace_groups,
        )?;
        base_polynomials.push(polynomials);
    }
    let base_polynomials = MainTracePolynomialSetV1::from_ordered_v1(
        &layout,
        MainTraceColumnKindV1::Base,
        base_polynomials,
    )?;
    aggregate::absorb_base_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &trace_groups)
        .map_err(map_aggregate_error_v1)?;
    let pre_aux = session.finish_pre_aux_v1()?;
    let base_transcript_state = transcript.state();
    let phase = ZkX509MainAwaitingCredentialBindingV1 {
        layout,
        statement,
        assembly,
        public,
        p256,
        sha,
        projection,
        io,
        trace_groups,
        base_polynomials,
        transcript,
        base_transcript_state,
        pre_aux,
    };
    phase.validate_v1()?;
    Ok((phase, pre_aux))
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
impl<'a> ZkX509MainAwaitingCredentialBindingV1<'a> {
    /// Consume phase one, bind all challenge-dependent children with one X5B1
    /// capability, commit exactly six auxiliary groups, absorb terminal
    /// claims, and sample the complete per-registration alpha schedule.
    pub(crate) fn bind_credential_pre_aux_v1_with_rng<R: TryRngCore>(
        self,
        binding: ZkX509CredentialPreAuxBindingV1,
        rng: &mut R,
    ) -> Result<ZkX509MainCompositionPhaseV1<'a>, ZkX509StarkErrorV1> {
        self.validate_v1()?;
        if !binding.matches_main_pre_aux_v1(self.pre_aux) {
            // Reject substitution before transcript absorption or any
            // challenge-dependent child transition.
            return Err(ZkX509StarkErrorV1::TranscriptMismatch);
        }
        let ZkX509MainAwaitingCredentialBindingV1 {
            layout,
            statement,
            assembly,
            public,
            p256,
            sha,
            mut projection,
            mut io,
            mut trace_groups,
            base_polynomials,
            mut transcript,
            base_transcript_state: _,
            pre_aux: _,
        } = self;
        absorb_zk_x509_credential_pre_aux_binding_v1(&mut transcript, binding)
            .map_err(map_credential_pre_aux_error_v1)?;
        let post_base = binding.main_post_base();
        projection.bind_challenges_v1(post_base)?;
        io.bind_challenges_v1(post_base)?;
        let mut log19 = MainLog19BoundTraceGroupSourceV1::bind_from_phase_v1(
            &layout, assembly, sha, p256, binding,
        )?;
        let mut aux_polynomials = Vec::new();
        aux_polynomials
            .try_reserve_exact(FULL_PROFILE_TRACE_GROUPS_V1)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        {
            let mut source = MainP256Log5TraceGroupSourceV1::for_bound_v1(&layout, &log19.p256)?;
            let (commitment, polynomials) = commit_main_trace_group_v1(
                &layout,
                0,
                MainTraceColumnKindV1::Aux,
                &mut source,
                rng,
            )?;
            record_main_group_commitment_v1(
                0,
                MainTraceColumnKindV1::Aux,
                &commitment,
                &mut trace_groups,
            )?;
            aux_polynomials.push(polynomials);
        }
        {
            let mut source = MainP256ScalarTraceGroupSourceV1::for_bound_v1(&layout, &log19.p256)?;
            let (commitment, polynomials) = commit_main_trace_group_v1(
                &layout,
                1,
                MainTraceColumnKindV1::Aux,
                &mut source,
                rng,
            )?;
            record_main_group_commitment_v1(
                1,
                MainTraceColumnKindV1::Aux,
                &commitment,
                &mut trace_groups,
            )?;
            aux_polynomials.push(polynomials);
        }
        {
            let (commitment, polynomials) = commit_main_trace_group_v1(
                &layout,
                2,
                MainTraceColumnKindV1::Aux,
                &mut projection,
                rng,
            )?;
            record_main_group_commitment_v1(
                2,
                MainTraceColumnKindV1::Aux,
                &commitment,
                &mut trace_groups,
            )?;
            aux_polynomials.push(polynomials);
        }
        {
            let mut source = MainP256Log16TraceGroupSourceV1::for_bound_v1(&layout, &log19.p256)?;
            let (commitment, polynomials) = commit_main_trace_group_v1(
                &layout,
                3,
                MainTraceColumnKindV1::Aux,
                &mut source,
                rng,
            )?;
            record_main_group_commitment_v1(
                3,
                MainTraceColumnKindV1::Aux,
                &commitment,
                &mut trace_groups,
            )?;
            aux_polynomials.push(polynomials);
        }
        {
            let (commitment, polynomials) =
                commit_main_trace_group_v1(&layout, 4, MainTraceColumnKindV1::Aux, &mut io, rng)?;
            record_main_group_commitment_v1(
                4,
                MainTraceColumnKindV1::Aux,
                &commitment,
                &mut trace_groups,
            )?;
            aux_polynomials.push(polynomials);
        }
        {
            let (commitment, polynomials) = commit_main_trace_group_v1(
                &layout,
                5,
                MainTraceColumnKindV1::Aux,
                &mut log19,
                rng,
            )?;
            record_main_group_commitment_v1(
                5,
                MainTraceColumnKindV1::Aux,
                &commitment,
                &mut trace_groups,
            )?;
            aux_polynomials.push(polynomials);
        }
        let aux_polynomials = MainTracePolynomialSetV1::from_ordered_v1(
            &layout,
            MainTraceColumnKindV1::Aux,
            aux_polynomials,
        )?;
        aggregate::absorb_aux_roots_v1(&mut transcript, AGGREGATE_DOMAINS_V1, &trace_groups)
            .map_err(map_aggregate_error_v1)?;
        let terminal_claims = log19.terminal_claims_v1();
        absorb_zk_x509_main_terminal_claims_v1(&mut transcript, terminal_claims)?;
        let alphas = derive_constraint_alphas_v1(&mut transcript, &layout)?;
        let composition_transcript_state = transcript.state();
        let phase = ZkX509MainCompositionPhaseV1 {
            layout,
            statement,
            assembly,
            public,
            log19,
            projection,
            io,
            trace_groups,
            base_polynomials,
            aux_polynomials,
            terminal_claims,
            alphas,
            transcript,
            composition_transcript_state,
            binding,
        };
        phase.validate_v1()?;
        Ok(phase)
    }
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
impl ZkX509MainCompositionPhaseV1<'_> {
    /// Consume the X5B1-bound phase and construct the canonical X5M1 proof.
    ///
    /// Every trace opening and DEEP value is replayed only from the retained
    /// masked polynomial coefficients committed by the two earlier phases.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn finish_v1_with_rng<R: TryRngCore>(
        mut self,
        rng: &mut R,
    ) -> Result<Vec<u8>, ZkX509StarkErrorV1> {
        self.validate_v1()?;
        let composition_material = self.composition_material_v1()?;
        let compositions = &composition_material.evaluations;
        let shared_layout = self.layout.as_shared()?;
        let mut composition_roots = Vec::new();
        composition_roots
            .try_reserve_exact(SECURITY_LANES)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for (lane, composition) in compositions.iter().enumerate() {
            composition_roots.push(
                aggregate::streaming_composition_commitment_v1(
                    AGGREGATE_DOMAINS_V1,
                    lane,
                    composition,
                    &[],
                )
                .map_err(map_aggregate_error_v1)?
                .root,
            );
        }
        aggregate::absorb_composition_roots_v1(
            &mut self.transcript,
            AGGREGATE_PARAMETERS_V1,
            AGGREGATE_DOMAINS_V1,
            &composition_roots,
        )
        .map_err(map_aggregate_error_v1)?;
        let fri_masks =
            aggregate::build_fri_mask_oracles_v1(AGGREGATE_PARAMETERS_V1, &shared_layout, rng)
                .map_err(map_aggregate_error_v1)?;
        let fri_mask_roots = fri_masks
            .iter()
            .map(|mask| mask.tree.root())
            .collect::<Vec<_>>();
        aggregate::absorb_fri_mask_roots_v1(
            &mut self.transcript,
            AGGREGATE_PARAMETERS_V1,
            &fri_mask_roots,
        )
        .map_err(map_aggregate_error_v1)?;
        let deep_point = aggregate::derive_deep_point_v1(
            &mut self.transcript,
            AGGREGATE_PARAMETERS_V1,
            &shared_layout,
        )
        .map_err(map_aggregate_error_v1)?;
        let mut deep_trace_groups = Vec::new();
        deep_trace_groups
            .try_reserve_exact(FULL_PROFILE_TRACE_GROUPS_V1)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for group_index in 0..FULL_PROFILE_TRACE_GROUPS_V1 {
            let base = self.base_polynomials.group_v1(
                &self.layout,
                MainTraceColumnKindV1::Base,
                group_index,
            )?;
            let aux = self.aux_polynomials.group_v1(
                &self.layout,
                MainTraceColumnKindV1::Aux,
                group_index,
            )?;
            let (base_current, base_next) =
                aggregate::evaluate_masked_trace_polynomial_columns_at_deep_v1(base, deep_point)
                    .map_err(map_aggregate_error_v1)?;
            let (aux_current, aux_next) =
                aggregate::evaluate_masked_trace_polynomial_columns_at_deep_v1(aux, deep_point)
                    .map_err(map_aggregate_error_v1)?;
            deep_trace_groups.push(aggregate::AggregateDeepTraceGroupOpeningV1 {
                base_current: fp4_values_to_wire_v1(base_current),
                base_next: fp4_values_to_wire_v1(base_next),
                aux_current: fp4_values_to_wire_v1(aux_current),
                aux_next: fp4_values_to_wire_v1(aux_next),
            });
        }
        let deep_composition_values = evaluate_retained_composition_coefficients_at_deep_v1(
            &composition_material.coefficient_chunks,
            deep_point,
        )?;
        let deep = aggregate::AggregateDeepProofV1 {
            trace_groups: deep_trace_groups,
            composition_values: deep_composition_values
                .into_iter()
                .map(fp4_values_to_wire_v1)
                .collect(),
        };
        aggregate::absorb_deep_openings_v1(
            &mut self.transcript,
            &deep,
            AGGREGATE_PARAMETERS_V1,
            &shared_layout,
        )
        .map_err(map_aggregate_error_v1)?;
        let (canonical_deep_traces, canonical_deep_compositions) =
            canonical_deep_values_v1(&deep, &self.layout)?;
        let mixes = derive_fri_mixes_v1(&mut self.transcript, &self.layout)?;
        let mut fri_bases = main_fri_bases_from_polynomials_v1(
            &self.layout,
            &self.base_polynomials,
            &self.aux_polynomials,
            &composition_material.coefficient_chunks,
            &mixes,
            deep_point,
            &canonical_deep_traces,
            &canonical_deep_compositions,
        )?;
        for (base, mask) in fri_bases.iter_mut().zip(&fri_masks) {
            aggregate::add_fri_mask_oracle_v1(base, mask).map_err(map_aggregate_error_v1)?;
        }
        let mut fri_materials = Vec::new();
        fri_materials
            .try_reserve_exact(SECURITY_LANES)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for lane in 0..SECURITY_LANES {
            let base_values = core::mem::take(
                fri_bases
                    .get_mut(lane)
                    .ok_or(ZkX509StarkErrorV1::InternalInvariant)?,
            );
            fri_materials.push(
                aggregate::build_streaming_fri_lane_v1(
                    AGGREGATE_PARAMETERS_V1,
                    AGGREGATE_DOMAINS_V1,
                    &shared_layout,
                    lane,
                    base_values,
                    &mut self.transcript,
                )
                .map_err(map_aggregate_error_v1)?,
            );
        }
        fri_bases = main_fri_bases_from_polynomials_v1(
            &self.layout,
            &self.base_polynomials,
            &self.aux_polynomials,
            &composition_material.coefficient_chunks,
            &mixes,
            deep_point,
            &canonical_deep_traces,
            &canonical_deep_compositions,
        )?;
        for (base, mask) in fri_bases.iter_mut().zip(&fri_masks) {
            aggregate::add_fri_mask_oracle_v1(base, mask).map_err(map_aggregate_error_v1)?;
        }
        let grinding_state = self.transcript.state();
        let grinding_nonce = grind_nonce_v1(&grinding_state, ZK_X509_GRINDING_BITS_V1)
            .map_err(map_transparent_error_v1)?;
        absorb_grinding_nonce_v1(&mut self.transcript, grinding_nonce)?;
        let query_indices = query_indices_v1(&self.transcript, &self.layout)?;
        let query_skeleton = query_indices
            .iter()
            .map(|index| {
                Ok(aggregate::AggregateQueryProofV1 {
                    index: u32::try_from(*index)
                        .map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
                    trace_groups: Vec::new(),
                    composition_values: Vec::new(),
                    fri_mask_values: Vec::new(),
                    fri_lanes: Vec::new(),
                })
            })
            .collect::<Result<Vec<_>, ZkX509StarkErrorV1>>()?;
        let mut base_openings = Vec::new();
        let mut aux_openings = Vec::new();
        base_openings
            .try_reserve_exact(FULL_PROFILE_TRACE_GROUPS_V1)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        aux_openings
            .try_reserve_exact(FULL_PROFILE_TRACE_GROUPS_V1)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for group_index in 0..FULL_PROFILE_TRACE_GROUPS_V1 {
            let opening_indices = aggregate::trace_group_opening_indices_v1(
                &query_skeleton,
                &shared_layout,
                group_index,
            )
            .map_err(map_aggregate_error_v1)?;
            let base = aggregate::replay_masked_trace_polynomial_columns_v1(
                BASE_LEAF_DOMAIN,
                BASE_NODE_DOMAIN,
                group_index,
                self.base_polynomials.group_v1(
                    &self.layout,
                    MainTraceColumnKindV1::Base,
                    group_index,
                )?,
                &opening_indices,
            )
            .map_err(map_aggregate_error_v1)?;
            let aux = aggregate::replay_masked_trace_polynomial_columns_v1(
                AUX_LEAF_DOMAIN,
                AUX_NODE_DOMAIN,
                group_index,
                self.aux_polynomials.group_v1(
                    &self.layout,
                    MainTraceColumnKindV1::Aux,
                    group_index,
                )?,
                &opening_indices,
            )
            .map_err(map_aggregate_error_v1)?;
            let trace_group = self
                .trace_groups
                .get_mut(group_index)
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
            if base.commitment.root != trace_group.base_root
                || aux.commitment.root != trace_group.aux_root
            {
                return Err(ZkX509StarkErrorV1::InternalInvariant);
            }
            trace_group.base_frontier = base.commitment.frontier.clone();
            trace_group.aux_frontier = aux.commitment.frontier.clone();
            base_openings.push(base);
            aux_openings.push(aux);
        }
        let composition_opening_indices =
            aggregate::composition_opening_indices_v1(&query_skeleton, &shared_layout)
                .map_err(map_aggregate_error_v1)?;
        let mut composition_frontiers = Vec::new();
        composition_frontiers
            .try_reserve_exact(SECURITY_LANES)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for (lane, composition) in compositions.iter().enumerate() {
            let commitment = aggregate::streaming_composition_commitment_v1(
                AGGREGATE_DOMAINS_V1,
                lane,
                composition,
                &composition_opening_indices,
            )
            .map_err(map_aggregate_error_v1)?;
            if commitment.root != composition_roots[lane] {
                return Err(ZkX509StarkErrorV1::InternalInvariant);
            }
            composition_frontiers.push(commitment.frontier);
        }
        let fri_mask_frontiers = fri_masks
            .iter()
            .map(|mask| {
                aggregate::canonical_multiproof_frontier_v1(
                    &mask.tree,
                    self.layout.common_lde_size(),
                    &composition_opening_indices,
                )
                .map_err(map_aggregate_error_v1)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut fri_openings = Vec::new();
        fri_openings
            .try_reserve_exact(SECURITY_LANES)
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for (lane, (base_values, material)) in fri_bases.into_iter().zip(&fri_materials).enumerate()
        {
            fri_openings.push(
                aggregate::open_streaming_fri_lane_v1(
                    AGGREGATE_PARAMETERS_V1,
                    AGGREGATE_DOMAINS_V1,
                    &shared_layout,
                    lane,
                    base_values,
                    material,
                    &query_indices,
                )
                .map_err(map_aggregate_error_v1)?,
            );
        }
        let mut queries = Vec::new();
        queries
            .try_reserve_exact(query_indices.len())
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        for (query_position, index) in query_indices.iter().copied().enumerate() {
            let mut opened_groups = Vec::new();
            opened_groups
                .try_reserve_exact(FULL_PROFILE_TRACE_GROUPS_V1)
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
            for group_index in 0..FULL_PROFILE_TRACE_GROUPS_V1 {
                let next_stride = self.layout.trace_groups[group_index]
                    .next_stride(self.layout.common_lde_log2)?;
                let next = (index + next_stride) % self.layout.common_lde_size();
                let base = base_openings
                    .get(group_index)
                    .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
                let aux = aux_openings
                    .get(group_index)
                    .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
                opened_groups.push(aggregate::AggregateTraceGroupQueryV1 {
                    base_current: base
                        .opened_rows
                        .get(&index)
                        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?
                        .iter()
                        .map(|value| value.0)
                        .collect(),
                    base_next: base
                        .opened_rows
                        .get(&next)
                        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?
                        .iter()
                        .map(|value| value.0)
                        .collect(),
                    aux_current: aux
                        .opened_rows
                        .get(&index)
                        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?
                        .iter()
                        .map(|value| value.0)
                        .collect(),
                    aux_next: aux
                        .opened_rows
                        .get(&next)
                        .ok_or(ZkX509StarkErrorV1::InternalInvariant)?
                        .iter()
                        .map(|value| value.0)
                        .collect(),
                });
            }
            queries.push(aggregate::AggregateQueryProofV1 {
                index: u32::try_from(index).map_err(|_| ZkX509StarkErrorV1::InternalInvariant)?,
                trace_groups: opened_groups,
                composition_values: compositions
                    .iter()
                    .map(|lane| {
                        lane.iter()
                            .map(|chunk| chunk[index].coefficients().map(F::value))
                            .collect()
                    })
                    .collect(),
                fri_mask_values: fri_masks
                    .iter()
                    .map(|mask| mask.evaluations[index].coefficients().map(F::value))
                    .collect(),
                fri_lanes: fri_openings
                    .iter()
                    .map(|lane| {
                        lane.queries
                            .get(query_position)
                            .cloned()
                            .ok_or(ZkX509StarkErrorV1::InternalInvariant)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            });
        }
        drop(self.base_polynomials);
        drop(self.aux_polynomials);
        let fri_lanes = fri_materials
            .into_iter()
            .zip(fri_openings)
            .map(|(material, openings)| FriLaneProofV1 {
                roots: material.roots,
                terminal_values: material
                    .terminal_values
                    .into_iter()
                    .map(|value| value.coefficients().map(F::value))
                    .collect(),
                round_frontiers: openings.round_frontiers,
            })
            .collect();
        let proof = ZkX509SegmentedStarkProofV1 {
            aggregate: aggregate::AggregateStarkProofV1 {
                version: ZK_X509_PROOF_VERSION_V1,
                trace_groups: self.trace_groups,
                composition_roots,
                composition_frontiers,
                fri_mask_roots,
                fri_mask_frontiers,
                fri_lanes,
                queries,
                grinding_nonce,
            },
            deep,
        };
        let aggregate_bytes = encode_zk_x509_segmented_stark_proof_v1(&proof, &self.layout)?;
        encode_zk_x509_main_proof_envelope_v1(self.terminal_claims, &aggregate_bytes)
    }
}
/// Exact six-provider registry for the verifier-owned full MAIN layout.
///
/// The layout is cloned only after every dimension and closed provider
/// discriminator is validated, preventing later caller mutation.
#[cfg(test)]
pub(super) struct MainTraceProviderSetV1<'a> {
    layout: AggregateProofLayoutV1,
    groups: Vec<MainTraceGroupProviderV1<'a>>,
}
#[cfg(test)]
impl<'a> MainTraceProviderSetV1<'a> {
    pub(super) fn new_v1(
        layout: &AggregateProofLayoutV1,
        groups: Vec<MainTraceGroupProviderV1<'a>>,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        if groups.len() != FULL_PROFILE_TRACE_GROUPS_V1
            || groups.len() != layout.trace_groups.len()
            || groups
                .iter()
                .zip(&layout.trace_groups)
                .any(|(provider, group)| provider.native_trace_log2_v1() != group.native_trace_log2)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(MainTraceProviderSetV1 {
            layout: layout.clone(),
            groups,
        })
    }
    fn validate_v1(&self) -> Result<(), ZkX509StarkErrorV1> {
        self.layout.validate_exact_full_profile_registration_v1()?;
        if self.groups.len() != FULL_PROFILE_TRACE_GROUPS_V1
            || self.groups.len() != self.layout.trace_groups.len()
            || self
                .groups
                .iter()
                .zip(&self.layout.trace_groups)
                .any(|(provider, group)| provider.native_trace_log2_v1() != group.native_trace_log2)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(())
    }
    pub(super) fn registered_column_v1(
        &self,
        group_index: usize,
        kind: MainTraceColumnKindV1,
        column_index: usize,
    ) -> Result<(RegisteredSegmentLayoutV1, usize), ZkX509StarkErrorV1> {
        self.validate_v1()?;
        registered_main_group_column_v1(&self.layout, group_index, kind, column_index)
    }
    pub(super) fn native_group_column_v1(
        &mut self,
        group_index: usize,
        kind: MainTraceColumnKindV1,
        column_index: usize,
    ) -> Result<ZeroizingMainTraceColumnV1, ZkX509StarkErrorV1> {
        let (registration, local_column) =
            self.registered_column_v1(group_index, kind, column_index)?;
        let expected_rows = registration.segment.trace_size();
        let source = self
            .groups
            .get_mut(group_index)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
            .source_mut_v1();
        let column = match kind {
            MainTraceColumnKindV1::Base => {
                source.native_base_column_v1(registration, local_column)?
            }
            MainTraceColumnKindV1::Aux => {
                source.native_aux_column_v1(registration, local_column)?
            }
        };
        if column.len() != expected_rows
            || column.iter().any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(column)
    }
}
/// Closed prover-side fixed-polynomial and quotient provider for one canonical MAIN trace group.
///
/// The variants are verifier-derived and exhaustive. No dynamic callback can supply fixed rows or a
/// quotient value, and every challenge-dependent source borrows the already-bound X5B1 phase.
#[cfg(any(test, feature = "privacy-release-evidence"))]
enum MainProverConstraintProviderV1<'phase, 'assembly> {
    Log5(MainP256Log5ProverConstraintSourceV1<'phase>),
    P256Scalar(MainP256ScalarProverConstraintSourceV1<'phase>),
    Projection(MainProjectionProverConstraintSourceV1),
    Log16(MainP256Log16ProverConstraintSourceV1<'phase>),
    Io(MainIoProverConstraintSourceV1),
    Log19(MainLog19ProverConstraintSourceV1<'assembly, 'phase>),
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
impl MainProverConstraintProviderV1<'_, '_> {
    fn native_trace_log2_v1(&self) -> u8 {
        match self {
            Self::Log5(_) => 5,
            Self::P256Scalar(_) => 8,
            Self::Projection(_) => 15,
            Self::Log16(_) => 16,
            Self::Io(_) => 18,
            Self::Log19(_) => 19,
        }
    }
    fn stream_fixed_polynomials_v1(
        &self,
        mut consume: impl FnMut(
            RegisteredSegmentLayoutV1,
            usize,
            &[F],
        ) -> Result<(), ZkX509StarkErrorV1>,
    ) -> Result<(), ZkX509StarkErrorV1> {
        match self {
            Self::Log5(source) => source.stream_fixed_polynomials_v1(&mut consume),
            Self::P256Scalar(source) => source.stream_fixed_polynomials_v1(&mut consume),
            Self::Projection(source) => {
                source.stream_fixed_polynomials_v1(|column, coefficients| {
                    consume(source.registration, column, coefficients)
                })
            }
            Self::Log16(source) => source.stream_fixed_polynomials_v1(&mut consume),
            Self::Io(source) => source.stream_fixed_polynomials_v1(|column, coefficients| {
                consume(source.registration, column, coefficients)
            }),
            Self::Log19(source) => source.stream_fixed_polynomials_v1(consume),
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn composition_value_v1(
        &self,
        registration: RegisteredSegmentLayoutV1,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
        fixed_current: &[F],
        fixed_next: &[F],
        alphas: &[E],
    ) -> Result<E, ZkX509StarkErrorV1> {
        if fixed_current.len() != registration.segment.fixed_width
            || fixed_next.len() != registration.segment.fixed_width
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        match self {
            Self::Log5(source) => {
                source.composition_value_v1(registration, x, opening, fixed_current, alphas)
            }
            Self::P256Scalar(source) => source.composition_value_v1(
                registration,
                x,
                opening,
                fixed_current
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                alphas,
            ),
            Self::Projection(source) => source.composition_value_v1(
                registration,
                x,
                opening,
                fixed_current
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                alphas,
            ),
            Self::Log16(source) => {
                source.composition_value_v1(registration, x, opening, fixed_current, alphas)
            }
            Self::Io(source) => source.composition_value_v1(
                registration,
                x,
                opening,
                fixed_current
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
                alphas,
            ),
            Self::Log19(source) => source.composition_value_v1(
                registration,
                x,
                opening,
                fixed_current,
                fixed_next,
                alphas,
            ),
        }
    }
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
struct ZeroizingMainFixedPolynomialSetV1 {
    registration: RegisteredSegmentLayoutV1,
    columns: Vec<Vec<F>>,
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
impl Drop for ZeroizingMainFixedPolynomialSetV1 {
    fn drop(&mut self) {
        for column in &mut self.columns {
            column.fill(F::ZERO);
        }
    }
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn stream_main_fixed_polynomial_sets_v1(
    provider: &MainProverConstraintProviderV1<'_, '_>,
    mut consume: impl FnMut(&ZeroizingMainFixedPolynomialSetV1) -> Result<(), ZkX509StarkErrorV1>,
) -> Result<(), ZkX509StarkErrorV1> {
    let mut pending: Option<ZeroizingMainFixedPolynomialSetV1> = None;
    provider.stream_fixed_polynomials_v1(|registration, local_column, coefficients| {
        if registration.segment.trace_log2 != provider.native_trace_log2_v1()
            || coefficients.len() != registration.segment.trace_size()
            || coefficients
                .iter()
                .any(|coefficient| F::canonical(coefficient.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        if pending
            .as_ref()
            .is_some_and(|set| set.registration != registration)
        {
            let completed = pending
                .take()
                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
            if completed.columns.len() != completed.registration.segment.fixed_width {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            consume(&completed)?;
        }
        let set = pending.get_or_insert_with(|| ZeroizingMainFixedPolynomialSetV1 {
            registration,
            columns: Vec::new(),
        });
        if set.registration != registration
            || local_column != set.columns.len()
            || local_column >= registration.segment.fixed_width
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let mut retained = Vec::new();
        retained
            .try_reserve_exact(coefficients.len())
            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
        retained.extend_from_slice(coefficients);
        set.columns.push(retained);
        Ok(())
    })?;
    let completed = pending.ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    if completed.columns.len() != completed.registration.segment.fixed_width {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    consume(&completed)
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn canonical_main_registration_index_v1(
    layout: &AggregateProofLayoutV1,
    registration: RegisteredSegmentLayoutV1,
) -> Result<usize, ZkX509StarkErrorV1> {
    layout.validate_exact_full_profile_registration_v1()?;
    layout
        .registered_segments
        .binary_search_by_key(
            &(
                registration.segment.trace_log2,
                registration.segment.adapter,
                registration.segment.instance,
            ),
            |candidate| {
                (
                    candidate.segment.trace_log2,
                    candidate.segment.adapter,
                    candidate.segment.instance,
                )
            },
        )
        .ok()
        .filter(|index| layout.registered_segments.get(*index) == Some(&registration))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn main_registration_trace_columns_on_coset_v1(
    layout: &AggregateProofLayoutV1,
    polynomials: &MainTracePolynomialSetV1,
    kind: MainTraceColumnKindV1,
    registration: RegisteredSegmentLayoutV1,
    evaluation_log2: u8,
) -> Result<ZeroizingBaseColumnsV1, ZkX509StarkErrorV1> {
    canonical_main_registration_index_v1(layout, registration)?;
    if evaluation_log2 <= registration.segment.trace_log2
        || evaluation_log2 > layout.common_lde_log2
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let group = polynomials.group_v1(layout, kind, registration.trace_group)?;
    let (start, end, width) = match kind {
        MainTraceColumnKindV1::Base => (
            registration.base_start,
            registration.base_end()?,
            registration.segment.base_width,
        ),
        MainTraceColumnKindV1::Aux => (
            registration.aux_start,
            registration.aux_end()?,
            registration.segment.aux_width,
        ),
    };
    let evaluation_rows = 1_usize
        .checked_shl(u32::from(evaluation_log2))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let evaluated = (start..end)
        .into_par_iter()
        .map(|column| {
            group
                .evaluate_column_on_coset_v1(column, evaluation_log2)
                .map_err(map_aggregate_error_v1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut columns = ZeroizingBaseColumnsV1(Vec::new());
    columns
        .0
        .try_reserve_exact(width)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for evaluated in evaluated {
        if evaluated.len() != evaluation_rows
            || evaluated
                .iter()
                .any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        columns.0.push(evaluated.into_vec_v1());
    }
    if columns.len() != width {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    Ok(columns)
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn main_fixed_columns_on_coset_v1(
    fixed: &ZeroizingMainFixedPolynomialSetV1,
    evaluation_log2: u8,
) -> Result<ZeroizingBaseColumnsV1, ZkX509StarkErrorV1> {
    let registration = fixed.registration;
    if evaluation_log2 <= registration.segment.trace_log2
        || fixed.columns.len() != registration.segment.fixed_width
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let evaluation_rows = 1_usize
        .checked_shl(u32::from(evaluation_log2))
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let evaluation_root =
        goldilocks_primitive_root_v1(evaluation_log2).map_err(map_transparent_error_v1)?;
    if fixed.columns.iter().any(|coefficients| {
        coefficients.len() != registration.segment.trace_size()
            || coefficients
                .iter()
                .any(|value| F::canonical(value.0).is_none())
    }) {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let evaluated = fixed
        .columns
        .par_iter()
        .map(|coefficients| {
            goldilocks_evaluate_coset_v1(
                coefficients,
                evaluation_rows,
                evaluation_root,
                F(GOLDILOCKS_GENERATOR_V1),
            )
            .map(ZeroizingMainTraceColumnV1)
            .map_err(map_transparent_error_v1)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut columns = ZeroizingBaseColumnsV1(Vec::new());
    columns
        .0
        .try_reserve_exact(evaluated.len())
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for evaluated in evaluated {
        if evaluated.len() != evaluation_rows {
            return Err(ZkX509StarkErrorV1::InternalInvariant);
        }
        columns.0.push(evaluated.into_vec_v1());
    }
    Ok(columns)
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
struct MainQuotientRowScratchV1 {
    base_current: ZeroizingMainTraceColumnV1,
    base_next: ZeroizingMainTraceColumnV1,
    aux_current: ZeroizingMainTraceColumnV1,
    aux_next: ZeroizingMainTraceColumnV1,
    fixed_current: ZeroizingMainTraceColumnV1,
    fixed_next: ZeroizingMainTraceColumnV1,
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
impl MainQuotientRowScratchV1 {
    fn new_v1(registration: RegisteredSegmentLayoutV1) -> Self {
        Self {
            base_current: ZeroizingMainTraceColumnV1(vec![
                F::ZERO;
                registration.segment.base_width
            ]),
            base_next: ZeroizingMainTraceColumnV1(vec![F::ZERO; registration.segment.base_width]),
            aux_current: ZeroizingMainTraceColumnV1(vec![F::ZERO; registration.segment.aux_width]),
            aux_next: ZeroizingMainTraceColumnV1(vec![F::ZERO; registration.segment.aux_width]),
            fixed_current: ZeroizingMainTraceColumnV1(vec![
                F::ZERO;
                registration.segment.fixed_width
            ]),
            fixed_next: ZeroizingMainTraceColumnV1(vec![F::ZERO; registration.segment.fixed_width]),
        }
    }
    fn fill_v1(
        &mut self,
        row: usize,
        next: usize,
        base: &[Vec<F>],
        aux: &[Vec<F>],
        fixed: &[Vec<F>],
    ) {
        for (column, (current, next_value)) in self
            .base_current
            .iter_mut()
            .zip(&mut *self.base_next)
            .enumerate()
        {
            *current = base[column][row];
            *next_value = base[column][next];
        }
        for (column, (current, next_value)) in self
            .aux_current
            .iter_mut()
            .zip(&mut *self.aux_next)
            .enumerate()
        {
            *current = aux[column][row];
            *next_value = aux[column][next];
        }
        for (column, (current, next_value)) in self
            .fixed_current
            .iter_mut()
            .zip(&mut *self.fixed_next)
            .enumerate()
        {
            *current = fixed[column][row];
            *next_value = fixed[column][next];
        }
    }
    fn opening_v1(&self) -> RegisteredOpenedRowsV1<'_> {
        RegisteredOpenedRowsV1 {
            base_current: &self.base_current,
            base_next: &self.base_next,
            aux_current: &self.aux_current,
            aux_next: &self.aux_next,
        }
    }
}
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn main_registration_composition_coefficient_chunks_v1(
    layout: &AggregateProofLayoutV1,
    provider: &MainProverConstraintProviderV1<'_, '_>,
    base_polynomials: &MainTracePolynomialSetV1,
    aux_polynomials: &MainTracePolynomialSetV1,
    fixed: &ZeroizingMainFixedPolynomialSetV1,
    alphas: &[Vec<E>],
    shared_layout: &aggregate::AggregateProofLayoutV1,
) -> Result<Vec<Vec<Vec<E>>>, ZkX509StarkErrorV1> {
    let registration = fixed.registration;
    canonical_main_registration_index_v1(layout, registration)?;
    let plan = registered_retained_prover_plan_v1(registration.segment, layout.common_lde_log2)?;
    if provider.native_trace_log2_v1() != registration.segment.trace_log2
        || alphas.len() != SECURITY_LANES
        || alphas
            .iter()
            .any(|lane| lane.len() != registration.segment.constraint_count)
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let base = main_registration_trace_columns_on_coset_v1(
        layout,
        base_polynomials,
        MainTraceColumnKindV1::Base,
        registration,
        plan.quotient_coset_log2,
    )?;
    let aux = main_registration_trace_columns_on_coset_v1(
        layout,
        aux_polynomials,
        MainTraceColumnKindV1::Aux,
        registration,
        plan.quotient_coset_log2,
    )?;
    let fixed = main_fixed_columns_on_coset_v1(fixed, plan.quotient_coset_log2)?;
    if base.len() != registration.segment.base_width
        || aux.len() != registration.segment.aux_width
        || fixed.len() != registration.segment.fixed_width
        || base
            .iter()
            .chain(aux.iter())
            .chain(fixed.iter())
            .any(|column| column.len() != plan.quotient_coset_rows)
    {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    let mut quotients = (0..SECURITY_LANES)
        .map(|_| {
            let mut quotient = ZeroizingExtensionColumnV1(Vec::new());
            quotient
                .0
                .try_reserve_exact(plan.quotient_coset_rows)
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
            quotient.0.resize(plan.quotient_coset_rows, E::ZERO);
            Ok::<_, ZkX509StarkErrorV1>(quotient)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let coset_root =
        goldilocks_primitive_root_v1(plan.quotient_coset_log2).map_err(map_transparent_error_v1)?;
    // Each quotient row is independent after the trace, fixed-polynomial, and
    // alpha schedules are frozen. The release runner owns an exact four-thread
    // Rayon pool; fixed-size chunks avoid per-row allocation while preserving
    // canonical row order and identical field values.
    const QUOTIENT_ROWS_PER_TASK_V1: usize = 1 << 12;
    quotients
        .par_iter_mut()
        .enumerate()
        .try_for_each(|(lane, quotient)| {
            quotient
                .0
                .par_chunks_mut(QUOTIENT_ROWS_PER_TASK_V1)
                .enumerate()
                .try_for_each_init(
                    || MainQuotientRowScratchV1::new_v1(registration),
                    |scratch, (chunk_index, output)| {
                        let first_row = chunk_index
                            .checked_mul(QUOTIENT_ROWS_PER_TASK_V1)
                            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
                        let mut x =
                            F(GOLDILOCKS_GENERATOR_V1).mul(coset_root.pow(first_row as u128));
                        for (offset, target) in output.iter_mut().enumerate() {
                            let row = first_row
                                .checked_add(offset)
                                .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
                            let next = (row + plan.quotient_next_stride) % plan.quotient_coset_rows;
                            scratch.fill_v1(row, next, &base, &aux, &fixed);
                            *target = provider.composition_value_v1(
                                registration,
                                x,
                                scratch.opening_v1(),
                                &scratch.fixed_current,
                                &scratch.fixed_next,
                                &alphas[lane],
                            )?;
                            x = x.mul(coset_root);
                        }
                        Ok::<_, ZkX509StarkErrorV1>(())
                    },
                )
        })?;
    drop(base);
    drop(aux);
    drop(fixed);
    let mut coefficient_chunks = Vec::new();
    coefficient_chunks
        .try_reserve_exact(SECURITY_LANES)
        .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
    for quotient in &quotients {
        let coefficients = fp4_coset_coefficients_v1(quotient, plan.quotient_coset_log2)?;
        coefficient_chunks.push(composition_coefficient_chunks_v1(
            &coefficients,
            plan.maximum_quotient_degree,
            shared_layout,
        )?);
    }
    Ok(coefficient_chunks)
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
pub(super) fn add_main_composition_coefficient_chunks_v1(
    accumulator: &mut [Vec<Vec<E>>],
    contribution: &[Vec<Vec<E>>],
    coefficient_cap: usize,
) -> Result<(), ZkX509StarkErrorV1> {
    if coefficient_cap == 0
        || accumulator.len() != SECURITY_LANES
        || contribution.len() != SECURITY_LANES
        || accumulator
            .iter()
            .chain(contribution)
            .any(|lane| lane.len() != COMPOSITION_DEGREE_CHUNKS)
        || accumulator.iter().chain(contribution).any(|lane| {
            lane.iter().any(|chunk| {
                chunk.len() > coefficient_cap
                    || chunk.iter().any(|coefficient| !coefficient.is_canonical())
            })
        })
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    // Complete every fallible capacity reservation before changing a logical
    // coefficient. A hostile late chunk therefore cannot leave a partially
    // accumulated composition behind.
    for lane in 0..SECURITY_LANES {
        for chunk in 0..COMPOSITION_DEGREE_CHUNKS {
            let source_len = contribution[lane][chunk].len();
            let target = &mut accumulator[lane][chunk];
            if target.len() < source_len {
                target
                    .try_reserve_exact(source_len - target.len())
                    .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
            }
        }
    }
    for lane in 0..SECURITY_LANES {
        for chunk in 0..COMPOSITION_DEGREE_CHUNKS {
            let source = &contribution[lane][chunk];
            let target = &mut accumulator[lane][chunk];
            if target.len() < source.len() {
                target.resize(source.len(), E::ZERO);
            }
            for (target, source) in target.iter_mut().zip(source) {
                *target = target.add(*source);
            }
            let retained = target
                .iter()
                .rposition(|coefficient| *coefficient != E::ZERO)
                .map_or(0, |degree| degree + 1);
            target.truncate(retained);
        }
    }
    Ok(())
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn evaluate_main_composition_coefficient_chunks_v1(
    coefficient_chunks: &[Vec<Vec<E>>],
    shared_layout: &aggregate::AggregateProofLayoutV1,
) -> Result<Vec<Vec<Vec<E>>>, ZkX509StarkErrorV1> {
    if coefficient_chunks.len() != SECURITY_LANES
        || coefficient_chunks
            .iter()
            .any(|lane| lane.len() != COMPOSITION_DEGREE_CHUNKS)
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let common_rows = shared_layout.common_lde_size();
    let common_root = goldilocks_primitive_root_v1(shared_layout.common_lde_log2())
        .map_err(map_transparent_error_v1)?;
    let coefficient_cap = shared_layout
        .fri_degree_cap(AGGREGATE_PARAMETERS_V1)
        .map_err(map_aggregate_error_v1)?;
    coefficient_chunks
        .iter()
        .map(|lane| {
            lane.iter()
                .map(|coefficients| {
                    if coefficients.len() > coefficient_cap
                        || coefficients
                            .iter()
                            .any(|coefficient| !coefficient.is_canonical())
                    {
                        return Err(ZkX509StarkErrorV1::ProfileMismatch);
                    }
                    if coefficients.is_empty() {
                        let mut zero = Vec::new();
                        zero.try_reserve_exact(common_rows)
                            .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
                        zero.resize(common_rows, E::ZERO);
                        Ok(zero)
                    } else {
                        goldilocks_fp4_evaluate_coset_v1(
                            coefficients,
                            common_rows,
                            common_root,
                            F(GOLDILOCKS_GENERATOR_V1),
                        )
                        .map_err(map_transparent_error_v1)
                    }
                })
                .collect()
        })
        .collect()
}
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn main_composition_material_from_polynomials_v1(
    layout: &AggregateProofLayoutV1,
    base_polynomials: &MainTracePolynomialSetV1,
    aux_polynomials: &MainTracePolynomialSetV1,
    providers: &[MainProverConstraintProviderV1<'_, '_>],
    alphas: &[Vec<Vec<E>>],
) -> Result<RetainedCompositionMaterialV1, ZkX509StarkErrorV1> {
    layout.validate_exact_full_profile_registration_v1()?;
    base_polynomials.validate_v1(layout, MainTraceColumnKindV1::Base)?;
    aux_polynomials.validate_v1(layout, MainTraceColumnKindV1::Aux)?;
    if providers.len() != FULL_PROFILE_TRACE_GROUPS_V1
        || providers
            .iter()
            .zip(&layout.trace_groups)
            .any(|(provider, group)| provider.native_trace_log2_v1() != group.native_trace_log2)
        || alphas.len() != layout.registered_segments.len()
        || alphas
            .iter()
            .zip(&layout.registered_segments)
            .any(|(lanes, registration)| {
                lanes.len() != SECURITY_LANES
                    || lanes
                        .iter()
                        .any(|lane| lane.len() != registration.segment.constraint_count)
            })
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let shared_layout = layout.as_shared()?;
    let coefficient_cap = shared_layout
        .fri_degree_cap(AGGREGATE_PARAMETERS_V1)
        .map_err(map_aggregate_error_v1)?;
    let mut coefficient_chunks = (0..SECURITY_LANES)
        .map(|_| {
            (0..COMPOSITION_DEGREE_CHUNKS)
                .map(|_| Vec::new())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut seen_registrations = 0_usize;
    for (group_index, provider) in providers.iter().enumerate() {
        let expected = layout
            .registered_segments
            .iter()
            .copied()
            .filter(|registration| registration.trace_group == group_index)
            .collect::<Vec<_>>();
        let mut seen_in_group = 0_usize;
        stream_main_fixed_polynomial_sets_v1(provider, |fixed| {
            if expected.get(seen_in_group).copied() != Some(fixed.registration) {
                return Err(ZkX509StarkErrorV1::ProfileMismatch);
            }
            let registration_index =
                canonical_main_registration_index_v1(layout, fixed.registration)?;
            let contribution = main_registration_composition_coefficient_chunks_v1(
                layout,
                provider,
                base_polynomials,
                aux_polynomials,
                fixed,
                &alphas[registration_index],
                &shared_layout,
            )?;
            add_main_composition_coefficient_chunks_v1(
                &mut coefficient_chunks,
                &contribution,
                coefficient_cap,
            )?;
            seen_in_group += 1;
            seen_registrations += 1;
            Ok(())
        })?;
        if seen_in_group != expected.len() {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
    }
    if seen_registrations != layout.registered_segments.len() {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let evaluations =
        evaluate_main_composition_coefficient_chunks_v1(&coefficient_chunks, &shared_layout)?;
    Ok(RetainedCompositionMaterialV1 {
        evaluations,
        coefficient_chunks,
    })
}
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
#[cfg(any(test, feature = "privacy-release-evidence"))]
fn main_fri_bases_from_polynomials_v1(
    layout: &AggregateProofLayoutV1,
    base_polynomials: &MainTracePolynomialSetV1,
    aux_polynomials: &MainTracePolynomialSetV1,
    composition_coefficients: &[Vec<Vec<E>>],
    mixes: &[Vec<FriMixV1>],
    deep_point: E,
    deep_trace_groups: &[aggregate::AggregateOpenedDeepTraceGroupV1],
    deep_compositions: &[Vec<E>],
) -> Result<Vec<Vec<E>>, ZkX509StarkErrorV1> {
    layout.validate_exact_full_profile_registration_v1()?;
    base_polynomials.validate_v1(layout, MainTraceColumnKindV1::Base)?;
    aux_polynomials.validate_v1(layout, MainTraceColumnKindV1::Aux)?;
    validate_main_fri_mixes_v1(layout, mixes)?;
    if !deep_point.is_canonical()
        || composition_coefficients.len() != SECURITY_LANES
        || composition_coefficients
            .iter()
            .any(|lane| lane.len() != COMPOSITION_DEGREE_CHUNKS)
        || deep_trace_groups.len() != FULL_PROFILE_TRACE_GROUPS_V1
        || deep_compositions.len() != SECURITY_LANES
        || deep_compositions
            .iter()
            .any(|values| values.len() != COMPOSITION_DEGREE_CHUNKS)
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let shared_layout = layout.as_shared()?;
    let coefficient_cap = shared_layout
        .fri_degree_cap(AGGREGATE_PARAMETERS_V1)
        .map_err(map_aggregate_error_v1)?;
    let mut accumulators = (0..SECURITY_LANES)
        .map(|_| {
            let mut accumulator = ZeroizingExtensionColumnV1(Vec::new());
            accumulator
                .0
                .try_reserve_exact(coefficient_cap)
                .map_err(|_| ZkX509StarkErrorV1::AllocationFailure)?;
            accumulator.0.resize(coefficient_cap, E::ZERO);
            Ok::<_, ZkX509StarkErrorV1>(accumulator)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for group_index in 0..FULL_PROFILE_TRACE_GROUPS_V1 {
        let group_layout = layout
            .trace_groups
            .get(group_index)
            .ok_or(ZkX509StarkErrorV1::InternalInvariant)?;
        let base_group =
            base_polynomials.group_v1(layout, MainTraceColumnKindV1::Base, group_index)?;
        let aux_group =
            aux_polynomials.group_v1(layout, MainTraceColumnKindV1::Aux, group_index)?;
        let deep = deep_trace_groups
            .get(group_index)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let group_mixes = mixes
            .get(group_index)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if deep.base_current.len() != group_layout.base_width
            || deep.base_next.len() != group_layout.base_width
            || deep.aux_current.len() != group_layout.aux_width
            || deep.aux_next.len() != group_layout.aux_width
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let native_root = goldilocks_primitive_root_v1(group_layout.native_trace_log2)
            .map_err(map_transparent_error_v1)?;
        let deep_next_point = deep_point.mul_base(native_root);
        for column in 0..base_group.width() {
            let coefficients = base_group
                .column_coefficients_v1(column)
                .map_err(map_aggregate_error_v1)?;
            for lane in 0..SECURITY_LANES {
                accumulate_base_deep_quotient_v1(
                    coefficients,
                    deep_point,
                    deep.base_current[column],
                    group_mixes[lane].base[column],
                    &mut accumulators[lane].0,
                )?;
                accumulate_base_deep_quotient_v1(
                    coefficients,
                    deep_next_point,
                    deep.base_next[column],
                    group_mixes[lane].base_next[column],
                    &mut accumulators[lane].0,
                )?;
            }
        }
        for column in 0..aux_group.width() {
            let coefficients = aux_group
                .column_coefficients_v1(column)
                .map_err(map_aggregate_error_v1)?;
            for lane in 0..SECURITY_LANES {
                accumulate_base_deep_quotient_v1(
                    coefficients,
                    deep_point,
                    deep.aux_current[column],
                    group_mixes[lane].aux[column],
                    &mut accumulators[lane].0,
                )?;
                accumulate_base_deep_quotient_v1(
                    coefficients,
                    deep_next_point,
                    deep.aux_next[column],
                    group_mixes[lane].aux_next[column],
                    &mut accumulators[lane].0,
                )?;
            }
        }
    }
    for lane in 0..SECURITY_LANES {
        let composition_mix = &mixes[0][lane].composition;
        for chunk in 0..COMPOSITION_DEGREE_CHUNKS {
            accumulate_extension_deep_quotient_v1(
                &composition_coefficients[lane][chunk],
                deep_point,
                deep_compositions[lane][chunk],
                composition_mix[chunk],
                &mut accumulators[lane].0,
            )?;
        }
    }
    let common_root =
        goldilocks_primitive_root_v1(layout.common_lde_log2).map_err(map_transparent_error_v1)?;
    accumulators
        .iter()
        .map(|coefficients| {
            goldilocks_fp4_evaluate_coset_v1(
                coefficients,
                layout.common_lde_size(),
                common_root,
                F(GOLDILOCKS_GENERATOR_V1),
            )
            .map_err(map_transparent_error_v1)
        })
        .collect()
}
/// Exact six-provider registry used only for verifier-safe opened-row evaluation.
pub(super) struct MainOpenedProviderSetV1<'a> {
    layout: AggregateProofLayoutV1,
    groups: Vec<MainOpenedGroupProviderV1<'a>>,
}
impl<'a> MainOpenedProviderSetV1<'a> {
    pub(super) fn new_v1(
        layout: &AggregateProofLayoutV1,
        groups: Vec<MainOpenedGroupProviderV1<'a>>,
    ) -> Result<Self, ZkX509StarkErrorV1> {
        layout.validate_exact_full_profile_registration_v1()?;
        if groups.len() != FULL_PROFILE_TRACE_GROUPS_V1
            || groups.len() != layout.trace_groups.len()
            || groups
                .iter()
                .zip(&layout.trace_groups)
                .any(|(provider, group)| provider.native_trace_log2_v1() != group.native_trace_log2)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(MainOpenedProviderSetV1 {
            layout: layout.clone(),
            groups,
        })
    }
    fn validate_v1(&self) -> Result<(), ZkX509StarkErrorV1> {
        self.layout.validate_exact_full_profile_registration_v1()?;
        if self.groups.len() != FULL_PROFILE_TRACE_GROUPS_V1
            || self.groups.len() != self.layout.trace_groups.len()
            || self
                .groups
                .iter()
                .zip(&self.layout.trace_groups)
                .any(|(provider, group)| provider.native_trace_log2_v1() != group.native_trace_log2)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        Ok(())
    }
    pub(super) fn registered_constraint_residues_v1(
        &mut self,
        registration: RegisteredSegmentLayoutV1,
        query_index: usize,
        next_query_index: usize,
        x: F,
        opening: RegisteredOpenedRowsV1<'_>,
    ) -> Result<Vec<F>, ZkX509StarkErrorV1> {
        self.validate_v1()?;
        if self
            .layout
            .registered_segments
            .get(
                self.layout
                    .registered_segments
                    .binary_search_by_key(
                        &(
                            registration.segment.trace_log2,
                            registration.segment.adapter,
                            registration.segment.instance,
                        ),
                        |candidate| {
                            (
                                candidate.segment.trace_log2,
                                candidate.segment.adapter,
                                candidate.segment.instance,
                            )
                        },
                    )
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?,
            )
            .copied()
            != Some(registration)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        let provider = self
            .groups
            .get_mut(registration.trace_group)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        if provider.native_trace_log2_v1() != registration.segment.trace_log2 {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        provider.constraint_residues_v1(registration, query_index, next_query_index, x, opening)
    }
}
fn validate_main_opened_evaluation_shape_v1(
    providers: &MainOpenedProviderSetV1<'_>,
    query_index: usize,
    lane: usize,
    trace_groups: &[aggregate::AggregateOpenedTraceGroupV1],
    alphas: &[Vec<Vec<E>>],
) -> Result<(), ZkX509StarkErrorV1> {
    providers.validate_v1()?;
    let layout = &providers.layout;
    if query_index >= layout.common_lde_size()
        || lane >= SECURITY_LANES
        || trace_groups.len() != layout.trace_groups.len()
        || alphas.len() != layout.registered_segments.len()
        || trace_groups
            .iter()
            .zip(&layout.trace_groups)
            .any(|(opening, group)| {
                opening.base_current.len() != group.base_width
                    || opening.base_next.len() != group.base_width
                    || opening.aux_current.len() != group.aux_width
                    || opening.aux_next.len() != group.aux_width
                    || opening
                        .base_current
                        .iter()
                        .chain(&opening.base_next)
                        .chain(&opening.aux_current)
                        .chain(&opening.aux_next)
                        .any(|value| F::canonical(value.0).is_none())
            })
        || alphas
            .iter()
            .zip(&layout.registered_segments)
            .any(|(lanes, registration)| {
                lanes.len() != SECURITY_LANES
                    || lanes
                        .iter()
                        .any(|values| values.len() != registration.segment.constraint_count)
            })
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(())
}
pub(super) fn main_opened_composition_value_v1(
    providers: &mut MainOpenedProviderSetV1<'_>,
    query_index: usize,
    lane: usize,
    trace_groups: &[aggregate::AggregateOpenedTraceGroupV1],
    alphas: &[Vec<Vec<E>>],
) -> Result<E, ZkX509StarkErrorV1> {
    validate_main_opened_evaluation_shape_v1(providers, query_index, lane, trace_groups, alphas)?;
    let lde_root = goldilocks_primitive_root_v1(providers.layout.common_lde_log2)
        .map_err(map_transparent_error_v1)?;
    let x = F(GOLDILOCKS_GENERATOR_V1).mul(lde_root.pow(query_index as u128));
    let mut composition = E::ZERO;
    for registration_index in 0..providers.layout.registered_segments.len() {
        let registration = providers.layout.registered_segments[registration_index];
        let opening = registered_opened_rows_v1(&providers.layout, registration, trace_groups)
            .map_err(map_aggregate_error_v1)?;
        let next_stride = providers
            .layout
            .trace_groups
            .get(registration.trace_group)
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?
            .next_stride(providers.layout.common_lde_log2)?;
        let next_query_index = query_index
            .checked_add(next_stride)
            .map(|index| index % providers.layout.common_lde_size())
            .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
        let residues = providers.registered_constraint_residues_v1(
            registration,
            query_index,
            next_query_index,
            x,
            opening,
        )?;
        if residues.len() != registration.segment.constraint_count
            || residues.iter().any(|value| F::canonical(value.0).is_none())
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
        composition = composition.add(accumulator_quotient_value_v1(
            registration.segment,
            x,
            &residues,
            &alphas[registration_index][lane],
        )?);
    }
    Ok(composition)
}
pub(super) fn validate_main_fri_mixes_v1(
    layout: &AggregateProofLayoutV1,
    mixes: &[Vec<FriMixV1>],
) -> Result<(), ZkX509StarkErrorV1> {
    layout.validate_exact_full_profile_registration_v1()?;
    if mixes.len() != layout.trace_groups.len()
        || mixes.iter().any(|lanes| lanes.len() != SECURITY_LANES)
        || mixes
            .iter()
            .zip(&layout.trace_groups)
            .any(|(lanes, group)| {
                lanes.iter().any(|mix| {
                    mix.base.len() != group.base_width
                        || mix.base_next.len() != group.base_width
                        || mix.aux.len() != group.aux_width
                        || mix.aux_next.len() != group.aux_width
                        || mix.composition.len() != COMPOSITION_DEGREE_CHUNKS
                })
            })
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    for lane in 0..SECURITY_LANES {
        let composition = &mixes[0][lane].composition;
        if mixes
            .iter()
            .any(|lanes| &lanes[lane].composition != composition)
        {
            return Err(ZkX509StarkErrorV1::ProfileMismatch);
        }
    }
    Ok(())
}
/// Full MAIN verifier opened-row evaluator.
///
/// This path is intentionally separate from prover fixed-polynomial streaming:
/// verification samples only the canonical query openings, while proving must
/// traverse the full common domain without inheriting a query-cache bound.
pub(super) struct MainOpenedRowEvaluatorV1<'a, 'providers> {
    pub(super) providers: &'a mut MainOpenedProviderSetV1<'providers>,
    pub(super) alphas: &'a [Vec<Vec<E>>],
    pub(super) mixes: &'a [Vec<FriMixV1>],
}
impl aggregate::AggregateOpenedRowEvaluatorV1 for MainOpenedRowEvaluatorV1<'_, '_> {
    fn evaluate_opened_row_v1(
        &mut self,
        query_index: usize,
        lane: usize,
        trace_groups: &[aggregate::AggregateOpenedTraceGroupV1],
        composition_chunks: &[E],
    ) -> Result<aggregate::AggregateExpectedOpeningV1, AggregateStarkErrorV1> {
        validate_main_fri_mixes_v1(&self.providers.layout, self.mixes)
            .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        if composition_chunks.len() != COMPOSITION_DEGREE_CHUNKS {
            return Err(AggregateStarkErrorV1::ConstraintOpening);
        }
        let composition = main_opened_composition_value_v1(
            self.providers,
            query_index,
            lane,
            trace_groups,
            self.alphas,
        )
        .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        let mut fri_base = E::ZERO;
        for (group_index, opening) in trace_groups.iter().enumerate() {
            let mix = &self.mixes[group_index][lane];
            fri_base = fri_base.add(
                opening
                    .base_current
                    .iter()
                    .zip(&mix.base)
                    .fold(E::ZERO, |sum, (value, coefficient)| {
                        sum.add(coefficient.mul_base(*value))
                    }),
            );
            fri_base = fri_base.add(
                opening
                    .aux_current
                    .iter()
                    .zip(&mix.aux)
                    .fold(E::ZERO, |sum, (value, coefficient)| {
                        sum.add(coefficient.mul_base(*value))
                    }),
            );
        }
        fri_base = fri_base.add(mix_opened_composition_chunks_v1(
            composition_chunks,
            &self.mixes[0][lane],
        )?);
        Ok(aggregate::AggregateExpectedOpeningV1 {
            composition,
            fri_base,
        })
    }
}
#[cfg(test)]
impl aggregate::AggregateOpenedRowEvaluatorV1 for DerOpenedRowEvaluatorV1<'_> {
    fn evaluate_opened_row_v1(
        &mut self,
        query_index: usize,
        lane: usize,
        trace_groups: &[aggregate::AggregateOpenedTraceGroupV1],
        composition_chunks: &[E],
    ) -> Result<aggregate::AggregateExpectedOpeningV1, AggregateStarkErrorV1> {
        let registration = self
            .aggregate_layout
            .registered_segment(SegmentAdapterIdV1::StrictDer, 0)
            .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        if registration.segment != self.layout {
            return Err(AggregateStarkErrorV1::ConstraintOpening);
        }
        let opening = registered_opened_rows_v1(self.aggregate_layout, registration, trace_groups)?;
        let alphas = self
            .alphas
            .get(lane)
            .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
        let mix = self
            .mixes
            .get(lane)
            .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
        let next_index = query_index
            .checked_add(
                self.aggregate_layout
                    .trace_groups
                    .get(registration.trace_group)
                    .ok_or(AggregateStarkErrorV1::ConstraintOpening)?
                    .next_stride(self.aggregate_layout.common_lde_log2)
                    .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?,
            )
            .map(|index| index % self.aggregate_layout.common_lde_size())
            .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
        let fixed = self
            .fixed_openings
            .get(&query_index)
            .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
        let next_fixed = self
            .fixed_openings
            .get(&next_index)
            .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(self.lde_root.pow(query_index as u128));
        let composition = der_quotient_value_v1(
            self.layout,
            x,
            opening.base_current,
            opening.base_next,
            opening.aux_current,
            opening.aux_next,
            fixed,
            next_fixed,
            self.challenges,
            self.public,
            self.claims,
            alphas,
        )
        .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        if opening.base_current.len() != mix.base.len()
            || opening.aux_current.len() != mix.aux.len()
        {
            return Err(AggregateStarkErrorV1::ConstraintOpening);
        }
        let mixed_base = opening
            .base_current
            .iter()
            .zip(&mix.base)
            .fold(E::ZERO, |sum, (value, coefficient)| {
                sum.add(coefficient.mul_base(*value))
            });
        let mixed_aux = opening
            .aux_current
            .iter()
            .zip(&mix.aux)
            .fold(E::ZERO, |sum, (value, coefficient)| {
                sum.add(coefficient.mul_base(*value))
            });
        Ok(aggregate::AggregateExpectedOpeningV1 {
            composition,
            fri_base: mixed_base
                .add(mixed_aux)
                .add(mix_opened_composition_chunks_v1(composition_chunks, mix)?),
        })
    }
}
#[cfg(test)]
impl aggregate::AggregateOpenedRowEvaluatorV1 for IoOpenedRowEvaluatorV1<'_> {
    fn evaluate_opened_row_v1(
        &mut self,
        query_index: usize,
        lane: usize,
        trace_groups: &[aggregate::AggregateOpenedTraceGroupV1],
        composition_chunks: &[E],
    ) -> Result<aggregate::AggregateExpectedOpeningV1, AggregateStarkErrorV1> {
        let registration = self
            .aggregate_layout
            .registered_segment(SegmentAdapterIdV1::ByteMemory, 0)
            .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        if registration.segment != self.layout {
            return Err(AggregateStarkErrorV1::ConstraintOpening);
        }
        let opening = registered_opened_rows_v1(self.aggregate_layout, registration, trace_groups)?;
        let alphas = self
            .alphas
            .get(lane)
            .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
        let mix = self
            .mixes
            .get(lane)
            .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
        let fixed = row_at_v1(self.fixed_lde, query_index)
            .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(self.lde_root.pow(query_index as u128));
        let composition = quotient_value_v1(
            self.layout,
            self.logical_active_rows,
            x,
            opening.base_current,
            opening.base_next,
            opening.aux_current,
            opening.aux_next,
            &fixed,
            self.io_challenges,
            alphas,
        )
        .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        if opening.base_current.len() != mix.base.len()
            || opening.aux_current.len() != mix.aux.len()
        {
            return Err(AggregateStarkErrorV1::ConstraintOpening);
        }
        let mixed_base = opening
            .base_current
            .iter()
            .zip(&mix.base)
            .fold(E::ZERO, |sum, (value, coefficient)| {
                sum.add(coefficient.mul_base(*value))
            });
        let mixed_aux = opening
            .aux_current
            .iter()
            .zip(&mix.aux)
            .fold(E::ZERO, |sum, (value, coefficient)| {
                sum.add(coefficient.mul_base(*value))
            });
        Ok(aggregate::AggregateExpectedOpeningV1 {
            composition,
            fri_base: mixed_base
                .add(mixed_aux)
                .add(mix_opened_composition_chunks_v1(composition_chunks, mix)?),
        })
    }
}
#[cfg(test)]
pub(super) struct ProjectionOpenedRowEvaluatorV1<'a> {
    pub(super) aggregate_layout: &'a AggregateProofLayoutV1,
    pub(super) layout: SegmentLayoutV1,
    pub(super) fixed_lde: &'a [Vec<F>],
    pub(super) challenges: ZkX509ProjectionChallengesV1,
    pub(super) alphas: &'a [Vec<E>],
    pub(super) mixes: &'a [FriMixV1],
    pub(super) lde_root: F,
}
#[cfg(test)]
pub(super) struct P256OpenedRowEvaluatorV1<'a> {
    pub(super) material: &'a P256OpenedMaterialV1,
    pub(super) challenges: P256AggregateChallengesV1,
    pub(super) alphas: &'a [Vec<Vec<E>>],
    pub(super) mixes: &'a [Vec<FriMixV1>],
    pub(super) lde_root: F,
}
pub(super) fn p256_scalar_opened_residues_v1(
    registration: RegisteredSegmentLayoutV1,
    opening: RegisteredOpenedRowsV1<'_>,
    fixed: &[F; P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1],
    challenges: P256ScalarBitBusChallengesV1,
    terminals: &P256TerminalRegistrationV1,
) -> Result<Vec<F>, ZkX509StarkErrorV1> {
    let Some((_, 0)) = p256_instance_parts_v1(registration.segment.instance) else {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    };
    if registration.segment.adapter != SegmentAdapterIdV1::P256ScalarBitBus
        || registration.segment.trace_log2 != P256_SCALAR_BIT_BUS_AGGREGATE_TRACE_LOG2_V1
        || registration.segment.base_width != P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1
        || registration.segment.aux_width != P256_SCALAR_BIT_BUS_STARK_AUX_WIDTH_V1
        || registration.segment.fixed_width != P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1
        || registration.segment.constraint_count
            != P256_SCALAR_BIT_BUS_REGISTERED_CONSTRAINT_COUNT_V1
        || opening
            .base_current
            .iter()
            .chain(opening.base_next)
            .chain(opening.aux_current)
            .chain(opening.aux_next)
            .chain(fixed)
            .any(|value| F::canonical(value.0).is_none())
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    challenges
        .validate_v1()
        .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?;
    let current: &[F; P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1] = opening
        .base_current
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let next: &[F; P256_SCALAR_BIT_BUS_STARK_BASE_WIDTH_V1] = opening
        .base_next
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let current_aux: &[F; P256_SCALAR_BIT_BUS_STARK_AUX_WIDTH_V1] = opening
        .aux_current
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let next_aux: &[F; P256_SCALAR_BIT_BUS_STARK_AUX_WIDTH_V1] = opening
        .aux_next
        .try_into()
        .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
    let mut residues = evaluate_p256_scalar_bit_bus_aggregate_residues_v1(
        current,
        next,
        current_aux,
        next_aux,
        fixed,
        challenges,
    )?;
    if residues.len() != P256_SCALAR_BIT_BUS_STARK_CONSTRAINT_COUNT_V1 {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    let terminal_bindings = evaluate_p256_scalar_source_terminal_openings_v1(
        p256_scalar_bit_bus_stark_last_active_selector_v1(fixed),
        terminals.buses.arithmetic_scalar,
        terminals.buses.window_scalar,
        p256_scalar_bit_bus_opened_terminals_v1(current_aux),
    );
    if terminal_bindings.len() != 2 * P256_SCALAR_BIT_BUS_LANES_V1 {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    residues.extend(terminal_bindings);
    if residues.len() != P256_SCALAR_BIT_BUS_REGISTERED_CONSTRAINT_COUNT_V1 {
        return Err(ZkX509StarkErrorV1::InternalInvariant);
    }
    Ok(residues)
}
pub(super) fn p256_opened_residues_v1(
    registration: RegisteredSegmentLayoutV1,
    opening: RegisteredOpenedRowsV1<'_>,
    fixed: &[F],
    challenges: P256AggregateChallengesV1,
    terminals: &P256TerminalRegistrationV1,
) -> Result<Vec<F>, ZkX509StarkErrorV1> {
    let (_, local_instance) = p256_instance_parts_v1(registration.segment.instance)
        .ok_or(ZkX509StarkErrorV1::ProfileMismatch)?;
    let mut residues = match (registration.segment.adapter, local_instance) {
        (SegmentAdapterIdV1::P256Reduction, instance @ 0..=1) => {
            let claim_role = if instance == 0 {
                P256CrossTraceTerminalRoleV1::DigestReduction
            } else {
                P256CrossTraceTerminalRoleV1::ResultXReduction
            };
            let claim = terminals.cross_claim(claim_role)?;
            let current: &[F; P256_REDUCTION_BASE_WIDTH_V1] = opening
                .base_current
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let next: &[F; P256_REDUCTION_BASE_WIDTH_V1] = opening
                .base_next
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let current_aux: &[F; P256_REDUCTION_AGGREGATE_AUX_WIDTH_V1] = opening
                .aux_current
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let next_aux: &[F; P256_REDUCTION_AGGREGATE_AUX_WIDTH_V1] = opening
                .aux_next
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let fixed: &[F; P256_REDUCTION_AGGREGATE_FIXED_WIDTH_V1] = fixed
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let mut residues = evaluate_p256_reduction_aggregate_residues_v1(
                current,
                next,
                current_aux,
                next_aux,
                fixed,
                claim.start,
                challenges.cross,
            )?;
            residues.extend(evaluate_p256_terminal_claim_binding_v1(
                p256_reduction_last_selector_v1(fixed),
                p256_reduction_cross_terminal_v1(current_aux)?,
                claim.terminal,
            ));
            residues
        }
        (SegmentAdapterIdV1::P256LowS, 0) => {
            let claim = terminals.cross_claim(P256CrossTraceTerminalRoleV1::WalletLowS)?;
            let current: &[F; P256_LOW_S_BASE_WIDTH_V1] = opening
                .base_current
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let next: &[F; P256_LOW_S_BASE_WIDTH_V1] = opening
                .base_next
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let current_aux: &[F; P256_LOW_S_AGGREGATE_AUX_WIDTH_V1] = opening
                .aux_current
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let next_aux: &[F; P256_LOW_S_AGGREGATE_AUX_WIDTH_V1] = opening
                .aux_next
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let fixed: &[F; P256_LOW_S_AGGREGATE_FIXED_WIDTH_V1] = fixed
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let mut residues = evaluate_p256_low_s_aggregate_residues_v1(
                current,
                next,
                current_aux,
                next_aux,
                fixed,
                claim.start,
                challenges.cross,
            )?;
            residues.extend(evaluate_p256_terminal_claim_binding_v1(
                p256_low_s_last_selector_v1(fixed),
                p256_low_s_cross_terminal_v1(current_aux)?,
                claim.terminal,
            ));
            residues
        }
        (SegmentAdapterIdV1::P256ScalarBitBus, 0) => {
            let fixed: &[F; P256_SCALAR_BIT_BUS_STARK_FIXED_WIDTH_V1] = fixed
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            p256_scalar_opened_residues_v1(
                registration,
                opening,
                fixed,
                challenges.scalar,
                terminals,
            )?
        }
        (SegmentAdapterIdV1::P256Window, 0) => {
            let claim = terminals.cross_claim(P256CrossTraceTerminalRoleV1::WindowBatch)?;
            let current: &[F; P256_WINDOW_BASE_WIDTH_V1] = opening
                .base_current
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let next: &[F; P256_WINDOW_BASE_WIDTH_V1] = opening
                .base_next
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let current_aux: &[F; P256_WINDOW_AGGREGATE_AUX_WIDTH_V1] = opening
                .aux_current
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let next_aux: &[F; P256_WINDOW_AGGREGATE_AUX_WIDTH_V1] = opening
                .aux_next
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let fixed: &[F; P256_WINDOW_AGGREGATE_FIXED_WIDTH_V1] = fixed
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let mut residues = evaluate_p256_window_aggregate_residues_v1(
                current,
                next,
                current_aux,
                next_aux,
                fixed,
                P256WindowAggregateChallengesV1 {
                    cross_start: claim.start,
                    cross: challenges.cross,
                    scalar: challenges.scalar,
                },
            )?;
            let selector = p256_window_last_selector_v1(fixed);
            residues.extend(evaluate_p256_terminal_claim_binding_v1(
                selector,
                p256_window_cross_terminal_v1(current_aux)?,
                claim.terminal,
            ));
            residues.extend(evaluate_p256_terminal_claim_binding_v1(
                selector,
                p256_window_scalar_terminal_v1(current_aux)?,
                terminals.buses.window_scalar,
            ));
            residues
        }
        (SegmentAdapterIdV1::P256ValueBus, 2) => {
            let current: &[F; P256_BINDING_SINK_BASE_WIDTH_V1] = opening
                .base_current
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let next: &[F; P256_BINDING_SINK_BASE_WIDTH_V1] = opening
                .base_next
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let current_aux: &[
                F;
                super::super::p256_cross_trace_bus::P256_CROSS_TRACE_SINK_AUX_WIDTH_V1
            ] =
                opening
                    .aux_current
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let next_aux: &[
                F;
                super::super::p256_cross_trace_bus::P256_CROSS_TRACE_SINK_AUX_WIDTH_V1
            ] =
                opening
                    .aux_next
                    .try_into()
                    .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let fixed: &[F; P256_BINDING_SINK_FIXED_WIDTH_V1] = fixed
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let mut residues = evaluate_p256_binding_sink_aggregate_residues_v1(
                current,
                next,
                current_aux,
                next_aux,
                fixed,
                challenges.cross,
            )?;
            residues.extend(evaluate_p256_terminal_claim_binding_v1(
                p256_binding_sink_last_selector_v1(fixed),
                p256_binding_sink_terminal_v1(current_aux)?,
                terminals.sink,
            ));
            residues
        }
        (SegmentAdapterIdV1::P256Arithmetic, 0) => {
            let current: &[F; P256_ARITHMETIC_BASE_WIDTH_V1] = opening
                .base_current
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let next: &[F; P256_ARITHMETIC_BASE_WIDTH_V1] = opening
                .base_next
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let current_aux: &[F; P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1] = opening
                .aux_current
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let next_aux: &[F; P256_ARITHMETIC_AGGREGATE_AUX_WIDTH_V1] = opening
                .aux_next
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let fixed: &[F; P256_ARITHMETIC_AGGREGATE_FIXED_WIDTH_V1] = fixed
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let mut residues = evaluate_p256_arithmetic_aggregate_residues_v1(
                current,
                next,
                current_aux,
                next_aux,
                fixed,
                challenges.scalar,
                challenges.arithmetic_copy,
            )?;
            let selector = p256_arithmetic_last_selector_v1(fixed);
            residues.extend(evaluate_p256_terminal_claim_binding_v1(
                selector,
                p256_arithmetic_scalar_terminal_v1(current_aux)?,
                terminals.buses.arithmetic_scalar,
            ));
            residues.extend(evaluate_p256_terminal_claim_binding_v1(
                selector,
                p256_arithmetic_value_copy_terminal_v1(current_aux)?,
                terminals.buses.arithmetic_value_copy,
            ));
            residues
        }
        (SegmentAdapterIdV1::P256ValueBus, 0) => {
            let claim = terminals.cross_claim(P256CrossTraceTerminalRoleV1::ValueWriter)?;
            let current: &[F; P256_VALUE_BUS_STARK_BASE_WIDTH_V1] = opening
                .base_current
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let next: &[F; P256_VALUE_BUS_STARK_BASE_WIDTH_V1] = opening
                .base_next
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let current_aux: &[F; P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1] = opening
                .aux_current
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let next_aux: &[F; P256_VALUE_EXECUTION_AGGREGATE_AUX_WIDTH_V1] = opening
                .aux_next
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let fixed: &[F; P256_VALUE_EXECUTION_AGGREGATE_FIXED_WIDTH_V1] = fixed
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let mut residues = evaluate_p256_value_execution_aggregate_residues_v1(
                current,
                next,
                current_aux,
                next_aux,
                fixed,
                P256ValueExecutionAggregateChallengesV1 {
                    value: challenges.value,
                    cross: challenges.cross,
                    arithmetic_copy: challenges.arithmetic_copy,
                },
            )?;
            let selector = p256_value_execution_last_selector_v1(fixed);
            let value_aux: &[F; P256_VALUE_BUS_STARK_AUX_WIDTH_V1] = current_aux
                [..P256_VALUE_BUS_STARK_AUX_WIDTH_V1]
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            residues.extend(evaluate_p256_terminal_claim_binding_v1(
                selector,
                p256_value_bus_stark_opened_terminal_v1(value_aux),
                terminals.buses.value_execution,
            ));
            residues.extend(evaluate_p256_terminal_claim_binding_v1(
                selector,
                p256_value_execution_arithmetic_copy_terminal_v1(current_aux)?,
                terminals.buses.value_arithmetic_copy,
            ));
            residues.extend(evaluate_p256_terminal_claim_binding_v1(
                selector,
                p256_value_execution_cross_terminal_v1(current_aux)?,
                claim.terminal,
            ));
            residues
        }
        (SegmentAdapterIdV1::P256ValueBus, 1) => {
            let current: &[F; P256_VALUE_BUS_STARK_BASE_WIDTH_V1] = opening
                .base_current
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let next: &[F; P256_VALUE_BUS_STARK_BASE_WIDTH_V1] = opening
                .base_next
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let current_aux: &[F; P256_VALUE_BUS_STARK_AUX_WIDTH_V1] = opening
                .aux_current
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let next_aux: &[F; P256_VALUE_BUS_STARK_AUX_WIDTH_V1] = opening
                .aux_next
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let fixed: &[F; P256_VALUE_BUS_STARK_FIXED_WIDTH_V1] = fixed
                .try_into()
                .map_err(|_| ZkX509StarkErrorV1::ProfileMismatch)?;
            let mut residues = evaluate_p256_value_bus_stark_residues_v1(
                current,
                next,
                current_aux,
                next_aux,
                fixed,
                challenges.value,
            )
            .map_err(|_| ZkX509StarkErrorV1::P256Witness)?;
            residues.extend(evaluate_p256_terminal_claim_binding_v1(
                p256_value_bus_stark_last_domain_selector_v1(fixed),
                p256_value_bus_stark_opened_terminal_v1(current_aux),
                terminals.buses.value_sorted,
            ));
            residues
        }
        _ => return Err(ZkX509StarkErrorV1::ProfileMismatch),
    };
    if residues.len() != registration.segment.constraint_count {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    Ok(core::mem::take(&mut residues))
}
#[cfg(test)]
impl aggregate::AggregateOpenedRowEvaluatorV1 for P256OpenedRowEvaluatorV1<'_> {
    fn evaluate_opened_row_v1(
        &mut self,
        query_index: usize,
        lane: usize,
        trace_groups: &[aggregate::AggregateOpenedTraceGroupV1],
        composition_chunks: &[E],
    ) -> Result<aggregate::AggregateExpectedOpeningV1, AggregateStarkErrorV1> {
        self.material
            .validate()
            .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        self.challenges
            .validate()
            .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        let aggregate_layout = &self.material.registration.layout;
        if lane >= SECURITY_LANES
            || self.alphas.len() != aggregate_layout.registered_segments.len()
            || self.mixes.len() != aggregate_layout.trace_groups.len()
        {
            return Err(AggregateStarkErrorV1::ConstraintOpening);
        }
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(self.lde_root.pow(query_index as u128));
        let mut composition = E::ZERO;
        let mut fri_base = E::ZERO;
        for (registration_index, registration) in aggregate_layout
            .registered_segments
            .iter()
            .copied()
            .enumerate()
        {
            let opening = registered_opened_rows_v1(aggregate_layout, registration, trace_groups)?;
            let fixed = self.material.fixed_openings[registration_index]
                .get(&query_index)
                .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
            let alphas = self.alphas[registration_index]
                .get(lane)
                .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
            let residues = p256_opened_residues_v1(
                registration,
                opening,
                fixed,
                self.challenges,
                &self.material.terminals,
            )
            .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
            let local_composition =
                accumulator_quotient_value_v1(registration.segment, x, &residues, alphas)
                    .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
            let mix = self
                .mixes
                .get(registration.trace_group)
                .and_then(|lanes| lanes.get(lane))
                .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
            let base_end = registration
                .base_end()
                .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
            let aux_end = registration
                .aux_end()
                .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
            let base_mix = mix
                .base
                .get(registration.base_start..base_end)
                .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
            let aux_mix = mix
                .aux
                .get(registration.aux_start..aux_end)
                .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
            if opening.base_current.len() != base_mix.len()
                || opening.aux_current.len() != aux_mix.len()
            {
                return Err(AggregateStarkErrorV1::ConstraintOpening);
            }
            let mixed_base = opening
                .base_current
                .iter()
                .zip(base_mix)
                .fold(E::ZERO, |sum, (value, coefficient)| {
                    sum.add(coefficient.mul_base(*value))
                });
            let mixed_aux = opening
                .aux_current
                .iter()
                .zip(aux_mix)
                .fold(E::ZERO, |sum, (value, coefficient)| {
                    sum.add(coefficient.mul_base(*value))
                });
            composition = composition.add(local_composition);
            fri_base = fri_base.add(mixed_base).add(mixed_aux);
        }
        let composition_mix = self
            .mixes
            .first()
            .and_then(|lanes| lanes.get(lane))
            .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
        if self.mixes.iter().any(|lanes| {
            lanes.get(lane).map(|mix| &mix.composition) != Some(&composition_mix.composition)
        }) {
            return Err(AggregateStarkErrorV1::ConstraintOpening);
        }
        fri_base = fri_base.add(mix_opened_composition_chunks_v1(
            composition_chunks,
            composition_mix,
        )?);
        Ok(aggregate::AggregateExpectedOpeningV1 {
            composition,
            fri_base,
        })
    }
}
#[cfg(test)]
impl aggregate::AggregateOpenedRowEvaluatorV1 for ProjectionOpenedRowEvaluatorV1<'_> {
    fn evaluate_opened_row_v1(
        &mut self,
        query_index: usize,
        lane: usize,
        trace_groups: &[aggregate::AggregateOpenedTraceGroupV1],
        composition_chunks: &[E],
    ) -> Result<aggregate::AggregateExpectedOpeningV1, AggregateStarkErrorV1> {
        let registration = self
            .aggregate_layout
            .registered_segment(SegmentAdapterIdV1::Projection, 0)
            .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        if registration.segment != self.layout {
            return Err(AggregateStarkErrorV1::ConstraintOpening);
        }
        let opening = registered_opened_rows_v1(self.aggregate_layout, registration, trace_groups)?;
        let alphas = self
            .alphas
            .get(lane)
            .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
        let mix = self
            .mixes
            .get(lane)
            .ok_or(AggregateStarkErrorV1::ConstraintOpening)?;
        let fixed = row_at_v1(self.fixed_lde, query_index)
            .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        let x = F(GOLDILOCKS_GENERATOR_V1).mul(self.lde_root.pow(query_index as u128));
        let composition = projection_quotient_value_v1(
            self.layout,
            x,
            opening.base_current,
            opening.base_next,
            opening.aux_current,
            opening.aux_next,
            &fixed,
            self.challenges,
            alphas,
        )
        .map_err(|_| AggregateStarkErrorV1::ConstraintOpening)?;
        if opening.base_current.len() != mix.base.len()
            || opening.aux_current.len() != mix.aux.len()
        {
            return Err(AggregateStarkErrorV1::ConstraintOpening);
        }
        let mixed_base = opening
            .base_current
            .iter()
            .zip(&mix.base)
            .fold(E::ZERO, |sum, (value, coefficient)| {
                sum.add(coefficient.mul_base(*value))
            });
        let mixed_aux = opening
            .aux_current
            .iter()
            .zip(&mix.aux)
            .fold(E::ZERO, |sum, (value, coefficient)| {
                sum.add(coefficient.mul_base(*value))
            });
        Ok(aggregate::AggregateExpectedOpeningV1 {
            composition,
            fri_base: mixed_base
                .add(mixed_aux)
                .add(mix_opened_composition_chunks_v1(composition_chunks, mix)?),
        })
    }
}
fn main_pre_aux_from_decoded_proof_v1(
    public: ZkX509CredentialPublicBindingV1,
    verifier_profile: ZkX509MainVerifierProfileV1,
    layout: &AggregateProofLayoutV1,
    proof: &ZkX509SegmentedStarkProofV1,
) -> Result<ZkX509CredentialMainPreAuxV1, ZkX509StarkErrorV1> {
    layout.validate_exact_full_profile_registration_v1()?;
    validate_zk_x509_main_verifier_profile_v1(verifier_profile)?;
    if public.consensus_context_digest == [0_u8; 32]
        || proof.aggregate.trace_groups.len() != FULL_PROFILE_TRACE_GROUPS_V1
    {
        return Err(ZkX509StarkErrorV1::ProfileMismatch);
    }
    let mut session = ZkX509MainBaseCommitmentSessionV1::new_v1(
        layout,
        public.consensus_context_digest,
        verifier_profile,
    )?;
    session.accept_decoded_base_groups_v1(&proof.aggregate.trace_groups)?;
    session.finish_pre_aux_v1()
}
/// Decode the canonical MAIN proof and mint its verifier-owned X5B1 input.
///
/// This performs the exact aggregate shape decode before exposing the opaque
/// pre-auxiliary token. The returned value contains no proof-selected
/// challenge and can only be consumed by the joint MAIN-plus-CA transcript.
pub(crate) fn zk_x509_main_pre_aux_from_proof_v1(
    public: ZkX509CredentialPublicBindingV1,
    proof_bytes: &[u8],
) -> Result<ZkX509CredentialMainPreAuxV1, ZkX509StarkErrorV1> {
    let verifier_profile = construct_zk_x509_main_verifier_profile_v1()?;
    let layout = AggregateProofLayoutV1::for_full_profile_v1()?;
    let envelope = decode_zk_x509_main_proof_envelope_v1(proof_bytes)?;
    let proof = decode_zk_x509_segmented_stark_proof_v1(envelope.aggregate_proof, &layout)?;
    main_pre_aux_from_decoded_proof_v1(public, verifier_profile, &layout, &proof)
}
/// Verify the complete six-group, 49-registration canonical MAIN aggregate.
///
/// Every fixed opening is reconstructed after post-grinding query derivation. The proof supplies
/// only authenticated trace/composition/FRI openings and terminal claims; it cannot select a
/// provider, registration, schedule, fixed row, or shared X5B1 challenge.
#[allow(clippy::too_many_lines)]
pub(crate) fn verify_zk_x509_main_aggregate_stark_v1(
    statement: &IrohaZkX509StarkP256StatementV1,
    rfc_statement: &ZkX509Rfc5280StatementV1,
    public: ZkX509CredentialPublicBindingV1,
    credential_binding: ZkX509CredentialPreAuxBindingV1,
    proof_bytes: &[u8],
) -> Result<ZkX509MainCaBindingV1, ZkX509StarkErrorV1> {
    let verifier_profile = construct_zk_x509_main_verifier_profile_v1()?;
    let layout = AggregateProofLayoutV1::for_full_profile_v1()?;
    let envelope = decode_zk_x509_main_proof_envelope_v1(proof_bytes)?;
    let proof = decode_zk_x509_segmented_stark_proof_v1(envelope.aggregate_proof, &layout)?;
    let main_pre_aux =
        main_pre_aux_from_decoded_proof_v1(public, verifier_profile, &layout, &proof)?;
    if !credential_binding.matches_main_pre_aux_v1(main_pre_aux) {
        return Err(ZkX509StarkErrorV1::TranscriptMismatch);
    }
    let mut transcript =
        new_main_transcript_v1(&public.consensus_context_digest, verifier_profile)?;
    absorb_aggregate_layout_v1(&mut transcript, MAIN_LAYOUT_DOMAIN_V1, &layout)?;
    aggregate::absorb_base_roots_v1(
        &mut transcript,
        AGGREGATE_DOMAINS_V1,
        &proof.aggregate.trace_groups,
    )
    .map_err(map_aggregate_error_v1)?;
    absorb_zk_x509_credential_pre_aux_binding_v1(&mut transcript, credential_binding)
        .map_err(map_credential_pre_aux_error_v1)?;
    aggregate::absorb_aux_roots_v1(
        &mut transcript,
        AGGREGATE_DOMAINS_V1,
        &proof.aggregate.trace_groups,
    )
    .map_err(map_aggregate_error_v1)?;
    absorb_zk_x509_main_terminal_claims_v1(&mut transcript, envelope.claims)?;
    let alphas = derive_constraint_alphas_v1(&mut transcript, &layout)?;
    aggregate::absorb_composition_roots_v1(
        &mut transcript,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &proof.aggregate.composition_roots,
    )
    .map_err(map_aggregate_error_v1)?;
    aggregate::absorb_fri_mask_roots_v1(
        &mut transcript,
        AGGREGATE_PARAMETERS_V1,
        &proof.aggregate.fri_mask_roots,
    )
    .map_err(map_aggregate_error_v1)?;
    let shared_layout = layout.as_shared()?;
    let deep_point =
        aggregate::derive_deep_point_v1(&mut transcript, AGGREGATE_PARAMETERS_V1, &shared_layout)
            .map_err(map_aggregate_error_v1)?;
    aggregate::absorb_deep_openings_v1(
        &mut transcript,
        &proof.deep,
        AGGREGATE_PARAMETERS_V1,
        &shared_layout,
    )
    .map_err(map_aggregate_error_v1)?;
    let mixes = derive_fri_mixes_v1(&mut transcript, &layout)?;
    let deep_mixes = aggregate_deep_lane_mixes_v1(&mixes, &layout)?;
    let (fri_betas, terminal_fields) = aggregate::verify_fri_commitments_v1(
        &proof.aggregate,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &shared_layout,
        &mut transcript,
    )
    .map_err(map_aggregate_error_v1)?;
    let grinding_state = transcript.state();
    verify_grinding_nonce_v1(
        &grinding_state,
        ZK_X509_GRINDING_BITS_V1,
        proof.aggregate.grinding_nonce,
    )
    .map_err(|_| ZkX509StarkErrorV1::TranscriptMismatch)?;
    absorb_grinding_nonce_v1(&mut transcript, proof.aggregate.grinding_nonce)?;
    let expected_indices = query_indices_v1(&transcript, &layout)?;
    aggregate::verify_all_merkle_openings_v1(
        &proof.aggregate,
        AGGREGATE_PARAMETERS_V1,
        AGGREGATE_DOMAINS_V1,
        &shared_layout,
        &expected_indices,
    )
    .map_err(map_aggregate_error_v1)?;
    let sha_shape = ZkX509ShaCallPublicShapeV1 {
        disclosed_attributes: rfc_statement.disclosed_attribute_indices.len(),
    };
    let derived_fixed = derive_zk_x509_main_fixed_openings_after_grinding_v1(
        verifier_profile,
        sha_shape,
        &expected_indices,
    )?;
    let post_base = credential_binding.main_post_base();
    let p256_fixed = P256MainVerifierFixedSourceV1::new_v1()?;
    let mut log5 = MainP256Log5VerifierConstraintSourceV1::for_main_v1(
        &layout,
        &p256_fixed,
        post_base,
        envelope.claims.p256,
    )?;
    let mut scalar = MainP256ScalarVerifierConstraintSourceV1::for_main_v1(
        &layout,
        &p256_fixed,
        post_base,
        &envelope.claims.p256,
    )?;
    let mut projection =
        MainProjectionVerifierConstraintSourceV1::for_main_v1(&layout, statement, post_base)?;
    let mut log16 = MainP256Log16VerifierConstraintSourceV1::for_main_v1(
        &layout,
        &p256_fixed,
        post_base,
        &envelope.claims.p256,
    )?;
    let mut io = MainIoVerifierConstraintSourceV1::for_main_v1(&layout, statement, post_base)?;
    let mut log19 = MainLog19VerifierConstraintSourceV1::for_main_v1(
        &layout,
        rfc_statement,
        post_base,
        envelope.claims,
    )?;
    log19.install_verifier_derived_fixed_openings_v1(derived_fixed)?;
    let mut providers = MainOpenedProviderSetV1::new_v1(
        &layout,
        vec![
            MainOpenedGroupProviderV1::Log5(&mut log5),
            MainOpenedGroupProviderV1::P256Scalar(&mut scalar),
            MainOpenedGroupProviderV1::Projection(&mut projection),
            MainOpenedGroupProviderV1::Log16(&mut log16),
            MainOpenedGroupProviderV1::Io(&mut io),
            MainOpenedGroupProviderV1::Log19(&mut log19),
        ],
    )?;
    let mut evaluator = MainOpenedRowEvaluatorV1 {
        providers: &mut providers,
        alphas: &alphas,
        mixes: &mixes,
    };
    aggregate::verify_opened_query_relations_with_deep_v1(
        &proof.aggregate,
        &proof.deep,
        deep_point,
        &deep_mixes,
        AGGREGATE_PARAMETERS_V1,
        &shared_layout,
        &expected_indices,
        &fri_betas,
        &terminal_fields,
        &mut evaluator,
    )
    .map_err(map_aggregate_error_v1)?;
    Ok(ZkX509MainCaBindingV1 {
        public,
        sha_terminals: envelope.claims.sha.credential_call_terminals_v1(),
        root_spki_consumer_products: envelope
            .claims
            .rfc5280
            .governed_trust_anchor_products_v1()
            .consumer_products,
    })
}
