//! Both-field active State scope/opening mutations, without Native or finalized funding authority.
use super::super::super::ordinary_mint_public::{
    ordinary_mint_public_column_v1, ordinary_mint_public_data_v1,
};
use super::*;
use crate::pasta_sha256::PastaSha256ConfigV1;
use halo2_base::gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder};
use halo2_proofs::{
    circuit::{Layouter, V1},
    dev::MockProver,
    halo2curves::pasta::{Fp, Fq},
    plonk::{Circuit, ConstraintSystem, Error},
};
use iroha_data_model::testing::ordinary_mint::kagemusha_ordinary_mint_codec_fixture_v1;

const K: usize = 17;
const UNUSABLE: usize = 9;
#[derive(Clone)]
struct BindingCircuit<F: KagemushaPoseidonFieldV1> {
    builder: BaseCircuitBuilder<F>,
    jobs: PastaSha256JobsV1<F>,
}
impl<F: KagemushaPoseidonFieldV1> Circuit<F> for BindingCircuit<F> {
    type Config = (BaseConfig<F>, PastaSha256ConfigV1);
    type FloorPlanner = V1;
    type Params = BaseCircuitParams;
    fn params(&self) -> Self::Params {
        self.builder.config_params.clone()
    }
    fn without_witnesses(&self) -> Self {
        Self {
            builder: self.builder.deep_clone().unknown(true),
            jobs: self.jobs.unknown(),
        }
    }
    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let mut base = BaseConfig::configure(meta, params);
        base.set_usable_rows((1 << K) - UNUSABLE);
        (base, PastaSha256ConfigV1::configure(meta))
    }
    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!("fixed active Mint binding profile")
    }
    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        self.builder
            .synthesize(config.0, layouter.namespace(|| "Mint binding Base"))?;
        self.jobs.synthesize(
            &config.1,
            &mut layouter,
            &self.builder.core().copy_manager,
            (1 << K) - UNUSABLE,
        )
    }
}
#[derive(Clone, Copy)]
enum Mutation {
    Column(usize),
    CreditOpening,
    RecipientOpening,
    RecoveryOpening,
    CreditId,
    Amount,
    Ciphertext,
    MissingOpening,
    InactiveColumn,
}

fn circuit<F: KagemushaPoseidonFieldV1>(
    active: bool,
    mutation: Option<Mutation>,
) -> BindingCircuit<F> {
    let fixture = kagemusha_ordinary_mint_codec_fixture_v1();
    let request = &fixture.request;
    let authorization = &request.authorization;
    let enrollment = fixture.enrollment_fixture.verify(1000).unwrap();
    let c = KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(
        enrollment.app_credential().original(),
    )
    .unwrap();
    let public = ordinary_mint_public_data_v1(
        &authorization.statement,
        &authorization.approval,
        &c,
        None,
        [42; 32],
    )
    .unwrap();
    let original = ordinary_mint_public_column_v1::<F>(&public, &[0; 544]);
    let mut data = if active {
        original.clone()
    } else {
        vec![F::ZERO; 113]
    };
    let mut credit = KagemushaCreditOpeningV1 {
        version: 1,
        credit_id: authorization.statement.credit_id,
        amount: authorization.statement.context.amount,
        credit_commitment_opening: [50; 32],
        recipient_binding_opening: [51; 32],
        recovery_nonce: [52; 32],
    };
    authorization
        .statement
        .context
        .validate_credit_opening(&credit)
        .unwrap();
    let mut cipher = request.encrypted_credit.clone();
    match mutation {
        Some(Mutation::Column(i)) => data[i] += F::ONE,
        Some(Mutation::CreditOpening) => credit.credit_commitment_opening[0] ^= 1,
        Some(Mutation::RecipientOpening) => credit.recipient_binding_opening[0] ^= 1,
        Some(Mutation::RecoveryOpening) => credit.recovery_nonce.fill(0),
        Some(Mutation::CreditId) => credit.credit_id[0] ^= 1,
        Some(Mutation::Amount) => credit.amount += 1,
        Some(Mutation::Ciphertext) => cipher[383] ^= 1,
        Some(Mutation::InactiveColumn) => data[0] = F::ONE,
        _ => {}
    }
    let mut builder = BaseCircuitBuilder::<F>::new(false)
        .use_k(K)
        .use_lookup_bits(16)
        .use_instance_columns(0);
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let digest = |ctx: &mut Context<F>, i: usize| {
        [original[2 * i], original[2 * i + 1]].map(|v| ctx.load_witness(v))
    };
    let bytes = |ctx: &mut Context<F>, i: usize| {
        assign_bytes(ctx, &range, &public.digests[i])
            .try_into()
            .unwrap()
    };
    let state = OrdinaryMintStateBindingCellsV1 {
        operation: ctx.load_witness(F::from(u64::from(active))),
        amount: ctx.load_witness(from_u128::<F>(public.scalars[1])),
        predecessor_outer: digest(ctx, 31),
        predecessor_sequence: ctx.load_witness(from_u128::<F>(public.scalars[4])),
        financial_epoch: digest(ctx, 18),
        release: digest(ctx, 7),
        suite: digest(ctx, 8),
        vk: digest(ctx, 9),
        network: digest(ctx, 11),
        asset: digest(ctx, 12),
        incarnation: digest(ctx, 13),
        pool: digest(ctx, 14),
        lane: digest(ctx, 15),
        profile: digest(ctx, 20),
        scale: ctx.load_witness(from_u128::<F>(public.scalars[2])),
        policy_epoch: ctx.load_witness(from_u128::<F>(public.scalars[3])),
        replay_credit_id: digest(ctx, 24),
        credential: bytes(ctx, 2),
        account_binding: bytes(ctx, 16),
        financial_authority: bytes(ctx, 19),
        provider_root: bytes(ctx, 33),
    };
    let column = data
        .into_iter()
        .map(|v| ctx.load_witness(v))
        .collect::<Vec<_>>();
    let opening = (active && !matches!(mutation, Some(Mutation::MissingOpening))).then_some(
        OrdinaryMintStateOpeningV1 {
            credit: &credit,
            encrypted_credit: &cipher,
        },
    );
    let mut jobs = PastaSha256JobsV1::default();
    constrain_ordinary_mint_state_bindings_v1(ctx, &range, &mut jobs, &column, &state, opening)
        .unwrap();
    assert_eq!(jobs.typed_claim_jobs().unwrap().len(), 3);
    builder.calculate_params(Some(UNUSABLE));
    BindingCircuit { builder, jobs }
}
fn check<F: KagemushaPoseidonFieldV1>(active: bool, mutation: Option<Mutation>) -> bool {
    MockProver::run(K as u32, &circuit::<F>(active, mutation), vec![])
        .unwrap()
        .verify()
        .is_ok()
}
#[test]
fn ordinary_mint113_active_scope_and_private_credit_mutations_reject_both_fields() {
    assert!(check::<Fp>(true, None));
    assert!(check::<Fq>(true, None));
    // Amount/scale/policy/u128 sequence and actual C/financial/asset/lane/predecessor scope.
    for mutation in [
        Mutation::Column(4),
        Mutation::Column(14),
        Mutation::Column(24),
        Mutation::Column(30),
        Mutation::Column(36),
        Mutation::Column(38),
        Mutation::Column(48),
        Mutation::Column(62),
        Mutation::Column(66),
        Mutation::Column(69),
        Mutation::Column(70),
        Mutation::Column(71),
        Mutation::Column(72),
        Mutation::CreditOpening,
        Mutation::RecipientOpening,
        Mutation::RecoveryOpening,
        Mutation::CreditId,
        Mutation::Amount,
        Mutation::Ciphertext,
        Mutation::MissingOpening,
    ] {
        assert!(!check::<Fp>(true, Some(mutation)));
        assert!(!check::<Fq>(true, Some(mutation)));
    }
}
#[test]
fn ordinary_mint113_inactive_semantics_emit_same_opening_sha_graph_and_cannot_carry_credit() {
    assert!(check::<Fp>(false, None));
    assert!(check::<Fq>(false, None));
    assert!(!check::<Fp>(false, Some(Mutation::InactiveColumn)));
    assert!(!check::<Fq>(false, Some(Mutation::InactiveColumn)));
}
