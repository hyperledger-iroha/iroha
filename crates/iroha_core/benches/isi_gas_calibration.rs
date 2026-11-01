//! Microbenchmarks to relate native ISI executor latency to the ISI gas meter.

//! Criterion retains execution latency; `ISI_GAS_CALIBRATION` records retain
//! the matching gas denominator for deriving `ns/op`, `gas/op`, and `ns/gas`.
#[path = "../../ivm/benches/support/gas_policy.rs"]
mod gas_policy;
#[path = "support/isi_gas_fixture.rs"]
mod isi_gas_fixture;

use criterion::Criterion;
use iroha_core::{
    executor::{Executor, InstructionExecutionProfile},
    gas as isi_gas,
    state::{State, World},
};
use iroha_data_model::{
    events::execute_trigger::ExecuteTriggerEventFilter,
    prelude::*,
    trigger::{
        Trigger,
        action::{Action, Repeats},
    },
};
use iroha_model_base::domain::DomainId;
use iroha_primitives::json::Json;
use iroha_test_samples::gen_account_in;
const BENCH_DOMAIN_NAME: &str = "wonderland";
const BENCH_DATASPACE_NAME: &str = "universal";
const BENCH_ASSET_NAME: &str = "xor";
#[derive(Clone)]
struct BenchContext {
    authority: AccountId,
    recipient: AccountId,
}
struct BenchState {
    state: State,
    ctx: BenchContext,
}
impl BenchState {
    fn apply_instrs(&mut self, instrs: impl IntoIterator<Item = InstructionBox>) {
        let instructions: Vec<InstructionBox> = instrs.into_iter().collect();
        if instructions.is_empty() {
            return;
        }
        let executor = Executor::default();
        let mut block = self.state.block(isi_gas_fixture::block_header(&self.state));
        let mut tx = block.transaction();
        for instr in instructions {
            executor
                .execute_instruction_with_profile(
                    &mut tx,
                    &self.ctx.authority,
                    instr,
                    InstructionExecutionProfile::Bench,
                )
                .expect("setup instruction execution");
        }
        tx.apply();
        block
            .commit_world_overlay_for_testing()
            .expect("setup block commit");
    }
}
/// Construct a world state with a single domain and two accounts (authority + recipient)
/// suitable for running isolated ISI executor benchmarks.
fn build_bench_state() -> BenchState {
    let (authority, _) = gen_account_in("wonderland");
    let (recipient, _) = gen_account_in("wonderland");
    let domain_id: DomainId =
        DomainId::try_new("wonderland", "universal").expect("valid domain id");
    let domain = Domain::new(domain_id.clone()).build(&authority);
    let authority_account = Account::new(authority.clone()).build(&authority);
    let recipient_account = Account::new(recipient.clone()).build(&recipient);
    let world = World::with([domain], [authority_account, recipient_account], []);
    let state = isi_gas_fixture::state_after_signed_genesis(
        world,
        vec![
            Grant::account_permission(
                iroha_executor_data_model::permission::role::CanManageRoles,
                authority.clone(),
            )
            .into(),
        ],
    );
    BenchState {
        state,
        ctx: BenchContext {
            authority,
            recipient,
        },
    }
}
fn setup_none(_state: &mut BenchState) {}
fn setup_role_exists(state: &mut BenchState) {
    let role_id: RoleId = "bench_role".parse().expect("role id");
    let new_role = Role::new(role_id, state.ctx.authority.clone());
    // The signed genesis grants CanManageRoles to this authority. Setup uses
    // the same permission-enforcing executor as the measured instruction.
    state.apply_instrs([Register::role(new_role).into()]);
}
fn setup_role_assigned(state: &mut BenchState) {
    setup_role_exists(state);
    let role_id: RoleId = "bench_role".parse().expect("role id");
    state.apply_instrs([iroha_data_model::isi::Grant::account_role(
        role_id,
        state.ctx.recipient.clone(),
    )
    .into()]);
}
fn setup_asset_definition(state: &mut BenchState) {
    state.apply_instrs([Register::asset_definition(bench_asset_definition()).into()]);
}
fn setup_asset_and_balance(state: &mut BenchState) {
    setup_asset_definition(state);
    let ad = bench_asset_definition_id();
    let asset_id = AssetId::of(ad, state.ctx.authority.clone());
    state.apply_instrs([iroha_data_model::isi::Mint::asset_quantity(10_u32, asset_id).into()]);
}
fn setup_trigger_registered(state: &mut BenchState) {
    let trigger_id: TriggerId = "bench_trg".parse().expect("trigger id");
    let action = Action::new(
        Vec::<InstructionBox>::new(),
        Repeats::Indefinitely,
        state.ctx.authority.clone(),
        ExecuteTriggerEventFilter::new()
            .for_trigger(trigger_id.clone())
            .under_authority(state.ctx.authority.clone()),
    )
    .expect("trigger action fixture satisfies validation invariants");
    let trigger = Trigger::new(trigger_id, action);
    state.apply_instrs([Register::trigger(trigger).into()]);
}
fn bench_asset_definition_id() -> AssetDefinitionId {
    iroha_data_model::asset::AssetDefinitionId::derive_from_components(
        DomainId::try_new(BENCH_DOMAIN_NAME, BENCH_DATASPACE_NAME).expect("valid domain id"),
        BENCH_ASSET_NAME
            .parse()
            .expect("valid asset definition name"),
    )
}
/// Construct a valid asset definition fixture for registration benchmarks.
fn bench_asset_definition() -> iroha_data_model::asset::NewAssetDefinition {
    isi_gas_fixture::domain_owned_asset_definition(
        DomainId::try_new(BENCH_DOMAIN_NAME, BENCH_DATASPACE_NAME).expect("valid domain id"),
        BENCH_ASSET_NAME,
    )
}
/// Benchmark a single ISI by executing it in a fresh transaction for each iteration.
fn bench_isi(
    c: &mut Criterion,
    label: &str,
    setup: fn(&mut BenchState),
    mk: impl Fn(&BenchContext) -> InstructionBox,
) {
    // Retain the denominator outside Criterion's timing window. The raw samples
    // measure one execution each; benchmark IDs remain stable across profiles.
    let mut bench_state = build_bench_state();
    setup(&mut bench_state);
    let quote_instruction = mk(&bench_state.ctx);
    let gas = isi_gas::meter_instruction(&quote_instruction, &iroha_core::gas::DEFAULT_ZK_GAS)
        .expect("gas quote must complete for this fixture");
    println!("ISI_GAS_CALIBRATION instruction={label} operations=1 total_gas={gas}");
    let mut g = c.benchmark_group("isi-gas-cal");
    g.bench_function(label, |b| {
        // The original committed State stays fixed. Dropping the uncommitted
        // transaction and block restores each iteration without replacing its
        // authenticated root or repeating unrelated genesis startup.
        b.iter(|| {
            let executor = Executor::default();
            let instr = mk(&bench_state.ctx);
            let mut block = bench_state
                .state
                .block(isi_gas_fixture::block_header(&bench_state.state));
            let mut tx = isi_gas_fixture::component_transaction(&mut block);
            std::hint::black_box(
                isi_gas::meter_instruction(&instr, &iroha_core::gas::DEFAULT_ZK_GAS)
                    .expect("gas quote must complete for this fixture"),
            );
            executor
                .execute_instruction_with_profile(
                    &mut tx,
                    &bench_state.ctx.authority,
                    instr,
                    InstructionExecutionProfile::Bench,
                )
                .expect("bench profile execution");
        });
    });
    g.finish();
}
/// Register the individual ISI microbenchmarks for this group.
fn run_benchmarks(c: &mut Criterion) {
    bench_isi(c, "RegisterAccount", setup_none, |_ctx| {
        let (acc, _) = gen_account_in("wonderland");
        Register::account(Account::new(acc.clone())).into()
    });
    bench_isi(c, "RegisterAssetDef", setup_none, |_ctx| {
        Register::asset_definition(bench_asset_definition()).into()
    });
    bench_isi(c, "SetAccountKV_small", setup_none, |ctx| {
        iroha_data_model::isi::SetKeyValue::account(
            ctx.authority.clone(),
            "k".parse().unwrap(),
            Json::new("v"),
        )
        .into()
    });
    bench_isi(c, "GrantAccountRole", setup_role_exists, |ctx| {
        let role: RoleId = "bench_role".parse().unwrap();
        iroha_data_model::isi::Grant::account_role(role, ctx.recipient.clone()).into()
    });
    bench_isi(c, "RevokeAccountRole", setup_role_assigned, |ctx| {
        let role: RoleId = "bench_role".parse().unwrap();
        iroha_data_model::isi::Revoke::account_role(role, ctx.recipient.clone()).into()
    });
    bench_isi(
        c,
        "ExecuteTrigger_empty_args",
        setup_trigger_registered,
        |_ctx| {
            let trg: TriggerId = "bench_trg".parse().unwrap();
            iroha_data_model::isi::ExecuteTrigger::new(trg).into()
        },
    );
    bench_isi(c, "MintAsset", setup_asset_definition, |ctx| {
        let ad = bench_asset_definition_id();
        let id = AssetId::of(ad, ctx.authority.clone());
        iroha_data_model::isi::Mint::asset_quantity(1_u32, id).into()
    });
    bench_isi(c, "TransferAsset", setup_asset_and_balance, |ctx| {
        let ad = bench_asset_definition_id();
        let id = AssetId::of(ad, ctx.authority.clone());
        iroha_data_model::isi::Transfer::asset_quantity(id, 1_u32, ctx.recipient.clone()).into()
    });
}
/// Criterion entrypoint for this benchmark binary.
fn main() {
    gas_policy::configure_from_environment();
    #[allow(unused_imports)]
    {
        use ivm::set_banner_enabled;
        set_banner_enabled(false);
    }
    let mut c = Criterion::default().configure_from_args();
    run_benchmarks(&mut c);
    c.final_summary();
}
