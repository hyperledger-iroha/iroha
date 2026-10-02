//! Exact full ordinary pre-debit authorization and finalized neutral MintCredit originals.
//! Genuine current IPA verification, full history/folds, Native finalized source and current
//! financial/global DATA admission remain mandatory at the enclosing State consumer.
use super::{
    KagemushaPastaParityV1,
    canonical_preimage::{
        assemble_bounded_canonical_frame_v1,
        field_stream::{
            byte_vector_v1, concat_fields_v1, field_v1, framed_hash_v1, struct_payload_v1,
        },
        stream::KagemushaBoundedByteStreamV1,
    },
    composite::{assigned_digest_bytes_v1, assigned_uint_bytes_v1},
    guard_bundle::{assign_bytes, constant_bytes, digest_limbs_assigned},
    mint_authority::public_instance as mint_public,
};
use crate::{
    kagemusha_v1_poseidon::KagemushaPoseidonFieldV1,
    pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1},
};
use halo2_base::{
    AssignedValue, Context, QuantumCell,
    gates::{GateInstructions as _, RangeChip, RangeInstructions as _},
};
use iroha_data_model::kagemusha::*;
// Evaluate nested semantic assignments before lending the same Context to field framing.
macro_rules! payload {
    ($ctx:expr, $range:expr, $grammar:expr; $($field:expr),* $(,)?) => {{
        let fields = [$($field),*];
        struct_payload_v1($ctx, $range, $grammar, &fields)
    }};
}
macro_rules! concatenate {
    ($ctx:expr, $range:expr; $($field:expr),* $(,)?) => {{
        let fields = [$($field),*];
        concat_fields_v1($ctx, $range, &fields)
    }};
}
type Bytes<F> = [PastaSha256ByteV1<F>; 32];
type Stream<F> = KagemushaBoundedByteStreamV1<F>;

/// Only cells already assigned to the exact recursively verified current proof and history.
/// Widths are fixed by the installed protocols, never by the offered proof bytes or envelope cap.
pub(super) struct OrdinaryMintCanonicalProofSourcesV1<'a, F: KagemushaPoseidonFieldV1> {
    /// Exact Eq/Ep protocol digest cells already pinned to the installed ordinary Mint family.
    pub(super) authorization_protocols: [[AssignedValue<F>; 2]; 2],
    pub(super) authorization_column: &'a [AssignedValue<F>],
    pub(super) finalized_column: &'a [AssignedValue<F>],
    pub(super) authorization_current_original: &'a [PastaSha256ByteV1<F>],
    pub(super) finalized_current_original: &'a [PastaSha256ByteV1<F>],
    pub(super) parity: KagemushaPastaParityV1,
    /// EqMint113, EpMint113, EqMintAuthority, EpMintAuthority actual transcript lengths.
    pub(super) proof_widths: [usize; 4],
    pub(super) enabled: AssignedValue<F>,
}

/// Exact constrained originals used to open the finalized IncomingReservation value. This is
/// a mathematical byte result, never a Native source loan or a verified monetary capability.
pub(super) struct OrdinaryMintCanonicalOriginalsV1<F: KagemushaPoseidonFieldV1> {
    pub(super) authorization_original: Stream<F>,
    pub(super) authorization_original_digest: Bytes<F>,
    pub(super) request_original: Stream<F>,
    pub(super) request_original_sha256: Bytes<F>,
    pub(super) finalized_credit_original: Stream<F>,
    pub(super) finalized_credit_original_sha256: Bytes<F>,
    pub(super) lineage_payload: Stream<F>,
    pub(super) source_semantic_digest: Bytes<F>,
    pub(super) predecessor_payload: Stream<F>,
    pub(super) context_digest: Bytes<F>,
    pub(super) clock_digest: Bytes<F>,
}
fn raw<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[u8],
    capacity: usize,
) -> Result<Stream<F>, String> {
    if bytes.len() > capacity {
        return Err("ordinary original exceeds installed stream capacity".into());
    }
    let mut padded = bytes.to_vec();
    padded.resize(capacity, 0);
    let assigned = assign_bytes(ctx, range, &padded);
    let len = ctx.load_witness(F::from(bytes.len() as u64));
    Stream::constrain(ctx, range, assigned, len)
}
fn fixed<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: Vec<PastaSha256ByteV1<F>>,
) -> Result<Stream<F>, String> {
    let len = ctx.load_constant(F::from(bytes.len() as u64));
    Stream::constrain(ctx, range, bytes, len)
}
fn integer<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    value: AssignedValue<F>,
    bits: usize,
) -> Result<Stream<F>, String> {
    range.range_check(ctx, value, bits);
    let bytes = assigned_uint_bytes_v1(ctx, range.gate(), value, bits);
    fixed(ctx, range, bytes)
}
fn literal<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &[u8],
) -> Result<Stream<F>, String> {
    fixed(ctx, range, constant_bytes(bytes))
}
fn digest<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    column: &[AssignedValue<F>],
    offset: usize,
) -> Result<Stream<F>, String> {
    let cells = column
        .get(offset..offset + 2)
        .ok_or("ordinary digest column truncated")?;
    let bytes = assigned_digest_bytes_v1(ctx, range.gate(), [cells[0], cells[1]]);
    fixed(ctx, range, bytes)
}
fn protocol_digest<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    cells: [AssignedValue<F>; 2],
) -> Result<Stream<F>, String> {
    let bytes = assigned_digest_bytes_v1(ctx, range.gate(), cells);
    fixed(ctx, range, bytes)
}
fn bytes_digest<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    bytes: &Bytes<F>,
) -> Result<Stream<F>, String> {
    fixed(ctx, range, bytes.to_vec())
}
fn equal_digest_if<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    actual: &Bytes<F>,
    column: &[AssignedValue<F>],
    offset: usize,
    enabled: AssignedValue<F>,
) -> Result<(), String> {
    let expected = column
        .get(offset..offset + 2)
        .ok_or("ordinary digest equality column truncated")?;
    for (actual, expected) in digest_limbs_assigned(ctx, actual).into_iter().zip(expected) {
        let difference = range.gate().sub(ctx, actual, *expected);
        let invalid = range.gate().mul(ctx, enabled, difference);
        range.gate().assert_is_const(ctx, &invalid, &F::ZERO);
    }
    Ok(())
}
fn raw_hash<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    raw: &Stream<F>,
) -> Result<Bytes<F>, String> {
    let words = jobs.digest_bounded_constrained(ctx, range, raw.bytes(), raw.actual_len())?;
    let mut bytes = Vec::new();
    for word in words {
        let bits = PastaSha256BitV1::decompose(ctx, range.gate(), word, 32);
        for offset in [24, 16, 8, 0] {
            bytes.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                range.gate(),
                &bits[offset..offset + 8],
            ));
        }
    }
    bytes
        .try_into()
        .map_err(|_| "ordinary complete raw SHA width".into())
}
fn grammar<T: KagemushaOrdinaryCanonicalFieldStreamV1>(
    value: &T,
) -> Result<KagemushaOrdinaryCanonicalFieldStreamGrammarV1, String> {
    KagemushaOrdinaryCanonicalFieldStreamGrammarV1::from_original(value)
}
fn history<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    column: &[AssignedValue<F>],
    start: usize,
) -> Result<Stream<F>, String> {
    let cells = column
        .get(start..start + 34)
        .ok_or("ordinary full history truncated")?;
    let mut bytes = Vec::new();
    for cell in cells {
        bytes.extend(assigned_uint_bytes_v1(ctx, range.gate(), *cell, 128));
    }
    fixed(ctx, range, bytes)
}
fn history_original<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    original: &[u8],
) -> Result<Stream<F>, String> {
    if original.len() != super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1 {
        return Err("ordinary counterpart full history original has wrong length".into());
    }
    raw(
        ctx,
        range,
        original,
        super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1,
    )
}
fn proof_streams<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    parity: KagemushaPastaParityV1,
    current: &[PastaSha256ByteV1<F>],
    eq: &[u8],
    ep: &[u8],
    widths: [usize; 2],
) -> Result<[Stream<F>; 2], String> {
    if eq.len() != widths[0]
        || ep.len() != widths[1]
        || current.len()
            != widths[if parity == KagemushaPastaParityV1::Eq {
                0
            } else {
                1
            }]
    {
        return Err("ordinary current proof differs from installed transcript length".into());
    }
    let eq = if parity == KagemushaPastaParityV1::Eq {
        fixed(ctx, range, current.to_vec())?
    } else {
        raw(ctx, range, eq, widths[0])?
    };
    let ep = if parity == KagemushaPastaParityV1::Ep {
        fixed(ctx, range, current.to_vec())?
    } else {
        raw(ctx, range, ep, widths[1])?
    };
    Ok([eq, ep])
}
fn enum_evidence<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    column: &[AssignedValue<F>],
    evidence: &KagemushaAppOperationApprovalEvidenceV1,
) -> Result<(Stream<F>, Stream<F>), String> {
    let actual = match evidence {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => {
            signature_der.as_slice()
        }
        KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => {
            raw_assertion.as_slice()
        }
    };
    let actual = raw(
        ctx,
        range,
        actual,
        KAGEMUSHA_ORDINARY_APPLE_ASSERTION_MAX_BYTES_V1,
    )?;
    let grammar = KagemushaOrdinaryApprovalEvidenceStreamGrammarV1::from_sole_encoder()?;
    let apple = column[78];
    range.range_check(ctx, apple, 1);
    let tag = grammar
        .android_tag()
        .into_iter()
        .zip(grammar.apple_tag())
        .map(|(android, ios)| {
            let selected = range.gate().select(
                ctx,
                QuantumCell::Constant(F::from(u64::from(ios))),
                QuantumCell::Constant(F::from(u64::from(android))),
                apple,
            );
            PastaSha256ByteV1::range_checked(ctx, range, selected)
        })
        .collect();
    let tag = fixed(ctx, range, tag)?;
    let vector = byte_vector_v1(ctx, range, &actual)?;
    let field = field_v1(ctx, range, &vector)?;
    Ok((concatenate!(ctx, range; tag, field)?, actual))
}

/// Open the entire ordinary hierarchy, with the current same-parity proof bytes and544 history
/// sourced directly from its recursive verifier. The other parity consumes its counterpart in
/// the same graph and both State proofs expose the same IncomingReservation original digest.
/// No OEM authorization offset, hardware credential or leaf-audit alias enters this family.
/// Inactive operations run this identical graph but select no semantic/hash authority.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn constrain_ordinary_mint_canonical_originals_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    sources: OrdinaryMintCanonicalProofSourcesV1<'_, F>,
    authorization: &KagemushaOrdinaryMintAuthorizationV1,
    credit: &KagemushaMintCreditV1,
) -> Result<OrdinaryMintCanonicalOriginalsV1<F>, String> {
    let column = sources.authorization_column;
    let mint = sources.finalized_column;
    let enabled = sources.enabled;
    if column.len() != 113 || mint.len() != 56 {
        return Err("ordinary Mint113/finalized56 original columns differ".into());
    }
    range.range_check(ctx, enabled, 1);
    let context = &authorization.statement.context;
    let owner = &context.lineage.owner;
    let rt = &owner.runtime;
    let cg = grammar(context)?;
    let lg = grammar(&context.lineage)?;
    let og = grammar(owner)?;
    let rg = grammar(rt)?;
    let account_payload = raw(ctx, range, &og.fields()[0], 4096)?;
    let account_prefix =
        kagemusha_canonical_mint_frame_prefix_v1(&owner.account_id).map_err(|e| e.to_string())?;
    let account_frame =
        assemble_bounded_canonical_frame_v1(ctx, range, &account_prefix, &account_payload)?;
    for (domain, index) in [
        (b"iroha:kagemusha:v1:app-approval-account\0".as_slice(), 16),
        (b"iroha:kagemusha:v1:account-identity\0".as_slice(), 17),
    ] {
        let sha = framed_hash_v1(ctx, range, jobs, domain, &account_frame)?;
        equal_digest_if(ctx, range, &sha, column, 2 * index, enabled)?;
    }
    let asset_payload = raw(ctx, range, &rg.fields()[4], 64)?;
    let asset_prefix =
        kagemusha_canonical_mint_frame_prefix_v1(&rt.asset).map_err(|e| e.to_string())?;
    let asset_frame =
        assemble_bounded_canonical_frame_v1(ctx, range, &asset_prefix, &asset_payload)?;
    let asset_sha = framed_hash_v1(
        ctx,
        range,
        jobs,
        b"iroha:kagemusha:v1:asset-identity\0",
        &asset_frame,
    )?;
    equal_digest_if(ctx, range, &asset_sha, column, 24, enabled)?;
    let incarnation_grammar = grammar(&rt.asset_incarnation)?;
    let incarnation_payload =
        payload!(ctx, range, &incarnation_grammar; digest(ctx, range, column, 26)?)?;
    let runtime_fields = vec![
        raw(
            ctx,
            range,
            &rg.fields()[0],
            KAGEMUSHA_RETAIL_ENROLLMENT_POLICY_MAX_BYTES_V1,
        )?,
        raw(ctx, range, &rg.fields()[1], 8)?,
        raw(
            ctx,
            range,
            &rg.fields()[2],
            KAGEMUSHA_RETAIL_ENROLLMENT_POLICY_MAX_BYTES_V1,
        )?,
        digest(ctx, range, column, 22)?,
        asset_payload.clone(),
        incarnation_payload.clone(),
        integer(ctx, range, column[70], 32)?,
    ];
    let runtime_payload = struct_payload_v1(ctx, range, &rg, &runtime_fields)?;
    let owner_payload = payload!(ctx, range, &og;
        account_payload.clone(),
        runtime_payload,
        digest(ctx, range, column, 30)?,
    )?;
    let one = literal(ctx, range, &1_u16.to_le_bytes())?;
    let lineage_payload = payload!(ctx, range, &lg;
        one.clone(),
        owner_payload,
        digest(ctx, range, column, 36)?,
        digest(ctx, range, column, 38)?,
    )?;
    let hg = grammar(&context.predecessor)?;
    let predecessor_payload = payload!(ctx, range, &hg;
        digest(ctx, range, column, 62)?,
        integer(ctx, range, column[72], 128)?,
        digest(ctx, range, column, 64)?,
    )?;
    let clockg = grammar(&context.clock_context)?;
    let clock_nonce = digest(ctx, range, column, 58)?;
    let observations = digest(ctx, range, column, 60)?;
    let lower = integer(ctx, range, column[73], 64)?;
    let upper = integer(ctx, range, column[74], 64)?;
    let clock_payload = payload!(ctx, range, &clockg;
        one.clone(),
        clock_nonce.clone(),
        observations.clone(),
        lower.clone(),
        upper.clone(),
    )?;
    let clock_message = concatenate!(ctx, range;
        literal(ctx, range, KAGEMUSHA_ORDINARY_CASH_CLOCK_DOMAIN_V1)?,
        one.clone(),
        clock_nonce,
        observations,
        lower,
        upper,
    )?;
    let clock_digest = raw_hash(ctx, range, jobs, &clock_message)?;
    equal_digest_if(ctx, range, &clock_digest, column, 52, enabled)?;
    let mut context_fields = vec![
        one.clone(),
        digest(ctx, range, column, 12)?,
        lineage_payload.clone(),
        predecessor_payload.clone(),
    ];
    for i in 7..=10 {
        context_fields.push(digest(ctx, range, column, 2 * i)?);
    }
    context_fields.extend([
        digest(ctx, range, column, 4)?,
        digest(ctx, range, column, 40)?,
        integer(ctx, range, column[71], 64)?,
        integer(ctx, range, column[69], 128)?,
        digest(ctx, range, column, 42)?,
        digest(ctx, range, column, 44)?,
        digest(ctx, range, column, 46)?,
        clock_payload,
        digest(ctx, range, column, 54)?,
    ]);
    let context_payload = struct_payload_v1(ctx, range, &cg, &context_fields)?;
    let context_frame =
        assemble_bounded_canonical_frame_v1(ctx, range, cg.framing(), &context_payload)?;
    let context_digest = framed_hash_v1(
        ctx,
        range,
        jobs,
        b"iroha:kagemusha:v1:ordinary-mint-context\0",
        &context_frame,
    )?;
    equal_digest_if(ctx, range, &context_digest, column, 2, enabled)?;
    let operation = digest(ctx, range, column, 12)?;
    let issuance_message = concatenate!(ctx, range;
        literal(ctx, range, b"iroha:kagemusha:v1:ordinary-mint-issuance\0")?,
        operation,
        bytes_digest(ctx, range, &context_digest)?,
    )?;
    let issuance = raw_hash(ctx, range, jobs, &issuance_message)?;
    let sg = grammar(&authorization.statement)?;
    let statement_payload = payload!(ctx, range, &sg;
        one.clone(),
        context_payload,
        bytes_digest(ctx, range, &issuance)?,
        digest(ctx, range, column, 48)?,
        digest(ctx, range, column, 50)?,
    )?;
    let statement_frame =
        assemble_bounded_canonical_frame_v1(ctx, range, sg.framing(), &statement_payload)?;
    let statement_sha = framed_hash_v1(
        ctx,
        range,
        jobs,
        b"iroha:kagemusha:v1:ordinary-mint-statement\0",
        &statement_frame,
    )?;
    equal_digest_if(ctx, range, &statement_sha, column, 0, enabled)?;
    let challenge = &authorization.approval.challenge;
    let ag = grammar(challenge)?;
    let challenge_payload = payload!(ctx, range, &ag;
        one.clone(),
        digest(ctx, range, column, 12)?,
        digest(ctx, range, column, 56)?,
        digest(ctx, range, column, 4)?,
        bytes_digest(ctx, range, &statement_sha)?,
        bytes_digest(ctx, range, &clock_digest)?,
        digest(ctx, range, column, 54)?,
        integer(ctx, range, column[75], 64)?,
        integer(ctx, range, column[76], 64)?,
    )?;
    let (evidence_payload, evidence_original) =
        enum_evidence(ctx, range, column, &authorization.approval.evidence)?;
    let evidence_sha = raw_hash(ctx, range, jobs, &evidence_original)?;
    equal_digest_if(ctx, range, &evidence_sha, column, 8, enabled)?;
    let approval_g = grammar(&authorization.approval)?;
    let approval_payload = payload!(ctx, range, &approval_g; challenge_payload, evidence_payload)?;
    let approval_frame =
        assemble_bounded_canonical_frame_v1(ctx, range, approval_g.framing(), &approval_payload)?;
    let approval_sha = framed_hash_v1(
        ctx,
        range,
        jobs,
        b"iroha:kagemusha:v1:ordinary-mint-approval-original\0",
        &approval_frame,
    )?;
    equal_digest_if(ctx, range, &approval_sha, column, 6, enabled)?;
    let ap = &authorization.proof;
    let pg = grammar(ap)?;
    let auth_proofs = proof_streams(
        ctx,
        range,
        sources.parity,
        sources.authorization_current_original,
        &ap.eq_proof,
        &ap.ep_proof,
        [sources.proof_widths[0], sources.proof_widths[1]],
    )?;
    let auth_history = history(ctx, range, column, 79)?;
    let eq_history = if sources.parity == KagemushaPastaParityV1::Eq {
        auth_history.clone()
    } else {
        history_original(ctx, range, &ap.eq_history)?
    };
    let ep_history = if sources.parity == KagemushaPastaParityV1::Ep {
        auth_history
    } else {
        history_original(ctx, range, &ap.ep_history)?
    };
    let paired_payload = payload!(ctx, range, &pg;
        one.clone(),
        protocol_digest(ctx, range, sources.authorization_protocols[0])?,
        protocol_digest(ctx, range, sources.authorization_protocols[1])?,
        bytes_digest(ctx, range, &statement_sha)?,
        bytes_digest(ctx, range, &approval_sha)?,
        byte_vector_v1(ctx, range, &auth_proofs[0])?,
        byte_vector_v1(ctx, range, &auth_proofs[1])?,
        byte_vector_v1(ctx, range, &eq_history)?,
        byte_vector_v1(ctx, range, &ep_history)?,
    )?;
    let authorization_g = grammar(authorization)?;
    let authorization_payload = payload!(ctx, range, &authorization_g;
        one.clone(),
        statement_payload,
        approval_payload,
        paired_payload,
    )?;
    let authorization_original = assemble_bounded_canonical_frame_v1(
        ctx,
        range,
        authorization_g.framing(),
        &authorization_payload,
    )?;
    let authorization_original_digest = framed_hash_v1(
        ctx,
        range,
        jobs,
        b"iroha:kagemusha:v1:ordinary-mint-authorization\0",
        &authorization_original,
    )?;
    // The rest of the finalized hierarchy is constructed below from actual MintAuthority cells.
    let lifecycle = &credit.statement.lifecycle;
    let lifecycle_g = grammar(lifecycle)?;
    let mut lifecycle_fields = vec![
        one.clone(),
        digest(ctx, range, column, 22)?,
        one.clone(),
        digest(ctx, range, column, 16)?,
        digest(ctx, range, column, 18)?,
        digest(ctx, range, column, 14)?,
        asset_payload,
        incarnation_payload,
        integer(ctx, range, column[70], 32)?,
        digest(ctx, range, column, 28)?,
        digest(ctx, range, column, 40)?,
        integer(ctx, range, column[71], 64)?,
    ];
    let operation_kind = literal(
        ctx,
        range,
        &norito::codec::encode_adaptive(&KagemushaOperationKindV1::MintFold),
    )?;
    lifecycle_fields.extend([
        operation_kind,
        literal(ctx, range, &[0; 32])?,
        literal(ctx, range, &[0; 32])?,
        digest(ctx, range, column, 48)?,
        digest(ctx, range, column, 50)?,
    ]);
    let lifecycle_payload = struct_payload_v1(ctx, range, &lifecycle_g, &lifecycle_fields)?;
    let credit_statement_g = grammar(&credit.statement)?;
    let minted_at = ctx.load_witness(F::from(credit.statement.minted_at_ms));
    let credit_statement_payload = payload!(ctx, range, &credit_statement_g;
        one.clone(),
        lifecycle_payload,
        digest(ctx, range, column, 42)?,
        bytes_digest(ctx, range, &context_digest)?,
        bytes_digest(ctx, range, &authorization_original_digest)?,
        integer(ctx, range, column[69], 128)?,
        bytes_digest(ctx, range, &issuance)?,
        account_payload,
        digest(ctx, range, column, 44)?,
        integer(ctx, range, minted_at, 64)?,
    )?;
    let credit_statement_frame = assemble_bounded_canonical_frame_v1(
        ctx,
        range,
        credit_statement_g.framing(),
        &credit_statement_payload,
    )?;
    let semantic = framed_hash_v1(
        ctx,
        range,
        jobs,
        b"iroha:kagemusha:v1:mint-statement\0",
        &credit_statement_frame,
    )?;
    equal_digest_if(
        ctx,
        range,
        &semantic,
        mint,
        mint_public::SEMANTIC_LO,
        enabled,
    )?;
    let p = &credit.proof;
    let credit_proof_g = grammar(p)?;
    let finalized_proofs = proof_streams(
        ctx,
        range,
        sources.parity,
        sources.finalized_current_original,
        &p.eq_proof,
        &p.ep_proof,
        [sources.proof_widths[2], sources.proof_widths[3]],
    )?;
    let finalized_history = history(ctx, range, mint, mint_public::HISTORY_START)?;
    let eq_history = if sources.parity == KagemushaPastaParityV1::Eq {
        finalized_history.clone()
    } else {
        history_original(ctx, range, &p.eq_history)?
    };
    let ep_history = if sources.parity == KagemushaPastaParityV1::Ep {
        finalized_history
    } else {
        history_original(ctx, range, &p.ep_history)?
    };
    let mut credit_proof_fields = vec![one.clone()];
    for offset in [
        mint_public::EQ_PROTOCOL_LO,
        mint_public::EP_PROTOCOL_LO,
        mint_public::SEMANTIC_LO,
        mint_public::CERTIFICATE_LO,
        mint_public::AUTHORITY_LO,
        mint_public::EQ_AUDIT_LO,
        mint_public::EP_AUDIT_LO,
    ] {
        credit_proof_fields.push(digest(ctx, range, mint, offset)?);
    }
    credit_proof_fields.extend([
        byte_vector_v1(ctx, range, &finalized_proofs[0])?,
        byte_vector_v1(ctx, range, &finalized_proofs[1])?,
        byte_vector_v1(ctx, range, &eq_history)?,
        byte_vector_v1(ctx, range, &ep_history)?,
    ]);
    let credit_proof_payload =
        struct_payload_v1(ctx, range, &credit_proof_g, &credit_proof_fields)?;
    let cipher = raw(
        ctx,
        range,
        &credit.encrypted_credit,
        KAGEMUSHA_ENCRYPTED_CREDIT_MAX_BYTES_V1,
    )?;
    let cipher_sha = framed_hash_v1(
        ctx,
        range,
        jobs,
        b"iroha:kagemusha:v1:ciphertext\0",
        &cipher,
    )?;
    equal_digest_if(ctx, range, &cipher_sha, column, 50, enabled)?;
    let credit_g = grammar(credit)?;
    let finalized_credit_payload = payload!(ctx, range, &credit_g;
        one.clone(),
        credit_statement_payload,
        credit_proof_payload,
        digest(ctx, range, mint, mint_public::CERTIFICATE_LO)?,
        digest(ctx, range, mint, mint_public::AUTHORITY_LO)?,
        digest(ctx, range, mint, mint_public::GENESIS_LO)?,
        digest(ctx, range, mint, mint_public::PAIR_BINDING_LO)?,
        byte_vector_v1(ctx, range, &cipher)?,
        digest(ctx, range, column, 20)?,
    )?;
    let finalized_credit_original = assemble_bounded_canonical_frame_v1(
        ctx,
        range,
        credit_g.framing(),
        &finalized_credit_payload,
    )?;
    let finalized_credit_original_sha256 = raw_hash(ctx, range, jobs, &finalized_credit_original)?;
    let request_g = grammar(&KagemushaOrdinaryTopUpRequestV1 {
        version: 1,
        authorization: authorization.clone(),
        encrypted_credit: credit.encrypted_credit.clone(),
    })?;
    let request_payload = payload!(ctx, range, &request_g;
        one,
        authorization_payload,
        byte_vector_v1(ctx, range, &cipher)?,
    )?;
    let request_original =
        assemble_bounded_canonical_frame_v1(ctx, range, request_g.framing(), &request_payload)?;
    let request_original_sha256 = raw_hash(ctx, range, jobs, &request_original)?;
    Ok(OrdinaryMintCanonicalOriginalsV1 {
        authorization_original,
        authorization_original_digest,
        request_original,
        request_original_sha256,
        finalized_credit_original,
        finalized_credit_original_sha256,
        lineage_payload,
        source_semantic_digest: semantic,
        predecessor_payload,
        context_digest,
        clock_digest,
    })
}

/// Complete Mint-side IncomingReservation source opening. The finalized-source SHA is copied
/// from the independently authenticated full debit/finality original by the Native/Core owner;
/// the exact neutral credit SHA and semantic are derived here from actual recursive proof cells.
/// This does not authenticate an offered finality SHA or create a global reservation capability.
#[allow(clippy::too_many_arguments)]
pub(super) fn constrain_ordinary_mint_incoming_reservation_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    range: &RangeChip<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    column: &[AssignedValue<F>],
    originals: &OrdinaryMintCanonicalOriginalsV1<F>,
    reservation: &KagemushaOrdinaryIncomingReservationV1,
    finalized_source_original_sha256: &Bytes<F>,
    state_envelope: [AssignedValue<F>; 2],
    enabled: AssignedValue<F>,
) -> Result<Bytes<F>, String> {
    if column.len() != 113 {
        return Err("ordinary incoming Mint reservation column differs".into());
    }
    let g = grammar(&reservation.selection)?;
    let rg = grammar(reservation)?;
    // Only the sole codec's neutral Mint discriminant is constant. No finalized source,
    // approval, proof, receipt or Native owner is manufactured by this data-only specimen.
    let specimen =
        norito::codec::encode_adaptive(&KagemushaOrdinaryIncomingSourceSelectionV1::Mint {
            topup_request_original_sha256: [0; 32],
        });
    let tag = specimen
        .get(..4)
        .ok_or("ordinary incoming Mint enum tag width")?;
    let source_digest = bytes_digest(ctx, range, &originals.request_original_sha256)?;
    let source_field = field_v1(ctx, range, &source_digest)?;
    let source = concatenate!(ctx, range; literal(ctx, range, tag)?, source_field)?;
    let payload = payload!(ctx, range, &g;
        literal(ctx, range, &1_u16.to_le_bytes())?,
        originals.lineage_payload.clone(),
        digest(ctx, range, column, 12)?,
        originals.predecessor_payload.clone(),
        source,
        digest(ctx, range, column, 48)?,
        integer(ctx, range, column[69], 128)?,
        integer(ctx, range, column[70], 32)?,
        digest(ctx, range, column, 4)?,
        digest(ctx, range, column, 54)?,
        bytes_digest(ctx, range, &originals.clock_digest)?,
    )?;
    let reservation_payload = payload!(ctx, range, &rg;
        payload,
        bytes_digest(ctx, range, finalized_source_original_sha256)?,
        bytes_digest(ctx, range, &originals.finalized_credit_original_sha256)?,
        bytes_digest(ctx, range, &originals.source_semantic_digest)?,
    )?;
    let full = assemble_bounded_canonical_frame_v1(ctx, range, rg.framing(), &reservation_payload)?;
    let envelope = framed_hash_v1(
        ctx,
        range,
        jobs,
        b"iroha:kagemusha:v1:ordinary-incoming-reservation\0",
        &full,
    )?;
    equal_digest_if(ctx, range, &envelope, &state_envelope, 0, enabled)?;
    Ok(envelope)
}

#[cfg(test)]
#[path = "ordinary_mint_full_canonical_consumer_tests.rs"]
mod tests;
