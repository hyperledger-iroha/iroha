//! Separate ordinary Mint issuer/platform/financial-secret/credit-opening relation.
//!
//! The public column is reconstructed only from the exact canonical TopUpRequest by the closed
//! ordinary verifier. It includes the complete context and statement identities plus every
//! scope/opening operand below. No offered public-instance vector is an authorization interface.
//! Variable FI namespace/Account Norito bytes are public-verifier preprocessing, not hidden
//! witness constants in a reusable key. Full C/PI originals, platform evidence and the credit
//! opening/ciphertext use genuine constrained SHA jobs and both active platform equations.
use super::super::ordinary_mint_public::{
    ORDINARY_MINT_PUBLIC_PREFIX_V1, ordinary_mint_public_data_v1,
};
use super::super::{
    DigestV1, KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1,
    canonical_preimage::{assemble_canonical_preimage_v1, stream::KagemushaBoundedByteStreamV1},
    composite::assigned_uint_bytes_v1,
    guard_bundle::{assign_bytes, constant_bytes, digest_limbs_assigned, hash},
    initial_kagemusha_ep_accumulator_v1, initial_kagemusha_eq_accumulator_v1,
    ordinary_app_guard_binding::OrdinaryCredentialIssuerCellsV1,
    ordinary_credential_union::{
        assign_ordinary_credential_union_v1, reconstruct_ordinary_credential_union_v1,
    },
    ordinary_integrity_union::constrain_ordinary_integrity_union_v1,
    ordinary_issuer_config::OrdinaryIssuerTableV1,
    ordinary_issuer_equation::constrain_ordinary_issuer_original_v1,
    ordinary_platform_union::constrain_ordinary_signed_message_union_v1,
};
use super::{KagemushaOrdinaryMintEpCircuitV1, KagemushaOrdinaryMintEqCircuitV1};
use crate::{
    kagemusha_v1_poseidon::{KagemushaPoseidonFieldV1, from_u128},
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{
        GateInstructions as _, RangeChip, RangeInstructions as _,
        circuit::builder::BaseCircuitBuilder,
    },
};
use halo2_proofs::{
    halo2curves::pasta::{EpAffine, EqAffine, Fp, Fq},
    poly::ipa::commitment::ParamsIPA,
};
use iroha_data_model::kagemusha::*;
use sha2::{Digest as _, Sha256};
type Bytes<F> = [PastaSha256ByteV1<F>; 32];
const ACCOUNT_MAX: usize = 4096;
const DIGEST_COUNT: usize = 34;
/// Proof witness only; construction lends no Native funding/debit or global lineage capability.
pub(crate) struct OrdinaryMintWitnessV1<'a> {
    pub(crate) statement: &'a KagemushaOrdinaryMintAuthorizationStatementV1,
    pub(crate) approval: &'a KagemushaOrdinaryMintApprovalV1,
    pub(crate) credential: &'a KagemushaOrdinaryAppCredentialV1,
    pub(crate) previous_app_attest_counter: Option<u32>,
    pub(crate) integrity_lease: Option<&'a KagemushaPlayIntegrityRefreshLeaseV1>,
    pub(crate) financial_secret: &'a DigestV1,
    pub(crate) credit_opening: &'a KagemushaCreditOpeningV1,
    pub(crate) encrypted_credit: &'a [u8],
}
fn equal<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    r: &RangeChip<F>,
    a: Bytes<F>,
    b: Bytes<F>,
) {
    for (a, b) in a.into_iter().zip(b) {
        let d = r.gate().sub(ctx, a.quantum_cell(), b.quantum_cell());
        r.gate().assert_is_const(ctx, &d, &F::ZERO);
    }
}
fn nonzero<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    r: &RangeChip<F>,
    b: &[PastaSha256ByteV1<F>],
) {
    let sum = r.gate().sum(ctx, b.iter().map(|b| b.quantum_cell()));
    let z = r.gate().is_zero(ctx, sum);
    r.gate().assert_is_const(ctx, &z, &F::ZERO);
}
fn uint<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    r: &RangeChip<F>,
    bytes: &[PastaSha256ByteV1<F>],
    bits: usize,
) -> Result<AssignedValue<F>, String> {
    if bytes.len() * 8 != bits {
        return Err("ordinary Mint C scalar width differs".into());
    }
    let value = r.gate().inner_product(
        ctx,
        bytes.iter().map(|b| b.quantum_cell()),
        (0..bytes.len()).map(|i| QuantumCell::Constant(F::from(1_u64 << (i * 8)))),
    );
    r.range_check(ctx, value, bits);
    Ok(value)
}
fn words_to_bytes<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    r: &RangeChip<F>,
    words: [AssignedValue<F>; 8],
) -> Result<Bytes<F>, String> {
    let mut bytes = Vec::with_capacity(32);
    for w in words {
        let bits = PastaSha256BitV1::decompose(ctx, r.gate(), w, 32);
        for k in [24, 16, 8, 0] {
            bytes.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                r.gate(),
                &bits[k..k + 8],
            ));
        }
    }
    bytes
        .try_into()
        .map_err(|_| "ordinary Mint SHA width differs".into())
}
fn framed_hash<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    domain: &[u8],
    raw: Vec<PastaSha256ByteV1<F>>,
) -> Result<Bytes<F>, String> {
    let mut m = constant_bytes(domain);
    m.extend(constant_bytes(&(raw.len() as u64).to_le_bytes()));
    m.extend(raw);
    hash(ctx, jobs, m)
}
fn account_hash<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    r: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    domain: &[u8],
    raw: &KagemushaBoundedByteStreamV1<F>,
) -> Result<Bytes<F>, String> {
    let mut prefix = constant_bytes(domain);
    prefix.extend(assigned_uint_bytes_v1(ctx, r.gate(), raw.actual_len(), 64));
    let plen = prefix.len();
    let len = ctx.load_constant(F::from(plen as u64));
    let prefix = KagemushaBoundedByteStreamV1::constrain(ctx, r, prefix, len)?;
    let message = prefix.concat(ctx, r, raw, plen + ACCOUNT_MAX)?;
    let words = jobs.digest_bounded_constrained(ctx, r, message.bytes(), message.actual_len())?;
    words_to_bytes(ctx, r, words)
}
fn build_half<F: KagemushaPoseidonFieldV1>(
    w: &OrdinaryMintWitnessV1<'_>,
    root: DigestV1,
    table: &OrdinaryIssuerTableV1,
    history: &[u8; KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
) -> Result<
    (
        BaseCircuitBuilder<F>,
        PastaSha256JobsV1<F>,
        [AssignedValue<F>; 2],
        [AssignedValue<F>; 65],
        [AssignedValue<F>; 2],
    ),
    String,
> {
    let data = ordinary_mint_public_data_v1(
        w.statement,
        w.approval,
        w.credential,
        w.integrity_lease,
        root,
    )?;
    w.statement.validate_encrypted_credit(w.encrypted_credit)?;
    w.statement
        .context
        .validate_credit_opening(w.credit_opening)?;
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(KAGEMUSHA_HALO2_K_V1 as usize)
        .use_lookup_bits((KAGEMUSHA_HALO2_K_V1 - 1) as usize)
        .use_instance_columns(1);
    let mut jobs = PastaSha256JobsV1::default();
    let range = builder.range_chip();
    let digest_cells: [Bytes<F>; DIGEST_COUNT] = data.digests.map(|d| {
        assign_bytes(builder.main(0), &range, &d)
            .try_into()
            .expect("digest32")
    });
    let scalar_cells = data
        .scalars
        .map(|n| builder.main(0).load_witness(from_u128::<F>(n)));
    let bits = [16, 128, 32, 64, 128, 64, 64, 64, 64, 32, 1];
    for (cell, bits) in scalar_cells.iter().zip(bits) {
        range.range_check(builder.main(0), *cell, bits);
    }
    range
        .gate()
        .assert_is_const(builder.main(0), &scalar_cells[0], &F::ONE);
    range.gate().assert_bit(builder.main(0), scalar_cells[10]);
    for i in (0..DIGEST_COUNT).filter(|i| *i != 5) {
        nonzero(builder.main(0), &range, &digest_cells[i]);
    }
    for i in [1, 3, 5, 6, 7, 8] {
        let z = range.gate().is_zero(builder.main(0), scalar_cells[i]);
        range.gate().assert_is_const(builder.main(0), &z, &F::ZERO);
    }
    let scale = range.is_less_than_safe(
        builder.main(0),
        scalar_cells[2],
        u64::from(KAGEMUSHA_ASSET_SCALE_MAX_V1) + 1,
    );
    range
        .gate()
        .assert_is_const(builder.main(0), &scale, &F::ONE);
    let mut union = assign_ordinary_credential_union_v1(&mut builder, w.credential)?;
    let index = table.selected(w.credential.subject.hardware_profile_id)?;
    let issuer_raw = table.slots[index].issuer_sec1;
    let issuer_cells = core::array::from_fn(|i| {
        builder
            .main(0)
            .load_witness(F::from(u64::from(issuer_raw[i])))
    });
    let ed = reconstruct_ordinary_credential_union_v1(
        &mut builder,
        &mut jobs,
        w.credential,
        &union,
        false,
    )?;
    let sig = assign_bytes(
        builder.main(0),
        &range,
        w.credential.circuit_admission.signature.as_raw_bytes(),
    )
    .try_into()
    .map_err(|_| "ordinary Mint issuer signature width")?;
    constrain_ordinary_issuer_original_v1(
        &mut builder,
        &mut jobs,
        1,
        &union.cells.fixed_digests[6],
        &union.cells.fixed_digests[7],
        &ed,
        &sig,
        &w.credential.circuit_admission,
        &issuer_raw,
        &issuer_cells,
    )?;
    union.cells.issuer_admission = Some(OrdinaryCredentialIssuerCellsV1 {
        ed_original_sha256: ed,
        signature: sig,
    });
    let cd = reconstruct_ordinary_credential_union_v1(
        &mut builder,
        &mut jobs,
        w.credential,
        &union,
        true,
    )?;
    equal(builder.main(0), &range, cd, digest_cells[2]);
    for (ci, di) in [(3, 16), (4, 11), (5, 15), (6, 7), (7, 20), (8, 8), (15, 19)] {
        equal(
            builder.main(0),
            &range,
            union.cells.fixed_digests[ci],
            digest_cells[di],
        );
    }
    let epoch = uint(builder.main(0), &range, &union.cells.scalars[0], 64)?;
    builder.main(0).constrain_equal(&epoch, &scalar_cells[3]);
    builder
        .main(0)
        .constrain_equal(&union.apple, &scalar_cells[10]);
    let mut epoch_message = constant_bytes(b"iroha:kagemusha:v1:ordinary-financial-epoch\0");
    epoch_message.extend(constant_bytes(&200_u64.to_le_bytes()));
    for i in [0, 4, 5, 6, 7, 15] {
        epoch_message.extend(union.cells.fixed_digests[i]);
    }
    epoch_message.extend_from_slice(&union.cells.scalars[1]);
    let financial_epoch = hash(builder.main(0), &mut jobs, epoch_message)?;
    equal(builder.main(0), &range, financial_epoch, digest_cells[18]);
    let secret = assign_bytes(builder.main(0), &range, w.financial_secret);
    nonzero(builder.main(0), &range, &secret);
    let authority = hash(
        builder.main(0),
        &mut jobs,
        [
            constant_bytes(b"iroha:kagemusha:v1:device-proof-authority\0"),
            secret,
        ]
        .concat(),
    )?;
    equal(builder.main(0), &range, authority, digest_cells[19]);
    let ctx = builder.main(0);
    let gate = range.gate();
    let issued = scalar_cells[7];
    let expires = scalar_cells[8];
    range.check_less_than(ctx, issued, expires, 64);
    let life = gate.sub(ctx, expires, issued);
    range.range_check(ctx, life, 64);
    let valid = range.is_less_than_safe(
        ctx,
        life,
        KAGEMUSHA_APP_OPERATION_APPROVAL_MAX_LIFETIME_MS_V1 + 1,
    );
    gate.assert_is_const(ctx, &valid, &F::ONE);
    for (before, after) in [
        (issued, scalar_cells[5]),
        (scalar_cells[5], scalar_cells[6]),
    ] {
        let bad = range.is_less_than(ctx, after, before, 64);
        gate.assert_is_const(ctx, &bad, &F::ZERO);
    }
    range.check_less_than(ctx, scalar_cells[6], expires, 64);
    let ci = uint(ctx, &range, &union.cells.scalars[2], 64)?;
    let ce = uint(ctx, &range, &union.cells.scalars[3], 64)?;
    for (before, after) in [(ci, issued), (expires, ce)] {
        let bad = range.is_less_than(ctx, after, before, 64);
        gate.assert_is_const(ctx, &bad, &F::ZERO);
    }
    let mut clock = constant_bytes(KAGEMUSHA_ORDINARY_CASH_CLOCK_DOMAIN_V1);
    clock.extend(constant_bytes(&1_u16.to_le_bytes()));
    clock.extend(digest_cells[29]);
    clock.extend(digest_cells[30]);
    clock.extend(assigned_uint_bytes_v1(ctx, gate, scalar_cells[5], 64));
    clock.extend(assigned_uint_bytes_v1(ctx, gate, scalar_cells[6], 64));
    let clock = hash(ctx, &mut jobs, clock)?;
    equal(ctx, &range, clock, digest_cells[26]);
    let mut message = constant_bytes(KAGEMUSHA_ORDINARY_MINT_APPROVAL_DOMAIN_V1);
    message.extend(constant_bytes(&210_u64.to_le_bytes()));
    message.extend(constant_bytes(&1_u16.to_le_bytes()));
    for i in [6, 28, 2, 0, 26, 27] {
        message.extend(digest_cells[i]);
    }
    message.extend(assigned_uint_bytes_v1(ctx, gate, issued, 64));
    message.extend(assigned_uint_bytes_v1(ctx, gate, expires, 64));
    let lease = constrain_ordinary_integrity_union_v1(
        &mut builder,
        &mut jobs,
        &union.cells,
        &cd,
        union.integrity,
        w.integrity_lease,
        Some((&issuer_raw, &issuer_cells)),
        issued,
        expires,
    )?;
    equal(builder.main(0), &range, lease, digest_cells[5]);
    let platform = constrain_ordinary_signed_message_union_v1(
        &mut builder,
        &mut jobs,
        &union.cells,
        w.credential.subject.app_public_key.as_sec1_bytes(),
        w.credential.subject.app_release_digest,
        &w.approval.evidence,
        &w.approval.challenge.canonical_signing_bytes()?,
        &message,
        union.apple,
        w.previous_app_attest_counter,
        None,
        false,
    )?;
    // Public floor is the actual signed enrollment minimum. The privately retained Native
    // floor supplied above is separately constrained >= this minimum and < accepted counter.
    let enrolled_floor = uint(builder.main(0), &range, &union.cells.scalars[4], 32)?;
    builder
        .main(0)
        .constrain_equal(&enrolled_floor, &scalar_cells[9]);
    let words = jobs.digest_bounded_constrained(
        builder.main(0),
        &range,
        platform.active_original.bytes(),
        platform.active_original.actual_len(),
    )?;
    let evidence = words_to_bytes(builder.main(0), &range, words)?;
    equal(builder.main(0), &range, evidence, digest_cells[4]);
    // Two independent account hashes use the exact same constrained complete original stream.
    let account = norito::encode_canonical(&w.statement.context.lineage.owner.account_id)
        .map_err(|e| e.to_string())?;
    if account.len() > ACCOUNT_MAX {
        return Err("ordinary Mint account original capacity differs".into());
    }
    let mut raw = vec![0; ACCOUNT_MAX];
    raw[..account.len()].copy_from_slice(&account);
    let bytes = assign_bytes(builder.main(0), &range, &raw);
    let len = builder.main(0).load_witness(F::from(account.len() as u64));
    let account = KagemushaBoundedByteStreamV1::constrain(builder.main(0), &range, bytes, len)?;
    let app_account = account_hash(
        builder.main(0),
        &range,
        &mut jobs,
        b"iroha:kagemusha:v1:app-approval-account\0",
        &account,
    )?;
    equal(builder.main(0), &range, app_account, digest_cells[16]);
    let neutral_account = account_hash(
        builder.main(0),
        &range,
        &mut jobs,
        b"iroha:kagemusha:v1:account-identity\0",
        &account,
    )?;
    equal(builder.main(0), &range, neutral_account, digest_cells[17]);
    let ctx = builder.main(0);
    let opening = w.credit_opening;
    let recipient = assign_bytes(ctx, &range, &opening.recipient_binding_opening);
    let credit = assign_bytes(ctx, &range, &opening.credit_commitment_opening);
    let recovery = assign_bytes(ctx, &range, &opening.recovery_nonce);
    for b in [&recipient, &credit, &recovery] {
        nonzero(ctx, &range, b);
    }
    let recipient_frame = assemble_canonical_preimage_v1(
        ctx,
        &range,
        &kagemusha_recipient_credential_commitment_preimage_layout_v1()
            .map_err(|e| e.to_string())?,
        &KAGEMUSHA_RECIPIENT_CREDENTIAL_COMMITMENT_PREIMAGE_FIELD_RANGES_V1,
        &[&digest_cells[6], &cd, &recipient],
    )?;
    let recipient_hash = framed_hash(
        ctx,
        &mut jobs,
        b"iroha:kagemusha:v1:recipient-credential-commitment\0",
        recipient_frame,
    )?;
    equal(ctx, &range, recipient_hash, digest_cells[21]);
    let version = constant_bytes(&1_u16.to_le_bytes());
    let scale = assigned_uint_bytes_v1(ctx, range.gate(), scalar_cells[2], 32);
    let amount = assigned_uint_bytes_v1(ctx, range.gate(), scalar_cells[1], 128);
    let credit_frame = assemble_canonical_preimage_v1(
        ctx,
        &range,
        &kagemusha_mint_credit_opening_commitment_preimage_layout_v1()
            .map_err(|e| e.to_string())?,
        &KAGEMUSHA_MINT_CREDIT_OPENING_COMMITMENT_PREIMAGE_FIELD_RANGES_V1,
        &[
            &version,
            &digest_cells[11],
            &digest_cells[12],
            &digest_cells[13],
            &scale,
            &digest_cells[14],
            &amount,
            &digest_cells[17],
            &digest_cells[23],
            &credit,
        ],
    )?;
    let credit_hash = framed_hash(
        ctx,
        &mut jobs,
        b"iroha:kagemusha:v1:mint-credit-opening-commitment\0",
        credit_frame,
    )?;
    equal(ctx, &range, credit_hash, digest_cells[22]);
    let actual_id = assign_bytes(ctx, &range, &opening.credit_id)
        .try_into()
        .map_err(|_| "ordinary Mint opening credit width")?;
    equal(ctx, &range, actual_id, digest_cells[24]);
    let actual_amount = ctx.load_witness(from_u128::<F>(opening.amount));
    ctx.constrain_equal(&actual_amount, &scalar_cells[1]);
    let actual_version = ctx.load_witness(F::from(u64::from(opening.version)));
    ctx.constrain_equal(&actual_version, &scalar_cells[0]);
    let ciphertext = assign_bytes(ctx, &range, w.encrypted_credit);
    let ciphertext = framed_hash(
        ctx,
        &mut jobs,
        b"iroha:kagemusha:v1:ciphertext\0",
        ciphertext,
    )?;
    equal(ctx, &range, ciphertext, digest_cells[25]);
    let provider_cells = digest_limbs_assigned(ctx, &digest_cells[33]);
    let profile_cells = digest_limbs_assigned(ctx, &digest_cells[20]);
    let mut public = digest_cells
        .iter()
        .flat_map(|d| digest_limbs_assigned(ctx, d))
        .collect::<Vec<_>>();
    public.extend(scalar_cells);
    if public.len() != ORDINARY_MINT_PUBLIC_PREFIX_V1 {
        return Err("ordinary Mint public contract width differs".into());
    }
    public.extend(history.chunks_exact(16).map(|b| {
        ctx.load_constant(from_u128::<F>(u128::from_le_bytes(
            b.try_into().expect("history16"),
        )))
    }));
    builder.assigned_instances = vec![public];
    super::super::base_packing::finalize_base_params_v1(&mut builder, 9)?;
    jobs.validate_capacity((1_usize << KAGEMUSHA_HALO2_K_V1) - 9)
        .map_err(|reason| {
            // Public circuit shape only; original financial/platform witnesses are not logged.
            format!(
                "{reason}; ordinary Mint Base layout: {:?}",
                builder.config_params
            )
        })?;
    Ok((builder, jobs, provider_cells, issuer_cells, profile_cells))
}
pub(crate) fn build_ordinary_mint_eq_v1(
    p: &ParamsIPA<EqAffine>,
    w: &OrdinaryMintWitnessV1<'_>,
    root: DigestV1,
    table: &OrdinaryIssuerTableV1,
) -> Result<KagemushaOrdinaryMintEqCircuitV1, String> {
    let h = initial_kagemusha_eq_accumulator_v1(p).map_err(|e| e.to_string())?;
    let (builder, jobs, provider_cells, issuer_cells, profile_cells) =
        build_half::<Fp>(w, root, table, h.as_bytes())?;
    Ok(KagemushaOrdinaryMintEqCircuitV1 {
        builder,
        jobs,
        provider_policy_root: root,
        provider_cells,
        issuer_table: table.clone(),
        issuer_index: table.selected(w.credential.subject.hardware_profile_id)?,
        issuer_cells,
        profile_cells,
    })
}
pub(crate) fn build_ordinary_mint_ep_v1(
    p: &ParamsIPA<EpAffine>,
    w: &OrdinaryMintWitnessV1<'_>,
    root: DigestV1,
    table: &OrdinaryIssuerTableV1,
) -> Result<KagemushaOrdinaryMintEpCircuitV1, String> {
    let h = initial_kagemusha_ep_accumulator_v1(p).map_err(|e| e.to_string())?;
    let (builder, jobs, provider_cells, issuer_cells, profile_cells) =
        build_half::<Fq>(w, root, table, h.as_bytes())?;
    Ok(KagemushaOrdinaryMintEpCircuitV1 {
        builder,
        jobs,
        provider_policy_root: root,
        provider_cells,
        issuer_table: table.clone(),
        issuer_index: table.selected(w.credential.subject.hardware_profile_id)?,
        issuer_cells,
        profile_cells,
    })
}

#[cfg(test)]
#[path = "ordinary_mint_relation_tests.rs"]
mod tests;
