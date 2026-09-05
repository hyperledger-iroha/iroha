//! Compact paired-Pasta transport deciders for Kagemusha mint proofs.
//!
//! Each parity verifies an actual ordinary inner proof under constants pinned
//! by the outer verifying key, then folds that proof's current opening claim
//! into its authenticated prior history. The reciprocal parity binds the
//! private inner audit tuple and evaluates all deferred curve equations. No
//! host-side certificate check or supplied accumulator replaces these steps.
//!
//! Mint authorization preserves transport cells 0..46 and privately cross-binds
//! the appended carrier-commitment limbs from both inner semantic columns.
//! Mint authority preserves cells 0..16, including the *outer* protocol
//! identities, and cells 20..22, containing the Eq audit pair commitment
//! already proved by the inner relation. That audit absorbs the complete inner
//! semantic/protocol/history transcript and the Ep audit. Only the four audit
//! cells and 34 history cells acquire an outer meaning.
//!
//! TODO: Validate the integrated generation, native-verifier, checkpoint, and artifact paths
//! with actual K16 mint proofs, splice tests, and resource/transport measurements. Source
//! integration and local layout tests are not that qualification.

use halo2_base::{
    AssignedValue,
    gates::{
        RangeInstructions as _,
        circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
    },
    utils::{BigPrimeField, CurveAffineExt, ScalarField},
};
use halo2_proofs::{
    circuit::{Layouter, V1},
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error as PlonkError},
    poly::{commitment::Params as _, ipa::commitment::ParamsIPA},
};
use snark_verifier::{
    loader::native::NativeLoader,
    pcs::ipa::{IpaAccumulator, IpaSuccinctVerifyingKey},
    verifier::plonk::PlonkProtocol,
};

use super::{
    KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1, KAGEMUSHA_RECURSION_IPA_K_V1, KagemushaPastaParityV1,
    composite::{assigned_digest_bytes, ep_succinct_vk, eq_succinct_vk},
    deferred_parent::{
        DeferredLoader, KagemushaDeferredParentOutputV1, accumulator_limb_count,
        bind_accumulator_limbs, constrain_reciprocal_output_with_u128_binding_serialized_v1,
        deferred_field_chips_v1, deferred_loader_v1,
        finalize_tagged_deferred_audit_with_u128_binding_v1, load_native_accumulator,
        ordinary_ipa_proof_profile_v1, verify_fold, verify_hybrid_ordinary_proof_and_stream_v1,
        verify_ordinary_proof_v1,
    },
    mint_authority::{
        KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1,
        public_instance as authority_public_instance,
    },
    mint_authorization::{
        MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1,
        MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1,
        public_instance as authorization_public_instance,
    },
};

const MINIMUM_UNUSABLE_ROWS: usize = 9;
const MINT_AUTHORIZATION_TRANSPORT_EQUATION_TAG_V1: u32 = 7;
const MINT_AUTHORITY_TRANSPORT_EQUATION_TAG_V1: u32 = 8;
const MINT_AUTHORIZATION_EQ_CARRIER_COMMITMENT_LO_V1: usize =
    MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1;
const MINT_AUTHORIZATION_EP_CARRIER_COMMITMENT_LO_V1: usize =
    MINT_AUTHORIZATION_EQ_CARRIER_COMMITMENT_LO_V1 + 2;

/// Complete public column of a compact recipient-authorization proof.
pub(super) const KAGEMUSHA_MINT_AUTHORIZATION_TRANSPORT_PUBLIC_INSTANCE_COUNT_V1: usize = 84;
/// Complete public column of a compact reserve/finality authority proof.
pub(super) const KAGEMUSHA_MINT_AUTHORITY_TRANSPORT_PUBLIC_INSTANCE_COUNT_V1: usize = 56;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MintTransportFamilyV1 {
    Authorization,
    Authority,
}

impl MintTransportFamilyV1 {
    const fn public_count(self) -> usize {
        match self {
            Self::Authorization => KAGEMUSHA_MINT_AUTHORIZATION_TRANSPORT_PUBLIC_INSTANCE_COUNT_V1,
            Self::Authority => KAGEMUSHA_MINT_AUTHORITY_TRANSPORT_PUBLIC_INSTANCE_COUNT_V1,
        }
    }

    const fn history_start(self) -> usize {
        match self {
            Self::Authorization => authorization_public_instance::HISTORY_START,
            Self::Authority => authority_public_instance::HISTORY_START,
        }
    }

    const fn inner_semantic_count(self) -> usize {
        match self {
            Self::Authorization => MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1,
            Self::Authority => KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1,
        }
    }

    const fn carrier_commitment_limb_indices(
        self,
        parity: KagemushaPastaParityV1,
    ) -> Option<[usize; 2]> {
        match (self, parity) {
            (Self::Authorization, KagemushaPastaParityV1::Eq) => Some([
                MINT_AUTHORIZATION_EQ_CARRIER_COMMITMENT_LO_V1,
                MINT_AUTHORIZATION_EQ_CARRIER_COMMITMENT_LO_V1 + 1,
            ]),
            (Self::Authorization, KagemushaPastaParityV1::Ep) => Some([
                MINT_AUTHORIZATION_EP_CARRIER_COMMITMENT_LO_V1,
                MINT_AUTHORIZATION_EP_CARRIER_COMMITMENT_LO_V1 + 1,
            ]),
            (Self::Authority, _) => None,
        }
    }

    const fn carrier_binding_range(self) -> Option<std::ops::Range<usize>> {
        match self {
            Self::Authorization => Some(
                MINT_AUTHORIZATION_EQ_CARRIER_COMMITMENT_LO_V1
                    ..MINT_AUTHORIZATION_EP_CARRIER_COMMITMENT_LO_V1 + 2,
            ),
            Self::Authority => None,
        }
    }

    const fn eq_audit_start(self) -> usize {
        match self {
            Self::Authorization => authorization_public_instance::EQ_AUDIT_LO,
            Self::Authority => authority_public_instance::EQ_AUDIT_LO,
        }
    }

    const fn ep_audit_start(self) -> usize {
        match self {
            Self::Authorization => authorization_public_instance::EP_AUDIT_LO,
            Self::Authority => authority_public_instance::EP_AUDIT_LO,
        }
    }

    const fn inner_binding_indices(self) -> [usize; 4] {
        let eq = self.eq_audit_start();
        let ep = self.ep_audit_start();
        [eq, eq + 1, ep, ep + 1]
    }

    fn copies_inner_cell(self, index: usize) -> bool {
        index < self.history_start() && !self.inner_binding_indices().contains(&index)
    }

    const fn equation_tag(self) -> u32 {
        match self {
            Self::Authorization => MINT_AUTHORIZATION_TRANSPORT_EQUATION_TAG_V1,
            Self::Authority => MINT_AUTHORITY_TRANSPORT_EQUATION_TAG_V1,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Authorization => "mint-authorization transport",
            Self::Authority => "mint-authority transport",
        }
    }
}

/// One parity's authenticated inner mint proof and public compact statement.
#[derive(Clone, Copy)]
pub(super) struct KagemushaMintTransportParityWitnessV1<'a, C: CurveAffineExt> {
    /// Inner protocol loaded entirely as outer-circuit constants, never as a witness VK.
    pub(super) inner_protocol: &'a PlonkProtocol<C>,
    /// Exact public columns authenticated by the inner proof.
    ///
    /// Mint authorization supplies `[semantic, wide carrier]`; mint authority
    /// retains its legacy single public column.
    pub(super) inner_instances: &'a [Vec<C::ScalarExt>],
    /// Exact ordinary inner proof, with no fabricated bootstrap transcript.
    pub(super) inner_proof: &'a [u8],
    /// Prior history bound to the inner public history tail.
    pub(super) inner_history: &'a IpaAccumulator<C, NativeLoader>,
    /// Two-input IPA-AS proof folding the current inner claim into its prior history.
    pub(super) inner_history_fold_proof: &'a [u8],
    /// Compact public column whose history tail is that fold's successor.
    pub(super) outer_instances: &'a [C::ScalarExt],
}

/// Both mutually audited parities of one compact mint proof.
#[derive(Clone, Copy)]
pub(super) struct KagemushaMintTransportDeciderWitnessV1<'a> {
    /// Eq/Fp inner proof and outer statement.
    pub(super) eq: KagemushaMintTransportParityWitnessV1<'a, EqAffine>,
    /// Ep/Fq inner proof and outer statement.
    pub(super) ep: KagemushaMintTransportParityWitnessV1<'a, EpAffine>,
}

/// Base-only configuration shared by the two typed mint transport families.
#[derive(Clone, Debug)]
pub(super) struct KagemushaMintTransportDeciderConfigV1<F: ScalarField> {
    base: BaseConfig<F>,
}

/// Eq/Fp compact recipient-authorization circuit.
#[derive(Clone)]
pub(super) struct KagemushaMintAuthorizationTransportEqCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fp>,
}

/// Ep/Fq compact recipient-authorization circuit.
#[derive(Clone)]
pub(super) struct KagemushaMintAuthorizationTransportEpCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fq>,
}

/// Eq/Fp compact reserve/finality authority circuit.
#[derive(Clone)]
pub(super) struct KagemushaMintAuthorityTransportEqCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fp>,
}

/// Ep/Fq compact reserve/finality authority circuit.
#[derive(Clone)]
pub(super) struct KagemushaMintAuthorityTransportEpCircuitV1 {
    pub(super) builder: BaseCircuitBuilder<Fq>,
}

/// Physical Base inventory, not a proving-key, RSS, or proof-size qualification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct KagemushaMintTransportDeciderCapacityProfileV1 {
    pub(super) k: usize,
    pub(super) domain_rows: usize,
    pub(super) usable_rows: usize,
    pub(super) gate_advice_cells: usize,
    pub(super) gate_advice_columns: usize,
    pub(super) gate_packed_rows: usize,
    pub(super) lookup_advice_cells: usize,
    pub(super) lookup_advice_columns: usize,
    pub(super) lookup_packed_rows: usize,
    pub(super) dense_jobs: usize,
    pub(super) dense_sources: usize,
    pub(super) dense_rows: usize,
    pub(super) max_component_rows: usize,
}

macro_rules! impl_mint_transport_circuit {
    ($circuit:ty, $field:ty, $label:literal) => {
        impl $circuit {
            /// Inventory the configured Base graph without claiming whole-prover feasibility.
            pub(super) fn capacity_profile(
                &self,
            ) -> Result<KagemushaMintTransportDeciderCapacityProfileV1, String> {
                mint_transport_capacity_profile_v1(&self.builder)
            }
        }

        impl Circuit<$field> for $circuit {
            type Config = KagemushaMintTransportDeciderConfigV1<$field>;
            type FloorPlanner = V1;
            type Params = BaseCircuitParams;

            fn params(&self) -> Self::Params {
                self.builder.config_params.clone()
            }

            fn without_witnesses(&self) -> Self {
                Self {
                    builder: self.builder.deep_clone().unknown(true),
                }
            }

            fn configure_with_params(
                meta: &mut ConstraintSystem<$field>,
                params: Self::Params,
            ) -> Self::Config {
                let usable_rows = (1_usize << params.k) - MINIMUM_UNUSABLE_ROWS;
                let mut base = BaseConfig::configure(meta, params);
                base.set_usable_rows(usable_rows);
                KagemushaMintTransportDeciderConfigV1 { base }
            }

            fn configure(_: &mut ConstraintSystem<$field>) -> Self::Config {
                unreachable!(concat!($label, " uses authenticated Base parameters"))
            }

            fn synthesize_for_measurement(
                &self,
                config: Self::Config,
                layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                let result = self.synthesize(config, layouter);
                self.builder.reset_synthesis_state();
                result
            }

            fn synthesize(
                &self,
                config: Self::Config,
                mut layouter: impl Layouter<$field>,
            ) -> Result<(), PlonkError> {
                <BaseCircuitBuilder<$field> as Circuit<$field>>::synthesize(
                    &self.builder,
                    config.base,
                    layouter.namespace(|| concat!($label, " Base")),
                )
            }
        }
    };
}

impl_mint_transport_circuit!(
    KagemushaMintAuthorizationTransportEqCircuitV1,
    Fp,
    "Kagemusha Eq mint-authorization transport"
);
impl_mint_transport_circuit!(
    KagemushaMintAuthorizationTransportEpCircuitV1,
    Fq,
    "Kagemusha Ep mint-authorization transport"
);
impl_mint_transport_circuit!(
    KagemushaMintAuthorityTransportEqCircuitV1,
    Fp,
    "Kagemusha Eq mint-authority transport"
);
impl_mint_transport_circuit!(
    KagemushaMintAuthorityTransportEpCircuitV1,
    Fq,
    "Kagemusha Ep mint-authority transport"
);

struct MintTransportScalarHalfV1<C>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + ScalarField,
{
    builder: BaseCircuitBuilder<C::ScalarExt>,
    output: KagemushaDeferredParentOutputV1<C>,
    inner_binding_cells: Vec<AssignedValue<C::ScalarExt>>,
}

/// Compact reciprocal audit material discovered without retaining either transport graph.
///
/// Eq construction is dropped before Ep construction starts.  The retained values are the
/// native deferred equations and their canonical digest limbs, not either scalar circuit's
/// multi-million-cell advice graph.
pub(super) struct KagemushaMintTransportDeferredAuditsV1 {
    family: MintTransportFamilyV1,
    eq: KagemushaDeferredParentOutputV1<EqAffine>,
    ep: KagemushaDeferredParentOutputV1<EpAffine>,
    eq_digest: [u8; 32],
    ep_digest: [u8; 32],
}

impl KagemushaMintTransportDeferredAuditsV1 {
    #[must_use]
    pub(super) const fn eq_digest(&self) -> [u8; 32] {
        self.eq_digest
    }

    #[must_use]
    pub(super) const fn ep_digest(&self) -> [u8; 32] {
        self.ep_digest
    }
}

fn derive_mint_transport_deferred_audits_v1(
    family: MintTransportFamilyV1,
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintTransportDeciderWitnessV1<'_>,
) -> Result<KagemushaMintTransportDeferredAuditsV1, String> {
    validate_mint_transport_parameter_degrees_v1(eq_params.k(), ep_params.k())?;
    let eq_svk = eq_succinct_vk(eq_params);
    let MintTransportScalarHalfV1 {
        builder: eq_builder,
        output: eq_output,
        inner_binding_cells: _,
    } = build_mint_transport_scalar_half_v1(
        family,
        KagemushaPastaParityV1::Eq,
        &eq_svk,
        witness.eq,
    )?;
    let eq_digest = assigned_digest_bytes(&eq_output.audit_digest_limbs)?;
    drop(eq_builder);
    halo2_proofs::release_allocator_slack();

    let ep_svk = ep_succinct_vk(ep_params);
    let MintTransportScalarHalfV1 {
        builder: ep_builder,
        output: ep_output,
        inner_binding_cells: _,
    } = build_mint_transport_scalar_half_v1(
        family,
        KagemushaPastaParityV1::Ep,
        &ep_svk,
        witness.ep,
    )?;
    let ep_digest = assigned_digest_bytes(&ep_output.audit_digest_limbs)?;
    drop(ep_builder);
    halo2_proofs::release_allocator_slack();

    Ok(KagemushaMintTransportDeferredAuditsV1 {
        family,
        eq: eq_output,
        ep: ep_output,
        eq_digest,
        ep_digest,
    })
}

/// Discover recipient-authorization transport audits one parity at a time.
pub(super) fn derive_kagemusha_mint_authorization_transport_deferred_audits_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintTransportDeciderWitnessV1<'_>,
) -> Result<KagemushaMintTransportDeferredAuditsV1, String> {
    derive_mint_transport_deferred_audits_v1(
        MintTransportFamilyV1::Authorization,
        eq_params,
        ep_params,
        witness,
    )
}

/// Discover reserve/finality-authority transport audits one parity at a time.
pub(super) fn derive_kagemusha_mint_authority_transport_deferred_audits_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintTransportDeciderWitnessV1<'_>,
) -> Result<KagemushaMintTransportDeferredAuditsV1, String> {
    derive_mint_transport_deferred_audits_v1(
        MintTransportFamilyV1::Authority,
        eq_params,
        ep_params,
        witness,
    )
}

fn build_mint_transport_eq_v1(
    family: MintTransportFamilyV1,
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintTransportDeciderWitnessV1<'_>,
    audits: &KagemushaMintTransportDeferredAuditsV1,
) -> Result<(BaseCircuitBuilder<Fp>, Vec<Fp>), String> {
    validate_mint_transport_parameter_degrees_v1(eq_params.k(), ep_params.k())?;
    if audits.family != family {
        return Err("Kagemusha transport audit family does not match Eq circuit".to_owned());
    }
    let public_instances = witness.eq.outer_instances.to_vec();
    let eq_svk = eq_succinct_vk(eq_params);
    let MintTransportScalarHalfV1 {
        builder: mut eq_builder,
        output: eq_output,
        inner_binding_cells: eq_inner_binding_cells,
    } = build_mint_transport_scalar_half_v1(
        family,
        KagemushaPastaParityV1::Eq,
        &eq_svk,
        witness.eq,
    )?;
    bind_own_audit_v1(&mut eq_builder, family.eq_audit_start(), &eq_output)?;
    let expected_ep = public_digest_cells_v1(&eq_builder, family.ep_audit_start())?;
    constrain_reciprocal_output_with_u128_binding_serialized_v1::<EpAffine>(
        &mut eq_builder,
        &audits.ep,
        &expected_ep,
        &eq_inner_binding_cells,
    )?;
    eq_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    if assigned_digest_bytes(&eq_output.audit_digest_limbs)? != audits.eq_digest {
        return Err(format!(
            "Kagemusha Eq {} audit changed after exact public rebinding",
            family.label()
        ));
    }
    Ok((eq_builder, public_instances))
}

fn build_mint_transport_ep_v1(
    family: MintTransportFamilyV1,
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintTransportDeciderWitnessV1<'_>,
    audits: &KagemushaMintTransportDeferredAuditsV1,
) -> Result<(BaseCircuitBuilder<Fq>, Vec<Fq>), String> {
    validate_mint_transport_parameter_degrees_v1(eq_params.k(), ep_params.k())?;
    if audits.family != family {
        return Err("Kagemusha transport audit family does not match Ep circuit".to_owned());
    }
    let public_instances = witness.ep.outer_instances.to_vec();
    let ep_svk = ep_succinct_vk(ep_params);
    let MintTransportScalarHalfV1 {
        builder: mut ep_builder,
        output: ep_output,
        inner_binding_cells: ep_inner_binding_cells,
    } = build_mint_transport_scalar_half_v1(
        family,
        KagemushaPastaParityV1::Ep,
        &ep_svk,
        witness.ep,
    )?;
    bind_own_audit_v1(&mut ep_builder, family.ep_audit_start(), &ep_output)?;
    let expected_eq = public_digest_cells_v1(&ep_builder, family.eq_audit_start())?;
    constrain_reciprocal_output_with_u128_binding_serialized_v1::<EqAffine>(
        &mut ep_builder,
        &audits.eq,
        &expected_eq,
        &ep_inner_binding_cells,
    )?;
    ep_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    if assigned_digest_bytes(&ep_output.audit_digest_limbs)? != audits.ep_digest {
        return Err(format!(
            "Kagemusha Ep {} audit changed after exact public rebinding",
            family.label()
        ));
    }
    Ok((ep_builder, public_instances))
}

/// Build only the Eq recipient-authorization transport graph.
pub(super) fn build_kagemusha_mint_authorization_transport_eq_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintTransportDeciderWitnessV1<'_>,
    audits: &KagemushaMintTransportDeferredAuditsV1,
) -> Result<(KagemushaMintAuthorizationTransportEqCircuitV1, Vec<Fp>), String> {
    let (builder, instances) = build_mint_transport_eq_v1(
        MintTransportFamilyV1::Authorization,
        eq_params,
        ep_params,
        witness,
        audits,
    )?;
    Ok((
        KagemushaMintAuthorizationTransportEqCircuitV1 { builder },
        instances,
    ))
}

/// Build only the Ep recipient-authorization transport graph.
pub(super) fn build_kagemusha_mint_authorization_transport_ep_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintTransportDeciderWitnessV1<'_>,
    audits: &KagemushaMintTransportDeferredAuditsV1,
) -> Result<(KagemushaMintAuthorizationTransportEpCircuitV1, Vec<Fq>), String> {
    let (builder, instances) = build_mint_transport_ep_v1(
        MintTransportFamilyV1::Authorization,
        eq_params,
        ep_params,
        witness,
        audits,
    )?;
    Ok((
        KagemushaMintAuthorizationTransportEpCircuitV1 { builder },
        instances,
    ))
}

/// Build only the Eq reserve/finality-authority transport graph.
pub(super) fn build_kagemusha_mint_authority_transport_eq_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintTransportDeciderWitnessV1<'_>,
    audits: &KagemushaMintTransportDeferredAuditsV1,
) -> Result<(KagemushaMintAuthorityTransportEqCircuitV1, Vec<Fp>), String> {
    let (builder, instances) = build_mint_transport_eq_v1(
        MintTransportFamilyV1::Authority,
        eq_params,
        ep_params,
        witness,
        audits,
    )?;
    Ok((
        KagemushaMintAuthorityTransportEqCircuitV1 { builder },
        instances,
    ))
}

/// Build only the Ep reserve/finality-authority transport graph.
pub(super) fn build_kagemusha_mint_authority_transport_ep_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintTransportDeciderWitnessV1<'_>,
    audits: &KagemushaMintTransportDeferredAuditsV1,
) -> Result<(KagemushaMintAuthorityTransportEpCircuitV1, Vec<Fq>), String> {
    let (builder, instances) = build_mint_transport_ep_v1(
        MintTransportFamilyV1::Authority,
        eq_params,
        ep_params,
        witness,
        audits,
    )?;
    Ok((
        KagemushaMintAuthorityTransportEpCircuitV1 { builder },
        instances,
    ))
}

/// Build compact recipient-authorization parities and derive both outer audits.
pub(super) fn build_kagemusha_mint_authorization_transport_pair_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintTransportDeciderWitnessV1<'_>,
) -> Result<
    (
        KagemushaMintAuthorizationTransportEqCircuitV1,
        KagemushaMintAuthorizationTransportEpCircuitV1,
        [u8; 32],
        [u8; 32],
    ),
    String,
> {
    let (eq_builder, ep_builder, eq_audit, ep_audit) = build_mint_transport_pair_v1(
        MintTransportFamilyV1::Authorization,
        eq_params,
        ep_params,
        witness,
    )?;
    Ok((
        KagemushaMintAuthorizationTransportEqCircuitV1 {
            builder: eq_builder,
        },
        KagemushaMintAuthorizationTransportEpCircuitV1 {
            builder: ep_builder,
        },
        eq_audit,
        ep_audit,
    ))
}

/// Build compact mint-authority parities, preserving the proven inner pair commitment.
pub(super) fn build_kagemusha_mint_authority_transport_pair_v1(
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintTransportDeciderWitnessV1<'_>,
) -> Result<
    (
        KagemushaMintAuthorityTransportEqCircuitV1,
        KagemushaMintAuthorityTransportEpCircuitV1,
        [u8; 32],
        [u8; 32],
    ),
    String,
> {
    let (eq_builder, ep_builder, eq_audit, ep_audit) = build_mint_transport_pair_v1(
        MintTransportFamilyV1::Authority,
        eq_params,
        ep_params,
        witness,
    )?;
    Ok((
        KagemushaMintAuthorityTransportEqCircuitV1 {
            builder: eq_builder,
        },
        KagemushaMintAuthorityTransportEpCircuitV1 {
            builder: ep_builder,
        },
        eq_audit,
        ep_audit,
    ))
}

fn build_mint_transport_pair_v1(
    family: MintTransportFamilyV1,
    eq_params: &ParamsIPA<EqAffine>,
    ep_params: &ParamsIPA<EpAffine>,
    witness: KagemushaMintTransportDeciderWitnessV1<'_>,
) -> Result<
    (
        BaseCircuitBuilder<Fp>,
        BaseCircuitBuilder<Fq>,
        [u8; 32],
        [u8; 32],
    ),
    String,
> {
    validate_mint_transport_parameter_degrees_v1(eq_params.k(), ep_params.k())?;
    let eq_svk = eq_succinct_vk(eq_params);
    let ep_svk = ep_succinct_vk(ep_params);
    let MintTransportScalarHalfV1 {
        builder: mut eq_builder,
        output: eq_output,
        inner_binding_cells: eq_inner_binding_cells,
    } = build_mint_transport_scalar_half_v1(
        family,
        KagemushaPastaParityV1::Eq,
        &eq_svk,
        witness.eq,
    )?;
    let MintTransportScalarHalfV1 {
        builder: mut ep_builder,
        output: ep_output,
        inner_binding_cells: ep_inner_binding_cells,
    } = build_mint_transport_scalar_half_v1(
        family,
        KagemushaPastaParityV1::Ep,
        &ep_svk,
        witness.ep,
    )?;

    bind_own_audit_v1(&mut eq_builder, family.eq_audit_start(), &eq_output)?;
    bind_own_audit_v1(&mut ep_builder, family.ep_audit_start(), &ep_output)?;

    let eq_expected_ep_audit = public_digest_cells_v1(&eq_builder, family.ep_audit_start())?;
    constrain_reciprocal_output_with_u128_binding_serialized_v1::<EpAffine>(
        &mut eq_builder,
        &ep_output,
        &eq_expected_ep_audit,
        &eq_inner_binding_cells,
    )?;
    let ep_expected_eq_audit = public_digest_cells_v1(&ep_builder, family.eq_audit_start())?;
    constrain_reciprocal_output_with_u128_binding_serialized_v1::<EqAffine>(
        &mut ep_builder,
        &eq_output,
        &ep_expected_eq_audit,
        &ep_inner_binding_cells,
    )?;

    eq_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    ep_builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
    let eq_audit = assigned_digest_bytes(&eq_output.audit_digest_limbs)?;
    let ep_audit = assigned_digest_bytes(&ep_output.audit_digest_limbs)?;
    Ok((eq_builder, ep_builder, eq_audit, ep_audit))
}

fn validate_mint_transport_parameter_degrees_v1(eq_k: u32, ep_k: u32) -> Result<(), String> {
    if eq_k != KAGEMUSHA_RECURSION_IPA_K_V1 {
        return Err(format!(
            "Kagemusha mint transport Eq parameters use k={eq_k}, expected k={KAGEMUSHA_RECURSION_IPA_K_V1}"
        ));
    }
    if ep_k != KAGEMUSHA_RECURSION_IPA_K_V1 {
        return Err(format!(
            "Kagemusha mint transport Ep parameters use k={ep_k}, expected k={KAGEMUSHA_RECURSION_IPA_K_V1}"
        ));
    }
    Ok(())
}

fn validate_mint_transport_column_shape_v1(
    family: MintTransportFamilyV1,
    protocol_columns: &[usize],
    inner_columns: &[usize],
    outer_count: usize,
) -> Result<(), String> {
    let inner_shape_matches = match family {
        MintTransportFamilyV1::Authorization => {
            protocol_columns.len() == 2
                && protocol_columns == inner_columns
                && protocol_columns[0] == family.inner_semantic_count()
                && protocol_columns[1] > protocol_columns[0]
        }
        MintTransportFamilyV1::Authority => {
            protocol_columns == [family.inner_semantic_count()] && protocol_columns == inner_columns
        }
    };
    if !inner_shape_matches
        || outer_count != family.public_count()
        || family.history_start() + accumulator_limb_count() != family.public_count()
    {
        return Err(format!(
            "Kagemusha {} public instance ABI mismatch",
            family.label()
        ));
    }
    Ok(())
}

fn validate_mint_transport_transcript_shape_v1(
    history_challenges: usize,
    proof_bytes: usize,
    expected_proof_bytes: usize,
    fold_bytes: usize,
) -> Result<(), String> {
    if history_challenges != KAGEMUSHA_RECURSION_IPA_K_V1 as usize {
        return Err("Kagemusha inner mint history has the wrong challenge count".to_owned());
    }
    if proof_bytes != expected_proof_bytes {
        return Err("Kagemusha inner mint ordinary proof has the wrong exact length".to_owned());
    }
    if fold_bytes != KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1 {
        return Err("Kagemusha inner mint history fold has the wrong exact length".to_owned());
    }
    Ok(())
}

fn constrain_public_projection_v1<F: ScalarField>(
    builder: &mut BaseCircuitBuilder<F>,
    family: MintTransportFamilyV1,
    inner_cells: &[AssignedValue<F>],
    outer_cells: &[AssignedValue<F>],
) -> Result<Vec<AssignedValue<F>>, String> {
    if inner_cells.len() != family.inner_semantic_count()
        || outer_cells.len() != family.public_count()
    {
        return Err("Kagemusha mint transport projection has the wrong shape".to_owned());
    }
    for index in 0..family.history_start() {
        if family.copies_inner_cell(index) {
            builder
                .main(0)
                .constrain_equal(&inner_cells[index], &outer_cells[index]);
        }
    }
    let range = builder.range_chip();
    let mut inner_binding_cells = family
        .inner_binding_indices()
        .into_iter()
        .map(|index| {
            range.range_check(builder.main(0), inner_cells[index], 128);
            range.range_check(builder.main(0), outer_cells[index], 128);
            inner_cells[index]
        })
        .collect::<Vec<_>>();
    if let Some(indices) = family.carrier_binding_range() {
        inner_binding_cells.extend(indices.map(|index| {
            range.range_check(builder.main(0), inner_cells[index], 128);
            inner_cells[index]
        }));
    }
    Ok(inner_binding_cells)
}

fn build_mint_transport_scalar_half_v1<C>(
    family: MintTransportFamilyV1,
    parity: KagemushaPastaParityV1,
    succinct_vk: &IpaSuccinctVerifyingKey<C>,
    witness: KagemushaMintTransportParityWitnessV1<'_, C>,
) -> Result<MintTransportScalarHalfV1<C>, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + ScalarField,
{
    let inner_column_lengths = witness
        .inner_instances
        .iter()
        .map(Vec::len)
        .collect::<Vec<_>>();
    validate_mint_transport_column_shape_v1(
        family,
        &witness.inner_protocol.num_instance,
        &inner_column_lengths,
        witness.outer_instances.len(),
    )?;
    let proof_profile = ordinary_ipa_proof_profile_v1(witness.inner_protocol)?;
    let expected_proof_bytes = match family {
        MintTransportFamilyV1::Authorization => proof_profile
            .byte_len
            .checked_add(32)
            .ok_or_else(|| "Kagemusha hybrid proof byte length overflowed".to_owned())?,
        MintTransportFamilyV1::Authority => proof_profile.byte_len,
    };
    validate_mint_transport_transcript_shape_v1(
        witness.inner_history.xi.len(),
        witness.inner_proof.len(),
        expected_proof_bytes,
        witness.inner_history_fold_proof.len(),
    )?;
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(KAGEMUSHA_RECURSION_IPA_K_V1 as usize)
        .use_lookup_bits((KAGEMUSHA_RECURSION_IPA_K_V1 - 1) as usize)
        .use_instance_columns(1);
    let outer_cells = witness
        .outer_instances
        .iter()
        .copied()
        .map(|value| builder.main(0).load_witness(value))
        .collect::<Vec<_>>();
    let inner_cells = witness
        .inner_instances
        .first()
        .expect("validated mint transport instance shape has a semantic column")
        .iter()
        .copied()
        .map(|value| builder.main(0).load_witness(value))
        .collect::<Vec<_>>();
    builder.assigned_instances = vec![outer_cells.clone()];
    let inner_binding_cells =
        constrain_public_projection_v1(&mut builder, family, &inner_cells, &outer_cells)?;

    let range = builder.range_chip();
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader: DeferredLoader<'_, C> =
        deferred_loader_v1(&mut builder, &coordinate, &scalar_integer);
    // `loaded`, unlike `loaded_preprocessed_as_witness`, fixes every VK point
    // and transcript initial state in the outer circuit. Authority's public
    // protocol cells still name its recursive OUTER checkpoint protocol.
    let loaded_protocol = witness.inner_protocol.loaded(&loader);
    let loaded_semantic_instances = inner_cells
        .iter()
        .copied()
        .map(|cell| loader.scalar_from_assigned(cell))
        .collect::<Vec<_>>();
    let current = match family {
        MintTransportFamilyV1::Authorization => {
            let carrier_commitment_limb_indices = family
                .carrier_commitment_limb_indices(parity)
                .expect("authorization transport has a carrier commitment");
            verify_hybrid_ordinary_proof_and_stream_v1(
                &loader,
                succinct_vk,
                &loaded_protocol,
                &loaded_semantic_instances,
                carrier_commitment_limb_indices,
                witness.inner_proof,
            )
            .map(|verified| verified.accumulator)
        }
        MintTransportFamilyV1::Authority => verify_ordinary_proof_v1(
            &loader,
            succinct_vk,
            &loaded_protocol,
            std::slice::from_ref(&loaded_semantic_instances),
            witness.inner_proof,
        ),
    }
    .map_err(|error| {
        format!(
            "failed to verify private {} proof: {error:?}",
            family.label()
        )
    })?;
    let inner_history = load_native_accumulator(&loader, witness.inner_history)
        .map_err(|error| format!("failed to load private mint history: {error:?}"))?;
    bind_accumulator_limbs(
        &loader,
        &inner_history,
        &inner_cells[family.history_start()..family.public_count()],
    )
    .map_err(|error| format!("failed to bind private mint history: {error:?}"))?;
    let transported_history = verify_fold(
        &loader,
        succinct_vk,
        &[current, inner_history],
        witness.inner_history_fold_proof,
    )
    .map_err(|error| format!("failed to fold current mint claim into history: {error:?}"))?;
    bind_accumulator_limbs(
        &loader,
        &transported_history,
        &outer_cells[family.history_start()..],
    )
    .map_err(|error| format!("failed to bind transported mint history: {error:?}"))?;
    let output = finalize_tagged_deferred_audit_with_u128_binding_v1(
        &mut builder,
        loader,
        family.equation_tag(),
        &inner_binding_cells,
    )
    .map_err(|error| format!("failed to finalize {} audit: {error:?}", family.label()))?;
    Ok(MintTransportScalarHalfV1 {
        builder,
        output,
        inner_binding_cells,
    })
}

fn bind_own_audit_v1<C>(
    builder: &mut BaseCircuitBuilder<C::ScalarExt>,
    offset: usize,
    output: &KagemushaDeferredParentOutputV1<C>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: BigPrimeField + ScalarField,
{
    let expected = public_digest_cells_v1(builder, offset)?;
    for (actual, expected) in output.audit_digest_limbs.iter().zip(expected) {
        builder.main(0).constrain_equal(actual, &expected);
    }
    Ok(())
}

fn public_digest_cells_v1<F: ScalarField>(
    builder: &BaseCircuitBuilder<F>,
    offset: usize,
) -> Result<[AssignedValue<F>; 2], String> {
    let end = offset
        .checked_add(2)
        .ok_or_else(|| "Kagemusha mint transport digest offset overflow".to_owned())?;
    builder
        .assigned_instances
        .first()
        .and_then(|column| column.get(offset..end))
        .ok_or_else(|| "Kagemusha mint transport public audit is absent".to_owned())?
        .try_into()
        .map_err(|_| "Kagemusha mint transport public audit has wrong shape".to_owned())
}

fn checked_inventory_sum_v1(values: &[usize]) -> Result<usize, String> {
    values
        .iter()
        .try_fold(0_usize, |sum, value| sum.checked_add(*value))
        .ok_or_else(|| "Kagemusha mint transport inventory overflow".to_owned())
}

fn packed_rows_v1(cells: &[usize], columns: &[usize]) -> Result<usize, String> {
    if cells.len() != columns.len() {
        return Err("Kagemusha mint transport phase inventory mismatch".to_owned());
    }
    cells
        .iter()
        .zip(columns)
        .try_fold(0_usize, |maximum, (cells, columns)| {
            if *cells == 0 {
                return Ok(maximum);
            }
            if *columns == 0 {
                return Err("Kagemusha mint transport cells have no physical column".to_owned());
            }
            Ok(maximum.max(cells.div_ceil(*columns)))
        })
}

fn mint_transport_capacity_profile_v1<F>(
    builder: &BaseCircuitBuilder<F>,
) -> Result<KagemushaMintTransportDeciderCapacityProfileV1, String>
where
    F: ScalarField + BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    let stats = builder.statistics();
    let params = &builder.config_params;
    let gate_advice_cells = checked_inventory_sum_v1(&stats.gate.total_advice_per_phase)?;
    let gate_advice_columns = checked_inventory_sum_v1(&params.num_advice_per_phase)?;
    let gate_packed_rows = packed_rows_v1(
        &stats.gate.total_advice_per_phase,
        &params.num_advice_per_phase,
    )?;
    let lookup_advice_cells = checked_inventory_sum_v1(&stats.total_lookup_advice_per_phase)?;
    let lookup_advice_columns = checked_inventory_sum_v1(&params.num_lookup_advice_per_phase)?;
    let lookup_packed_rows = packed_rows_v1(
        &stats.total_lookup_advice_per_phase,
        &params.num_lookup_advice_per_phase,
    )?;
    let k = params.k;
    if k != KAGEMUSHA_RECURSION_IPA_K_V1 as usize {
        return Err(
            "Kagemusha mint transport capacity inventory requires the fixed K16 domain".to_owned(),
        );
    }
    let domain_rows = 1_usize
        .checked_shl(
            u32::try_from(k)
                .map_err(|_| "Kagemusha mint transport domain exponent overflow".to_owned())?,
        )
        .ok_or_else(|| "Kagemusha mint transport domain row count overflow".to_owned())?;
    let usable_rows = domain_rows
        .checked_sub(MINIMUM_UNUSABLE_ROWS)
        .ok_or_else(|| "Kagemusha mint transport unusable rows exceed domain".to_owned())?;
    let max_component_rows = gate_packed_rows.max(lookup_packed_rows);
    if max_component_rows > usable_rows {
        return Err(format!(
            "Kagemusha mint transport requires {max_component_rows} rows, exceeding {usable_rows}"
        ));
    }
    Ok(KagemushaMintTransportDeciderCapacityProfileV1 {
        k,
        domain_rows,
        usable_rows,
        gate_advice_cells,
        gate_advice_columns,
        gate_packed_rows,
        lookup_advice_cells,
        lookup_advice_columns,
        lookup_packed_rows,
        dense_jobs: 0,
        dense_sources: 0,
        dense_rows: 0,
        max_component_rows,
    })
}

const _: () = {
    assert!(MINT_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1 == 84);
    assert!(MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1 == 88);
    assert!(KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1 == 56);
    assert!(authorization_public_instance::EQ_AUDIT_LO == 46);
    assert!(authorization_public_instance::EP_AUDIT_LO == 48);
    assert!(authorization_public_instance::HISTORY_START == 50);
    assert!(MINT_AUTHORIZATION_EQ_CARRIER_COMMITMENT_LO_V1 == 84);
    assert!(MINT_AUTHORIZATION_EP_CARRIER_COMMITMENT_LO_V1 == 86);
    assert!(authority_public_instance::EQ_PROTOCOL_LO == 12);
    assert!(authority_public_instance::EP_PROTOCOL_HI == 15);
    assert!(authority_public_instance::EQ_AUDIT_LO == 16);
    assert!(authority_public_instance::EP_AUDIT_LO == 18);
    assert!(authority_public_instance::PAIR_BINDING_LO == 20);
    assert!(authority_public_instance::PAIR_BINDING_HI == 21);
    assert!(authority_public_instance::HISTORY_START == 22);
    assert!(accumulator_limb_count() == 34);
};

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_proofs::{dev::MockProver, poly::commitment::ParamsProver as _};

    #[test]
    fn mint_transport_rejects_wrong_parameter_degrees_before_succinct_keys() {
        let fixed = KAGEMUSHA_RECURSION_IPA_K_V1;
        assert!(validate_mint_transport_parameter_degrees_v1(fixed, fixed).is_ok());
        // Small actual parameter objects exercise degree extraction without
        // constructing a full cash circuit, key pair, or purported mint proof.
        let eq_small = ParamsIPA::<EqAffine>::new(6);
        let ep_small = ParamsIPA::<EpAffine>::new(6);
        assert!(
            validate_mint_transport_parameter_degrees_v1(eq_small.k(), fixed)
                .unwrap_err()
                .contains("Eq parameters use k=6")
        );
        assert!(
            validate_mint_transport_parameter_degrees_v1(fixed, ep_small.k())
                .unwrap_err()
                .contains("Ep parameters use k=6")
        );
        assert!(validate_mint_transport_parameter_degrees_v1(eq_small.k(), ep_small.k()).is_err());
        for wrong in [0, 15, 17, u32::MAX] {
            assert!(validate_mint_transport_parameter_degrees_v1(wrong, fixed).is_err());
            assert!(validate_mint_transport_parameter_degrees_v1(fixed, wrong).is_err());
        }
    }

    #[test]
    fn mint_transport_layouts_preserve_exact_semantics_and_inner_pair_commitment() {
        for (family, copied, binding, history, tag) in [
            (
                MintTransportFamilyV1::Authorization,
                (0..46).collect::<Vec<_>>(),
                [46, 47, 48, 49],
                50,
                7,
            ),
            (
                MintTransportFamilyV1::Authority,
                (0..16).chain(20..22).collect::<Vec<_>>(),
                [16, 17, 18, 19],
                22,
                8,
            ),
        ] {
            assert_eq!(
                (0..family.public_count())
                    .filter(|index| family.copies_inner_cell(*index))
                    .collect::<Vec<_>>(),
                copied
            );
            assert_eq!(family.inner_binding_indices(), binding);
            assert_eq!(
                family.carrier_binding_range(),
                match family {
                    MintTransportFamilyV1::Authorization => Some(84..88),
                    MintTransportFamilyV1::Authority => None,
                }
            );
            assert_eq!(
                family.carrier_commitment_limb_indices(KagemushaPastaParityV1::Eq),
                match family {
                    MintTransportFamilyV1::Authorization => Some([84, 85]),
                    MintTransportFamilyV1::Authority => None,
                }
            );
            assert_eq!(
                family.carrier_commitment_limb_indices(KagemushaPastaParityV1::Ep),
                match family {
                    MintTransportFamilyV1::Authorization => Some([86, 87]),
                    MintTransportFamilyV1::Authority => None,
                }
            );
            assert_eq!(family.history_start(), history);
            assert_eq!(family.public_count() - history, 34);
            assert_eq!(family.equation_tag(), tag);
            assert!(
                tag > 6,
                "mint decider tags must not overlap existing families"
            );
        }
    }

    #[test]
    fn mint_transport_rejects_malformed_columns_and_transcript_shapes() {
        for family in [
            MintTransportFamilyV1::Authorization,
            MintTransportFamilyV1::Authority,
        ] {
            let count = family.public_count();
            let valid_columns = match family {
                MintTransportFamilyV1::Authorization => vec![
                    MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1,
                    MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1 + 1,
                ],
                MintTransportFamilyV1::Authority => vec![count],
            };
            assert!(
                validate_mint_transport_column_shape_v1(
                    family,
                    &valid_columns,
                    &valid_columns,
                    count,
                )
                .is_ok()
            );
            let malformed_protocols = match family {
                MintTransportFamilyV1::Authorization => vec![
                    vec![],
                    vec![MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1],
                    vec![MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1, 0],
                    vec![
                        MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1,
                        MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1,
                    ],
                    vec![
                        MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1 - 1,
                        MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1 + 1,
                    ],
                    vec![
                        MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1,
                        MINT_AUTHORIZATION_INNER_SEMANTIC_INSTANCE_COUNT_V1 + 1,
                        0,
                    ],
                ],
                MintTransportFamilyV1::Authority => vec![
                    vec![],
                    vec![count - 1],
                    vec![count + 1],
                    vec![count, 0],
                    vec![count / 2, count / 2],
                ],
            };
            for columns in malformed_protocols {
                assert!(
                    validate_mint_transport_column_shape_v1(family, &columns, &columns, count)
                        .is_err(),
                    "malformed {family:?} columns unexpectedly passed: {columns:?}",
                );
            }
            let mut wrong_inner = valid_columns.clone();
            wrong_inner[0] -= 1;
            assert!(
                validate_mint_transport_column_shape_v1(
                    family,
                    &valid_columns,
                    &wrong_inner,
                    count,
                )
                .is_err()
            );
            for wrong in [0, count - 1, count + 1, usize::MAX] {
                assert!(
                    validate_mint_transport_column_shape_v1(
                        family,
                        &valid_columns,
                        &valid_columns,
                        wrong,
                    )
                    .is_err()
                );
            }
        }
        let fold = KAGEMUSHA_IPA_FOLD_PROOF_BYTES_V1;
        // This tests exact-size admission only, not validity of a proof of this length.
        assert!(validate_mint_transport_transcript_shape_v1(16, 1920, 1920, fold).is_ok());
        for history in [0, 15, 17, usize::MAX] {
            assert!(
                validate_mint_transport_transcript_shape_v1(history, 1920, 1920, fold).is_err()
            );
        }
        for proof in [0, 1919, 1921, usize::MAX] {
            assert!(validate_mint_transport_transcript_shape_v1(16, proof, 1920, fold).is_err());
        }
        for wrong_fold in [0, fold - 1, fold + 1, usize::MAX] {
            assert!(
                validate_mint_transport_transcript_shape_v1(16, 1920, 1920, wrong_fold).is_err()
            );
        }
    }

    fn projection_builder<F: ScalarField>(
        family: MintTransportFamilyV1,
        inner: &[F],
        outer: &[F],
    ) -> BaseCircuitBuilder<F> {
        let mut builder = BaseCircuitBuilder::new(false)
            .use_k(9)
            .use_lookup_bits(8)
            .use_instance_columns(1);
        let inner = inner
            .iter()
            .copied()
            .map(|value| builder.main(0).load_witness(value))
            .collect::<Vec<_>>();
        let outer = outer
            .iter()
            .copied()
            .map(|value| builder.main(0).load_witness(value))
            .collect::<Vec<_>>();
        let bound = constrain_public_projection_v1(&mut builder, family, &inner, &outer).unwrap();
        assert_eq!(bound.len(), 4);
        for (bound, index) in bound.iter().zip(family.inner_binding_indices()) {
            assert_eq!(bound.cell, inner[index].cell);
        }
        builder.assigned_instances = vec![outer];
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        builder
    }

    fn check_projection_constraints<F>(family: MintTransportFamilyV1)
    where
        F: ScalarField + BigPrimeField + ff::WithSmallOrderMulGroup<3>,
    {
        let inner = (0..family.public_count())
            .map(|index| F::from(index as u64 + 1))
            .collect::<Vec<_>>();
        let mut outer = inner.clone();
        for index in 0..family.public_count() {
            if !family.copies_inner_cell(index) {
                outer[index] += F::from(101);
            }
        }
        // Only the copy/range subrelation is tested here. The full circuit must
        // additionally verify both inner proofs, folds, and reciprocal equations.
        let circuit = projection_builder(family, &inner, &outer);
        MockProver::run(9, &circuit, vec![outer.clone()])
            .unwrap()
            .assert_satisfied();
        for index in (0..family.public_count()).filter(|index| family.copies_inner_cell(*index)) {
            let mut changed = outer.clone();
            changed[index] += F::ONE;
            let invalid = projection_builder(family, &inner, &changed);
            assert!(
                MockProver::run(9, &invalid, vec![changed])
                    .unwrap()
                    .verify()
                    .is_err(),
                "copied cell {index} was unconstrained for {family:?}"
            );
        }
        for index in family.inner_binding_indices() {
            let mut changed_inner = inner.clone();
            changed_inner[index] = -F::ONE;
            let invalid = projection_builder(family, &changed_inner, &outer);
            assert!(
                MockProver::run(9, &invalid, vec![outer.clone()])
                    .unwrap()
                    .verify()
                    .is_err(),
                "inner audit cell {index} was not u128"
            );
            let mut changed_outer = outer.clone();
            changed_outer[index] = -F::ONE;
            let invalid = projection_builder(family, &inner, &changed_outer);
            assert!(
                MockProver::run(9, &invalid, vec![changed_outer])
                    .unwrap()
                    .verify()
                    .is_err(),
                "outer audit cell {index} was not u128"
            );
        }
    }

    #[test]
    fn mint_authorization_transport_projection_constraints_both_parities() {
        check_projection_constraints::<Fp>(MintTransportFamilyV1::Authorization);
        check_projection_constraints::<Fq>(MintTransportFamilyV1::Authorization);
    }

    #[test]
    fn mint_authority_transport_projection_constraints_both_parities() {
        check_projection_constraints::<Fp>(MintTransportFamilyV1::Authority);
        check_projection_constraints::<Fq>(MintTransportFamilyV1::Authority);
    }

    #[test]
    fn mint_transport_projection_and_audit_extraction_reject_bad_shapes() {
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(9)
            .use_lookup_bits(8)
            .use_instance_columns(1);
        let cell = builder.main(0).load_witness(Fp::from(1));
        for family in [
            MintTransportFamilyV1::Authorization,
            MintTransportFamilyV1::Authority,
        ] {
            let correct = vec![cell; family.public_count()];
            assert!(constrain_public_projection_v1(&mut builder, family, &[], &correct).is_err());
            assert!(constrain_public_projection_v1(&mut builder, family, &correct, &[]).is_err());
        }
        assert!(public_digest_cells_v1(&builder, 0).is_err());
        builder.assigned_instances = vec![vec![cell, cell]];
        assert_eq!(
            public_digest_cells_v1(&builder, 0).unwrap()[0].cell,
            cell.cell
        );
        assert!(public_digest_cells_v1(&builder, 1).is_err());
        assert!(public_digest_cells_v1(&builder, usize::MAX).is_err());
    }

    #[test]
    fn mint_transport_capacity_inventory_rejects_overflow_and_bad_phase_packing() {
        assert_eq!(checked_inventory_sum_v1(&[1, 2, 3]).unwrap(), 6);
        assert_eq!(checked_inventory_sum_v1(&[]).unwrap(), 0);
        assert!(checked_inventory_sum_v1(&[usize::MAX, 1]).is_err());
        assert_eq!(packed_rows_v1(&[10, 0, 17], &[3, 0, 2]).unwrap(), 9);
        assert_eq!(packed_rows_v1(&[usize::MAX], &[usize::MAX]).unwrap(), 1);
        assert!(packed_rows_v1(&[1], &[]).is_err());
        assert!(packed_rows_v1(&[1], &[0]).is_err());
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(16)
            .use_lookup_bits(15)
            .use_instance_columns(1);
        builder.main(0).load_witness(Fp::from(3));
        builder.calculate_params(Some(MINIMUM_UNUSABLE_ROWS));
        let profile = mint_transport_capacity_profile_v1(&builder).unwrap();
        assert_eq!(profile.domain_rows, 65536);
        assert_eq!(profile.usable_rows, 65527);
        assert_eq!(
            (
                profile.dense_jobs,
                profile.dense_sources,
                profile.dense_rows
            ),
            (0, 0, 0)
        );
        assert!(profile.max_component_rows <= profile.usable_rows);
        builder.config_params.k = 15;
        assert!(mint_transport_capacity_profile_v1(&builder).is_err());
    }
}
