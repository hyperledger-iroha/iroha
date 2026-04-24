use std::collections::HashMap;

use ivm::{
    IVM,
    kotodama::compiler::Compiler as KotodamaCompiler,
    mock_wsv::{MockWorldStateView, PermissionToken, WsvHost},
};

const TEST_ACCOUNT_LITERAL: &str = "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB";
const TEST_CALLER_PUBLIC_KEY: &str =
    "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";

fn literal_account() -> ivm::mock_wsv::AccountId {
    iroha_data_model::account::AccountId::parse_encoded(TEST_ACCOUNT_LITERAL)
        .expect("parse test account literal")
        .into_account_id()
}

fn public_key_account() -> ivm::mock_wsv::AccountId {
    ivm::mock_wsv::AccountId::new(
        TEST_CALLER_PUBLIC_KEY
            .parse()
            .expect("test public key must parse"),
    )
}

#[test]
fn kotodama_create_and_grant_role_enables_mint() {
    let src = r#"
        fn main() {
          // Bootstrap the asset definition used by the role permission.
          register_asset(asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), "ROSE", 0, 1);
          // Create role with mint permission and grant it to the caller.
          create_role(name("minter"), json("{\"perms\":[\"mint_asset:62Fk4FPcMuLvW5QjDGNF2a4jAmjM\"]}"));
          grant_role(authority(), name("minter"));
          // Mint using role permission
          mint_asset(authority(), asset_definition("62Fk4FPcMuLvW5QjDGNF2a4jAmjM"), 1);
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile");
    let caller = literal_account();
    let mut wsv = MockWorldStateView::new();
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    let host = WsvHost::new_with_subject(wsv, caller.clone(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&prog).expect("load");
    vm.run()
        .expect("program should execute with role-created permissions");
}

#[test]
fn kotodama_grant_role_accepts_runtime_account_argument() {
    let src = r#"
        fn grant_it(AccountId who) {
          grant_role(who, name("minter"));
        }

        fn main() {
          create_role(name("minter"), json("{\"perms\":[\"mint_asset:62Fk4FPcMuLvW5QjDGNF2a4jAmjM\"]}"));
          let who = account_id("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB");
          grant_it(who);
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile");
    let caller = public_key_account();
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    wsv.grant_permission(&caller, PermissionToken::RegisterDomain);
    wsv.grant_permission(&caller, PermissionToken::RegisterAccount);
    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    let host = WsvHost::new_with_subject(wsv, caller.clone(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&prog).expect("load");
    vm.run()
        .expect("grant_role should accept runtime account arguments");
}

#[test]
fn kotodama_grant_permission_accepts_runtime_account_argument() {
    let src = r#"
        fn grant_it(AccountId who) {
          grant_permission(who, name("BenefitSpend"));
        }

        fn main() {
          let who = account_id("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB");
          grant_it(who);
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile");
    let caller = public_key_account();
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    wsv.grant_permission(&caller, PermissionToken::ManagePermissions);
    let host = WsvHost::new_with_subject(wsv, caller.clone(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&prog).expect("load");
    vm.run()
        .expect("grant_permission should accept runtime account arguments");
}

#[test]
fn kotodama_runtime_account_argument_survives_syscall_before_grant_permission() {
    let src = r#"
        fn grant_it(AccountId who) {
          let _now = current_time_ms();
          grant_permission(who, name("BenefitSpend"));
        }

        fn main() {
          let who = account_id("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB");
          grant_it(who);
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile");
    let caller = public_key_account();
    let mut wsv = MockWorldStateView::new();
    wsv.add_account_unchecked(caller.clone());
    wsv.grant_permission(&caller, PermissionToken::ManagePermissions);
    let mut host = WsvHost::new_with_subject(wsv, caller.clone(), HashMap::new());
    host.set_current_time_ms(1_717_171_717_000);
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&prog).expect("load");
    vm.run()
        .expect("runtime account arguments must survive intervening syscalls");
}

#[test]
fn kotodama_authority_matches_domainless_account_literal() {
    let src = r#"
        fn main() {
          let who = account_id("sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB");
          assert(authority() == who, "authority should normalize to domainless subject");
        }
    "#;
    let compiler = KotodamaCompiler::new();
    let prog = compiler.compile_source(src).expect("compile");
    let caller = literal_account();
    let wsv = MockWorldStateView::new();
    let host = WsvHost::new_with_subject(wsv, caller.clone(), HashMap::new());
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(&prog).expect("load");
    vm.run()
        .expect("authority() should match the domainless account literal");
}
