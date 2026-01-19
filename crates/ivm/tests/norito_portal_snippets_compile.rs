use std::{
    any::Any,
    collections::HashMap,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
};

use iroha_primitives::numeric::Numeric;
use ivm::{
    IVM, KotodamaCompiler, VMError,
    host::IVMHost,
    kotodama::compiler::CompilerOptions,
    mock_wsv::{
        AccountId, AssetDefinitionId, DomainId, Mintable, MockWorldStateView, NftId,
        PermissionToken, WsvHost,
    },
    syscalls,
};

fn repository_root() -> PathBuf {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    crate_dir
        .parent()
        .and_then(Path::parent)
        .expect("crates/ivm must live two levels below the repository root")
        .to_path_buf()
}

fn snippets_dir() -> PathBuf {
    repository_root().join("docs/portal/static/norito-snippets")
}

fn snippet_paths() -> Vec<PathBuf> {
    let dir = snippets_dir();
    assert!(
        dir.is_dir(),
        "expected snippets directory at {}",
        dir.display()
    );

    let mut files = Vec::new();
    for entry in fs::read_dir(&dir).expect("read norito-snippets dir") {
        let entry = entry.expect("directory entry");
        let path = entry.path();
        if path.extension() == Some(OsStr::new("ko")) {
            files.push(path);
        }
    }
    assert!(
        !files.is_empty(),
        "expected at least one .ko snippet in {}",
        dir.display()
    );
    files.sort();
    files
}

fn kotodama_compiler() -> KotodamaCompiler {
    KotodamaCompiler::new_with_options(CompilerOptions {
        enforce_on_chain_profile: false,
        ..CompilerOptions::default()
    })
}

#[test]
fn developer_portal_norito_snippets_compile() {
    let compiler = kotodama_compiler();
    for path in snippet_paths() {
        let artifact = compiler
            .compile_file(&path)
            .unwrap_or_else(|err| panic!("failed to compile {}: {err}", path.display()));
        assert!(
            !artifact.is_empty(),
            "compiler produced empty bytecode for {}",
            path.display()
        );
    }
}

#[test]
fn developer_portal_norito_snippets_run() {
    let compiler = kotodama_compiler();
    for path in snippet_paths() {
        let stem = path
            .file_stem()
            .and_then(std::ffi::OsStr::to_str)
            .unwrap_or_default()
            .to_string();
        eprintln!("running snippet {stem}");
        match stem.as_str() {
            "hajimari-entrypoint" => run_hajimari_snippet(&compiler, &path),
            "call-transfer-asset" => run_call_transfer_asset_snippet(&compiler, &path),
            "register-and-mint" => run_register_and_mint_snippet(&compiler, &path),
            "transfer-asset" => run_transfer_asset_snippet(&compiler, &path),
            "nft-flow" => run_nft_flow_snippet(&compiler, &path),
            other => panic!("unexpected snippet {other}"),
        }
    }
}

fn compile_snippet(compiler: &KotodamaCompiler, path: &Path) -> Vec<u8> {
    compiler
        .compile_file(path)
        .unwrap_or_else(|err| panic!("failed to compile {}: {err}", path.display()))
}

fn run_program_with_host<F>(label: &str, bytecode: &[u8], host: WsvHost, check: F)
where
    F: FnOnce(&mut WsvHost),
{
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(host);
    vm.load_program(bytecode)
        .unwrap_or_else(|err| panic!("load developer portal snippet {label}: {err:?}"));
    if let Err(err) = vm.run() {
        panic!("run developer portal snippet {label}: {err:?}");
    }

    let host_any = vm
        .host_mut_any()
        .expect("host must remain attached to the VM");
    let host_ref = host_any
        .downcast_mut::<WsvHost>()
        .expect("developer snippets use WsvHost");
    check(host_ref);
}

#[derive(Default)]
struct LoggingCoreHost {
    inner: ivm::CoreHost,
}

impl LoggingCoreHost {
    fn new() -> Self {
        Self {
            inner: ivm::CoreHost::new(),
        }
    }
}

impl IVMHost for LoggingCoreHost {
    fn syscall(&mut self, number: u32, vm: &mut IVM) -> Result<u64, VMError> {
        match number {
            syscalls::SYSCALL_DEBUG_PRINT | syscalls::SYSCALL_DEBUG_LOG => Ok(0),
            _ => self.inner.syscall(number, vm),
        }
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

fn setup_base_world(domain: &DomainId, caller: &AccountId) -> MockWorldStateView {
    let mut wsv = MockWorldStateView::new();
    wsv.grant_permission(caller, PermissionToken::RegisterDomain);
    wsv.grant_permission(caller, PermissionToken::RegisterAccount);
    assert!(
        wsv.register_domain(caller, domain.clone()),
        "register domain"
    );
    assert!(
        wsv.register_account(caller, caller.clone()),
        "register caller account"
    );
    wsv
}

fn run_hajimari_snippet(compiler: &KotodamaCompiler, path: &Path) {
    let program = compile_snippet(compiler, path);
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(LoggingCoreHost::new());
    vm.load_program(&program)
        .expect("load hajimari snippet into IVM");
    vm.run().expect("run hajimari snippet");
}

fn run_register_and_mint_snippet(compiler: &KotodamaCompiler, path: &Path) {
    let program = compile_snippet(compiler, path);
    let caller: AccountId =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
            .parse()
            .expect("canonical caller");
    let domain = caller.domain().clone();
    let mut wsv = setup_base_world(&domain, &caller);

    let asset_id =
        AssetDefinitionId::from_str("rose#wonderland").expect("asset definition identifier");
    let recipient: AccountId =
        "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland"
            .parse()
            .expect("recipient account");

    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    wsv.grant_permission(&caller, PermissionToken::MintAsset(asset_id.clone()));
    assert!(
        wsv.register_account(&caller, recipient.clone()),
        "register recipient"
    );

    let host = WsvHost::new(wsv, caller.clone(), HashMap::new(), HashMap::new());
    let asset_id_clone = asset_id.clone();
    run_program_with_host("register-and-mint", &program, host, move |host| {
        let balance = host.wsv.balance(recipient.clone(), asset_id_clone.clone());
        assert!(
            balance >= Numeric::from(250_u64),
            "register-and-mint should mint at least 250 units (observed {balance})"
        );
    });
}

fn run_transfer_asset_snippet(compiler: &KotodamaCompiler, path: &Path) {
    let program = compile_snippet(compiler, path);
    let caller: AccountId =
        "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland"
            .parse()
            .expect("transfer caller");
    let domain = caller.domain().clone();
    let mut wsv = setup_base_world(&domain, &caller);

    let recipient: AccountId =
        "ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB@wonderland"
            .parse()
            .expect("recipient account");
    let asset_id =
        AssetDefinitionId::from_str("rose#wonderland").expect("asset definition identifier");

    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    wsv.grant_permission(&caller, PermissionToken::MintAsset(asset_id.clone()));
    assert!(
        wsv.register_account(&caller, recipient.clone()),
        "register recipient"
    );
    assert!(
        wsv.register_asset_definition(&caller, asset_id.clone(), Mintable::Infinitely),
        "seed asset definition"
    );
    assert!(
        wsv.mint(&caller, caller.clone(), asset_id.clone(), Numeric::from(20_u64)),
        "seed caller balance"
    );

    let host = WsvHost::new(wsv, caller.clone(), HashMap::new(), HashMap::new());
    let asset_id_clone = asset_id.clone();
    run_program_with_host("transfer-asset", &program, host, move |host| {
        let caller_balance = host.wsv.balance(caller.clone(), asset_id_clone.clone());
        let recipient_balance = host.wsv.balance(recipient.clone(), asset_id_clone.clone());
        assert!(
            recipient_balance >= Numeric::from(10_u64),
            "recipient balance should increase (observed {recipient_balance})"
        );
        let total = caller_balance
            .checked_add(recipient_balance)
            .expect("sum caller + recipient");
        assert_eq!(total, Numeric::from(20_u64), "transfer preserves total balance");
    });
}

fn run_call_transfer_asset_snippet(compiler: &KotodamaCompiler, path: &Path) {
    let program = compile_snippet(compiler, path);
    let caller: AccountId =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
            .parse()
            .expect("contract account id");
    let domain = caller.domain().clone();
    let mut wsv = setup_base_world(&domain, &caller);

    let alice: AccountId =
        "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland"
            .parse()
            .expect("alice account id");
    let bob: AccountId =
        "ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB@wonderland"
            .parse()
            .expect("bob account id");
    let asset_id =
        AssetDefinitionId::from_str("rose#wonderland").expect("asset definition identifier");

    wsv.grant_permission(&caller, PermissionToken::RegisterAssetDefinition);
    wsv.grant_permission(&caller, PermissionToken::MintAsset(asset_id.clone()));
    wsv.grant_permission(&caller, PermissionToken::TransferAsset(asset_id.clone()));
    assert!(
        wsv.register_account(&caller, alice.clone()),
        "register alice"
    );
    assert!(wsv.register_account(&caller, bob.clone()), "register bob");
    assert!(
        wsv.register_asset_definition(&caller, asset_id.clone(), Mintable::Infinitely),
        "register asset definition"
    );
    assert!(
        wsv.mint(&caller, alice.clone(), asset_id.clone(), Numeric::from(15_u64)),
        "seed alice balance"
    );

    let asset_id_clone = asset_id.clone();
    run_program_with_host(
        "call-transfer-asset",
        &program,
        WsvHost::new(wsv, caller.clone(), HashMap::new(), HashMap::new()),
        move |host| {
            let alice_balance = host.wsv.balance(alice.clone(), asset_id_clone.clone());
            let bob_balance = host.wsv.balance(bob.clone(), asset_id_clone.clone());
            assert!(
                bob_balance >= Numeric::from(10_u64),
                "contract transfer should credit bob (observed {bob_balance})"
            );
            let total = alice_balance
                .checked_add(bob_balance)
                .expect("sum alice + bob");
            assert_eq!(total, Numeric::from(15_u64), "transfer preserves total supply");
        },
    );
}

fn run_nft_flow_snippet(compiler: &KotodamaCompiler, path: &Path) {
    let program = compile_snippet(compiler, path);
    let caller: AccountId =
        "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland"
            .parse()
            .expect("nft owner");
    let domain = caller.domain().clone();
    let mut wsv = setup_base_world(&domain, &caller);

    let recipient: AccountId =
        "ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB@wonderland"
            .parse()
            .expect("nft recipient");
    assert!(
        wsv.register_account(&caller, recipient.clone()),
        "register recipient"
    );

    let nft_id = NftId::from_str("n0$wonderland").expect("nft id");
    let mut vm = IVM::new(u64::MAX);
    vm.set_host(WsvHost::new(
        wsv,
        caller.clone(),
        HashMap::new(),
        HashMap::new(),
    ));
    vm.load_program(&program)
        .expect("load developer portal snippet nft-flow");
    if let Err(err) = vm.run() {
        let host_any = vm
            .host_mut_any()
            .expect("host must remain attached to the VM");
        let host_ref = host_any
            .downcast_mut::<WsvHost>()
            .expect("developer snippets use WsvHost");
        let owner_after = host_ref.wsv.nft_owner(&nft_id);
        panic!(
            "nft-flow failed with {err:?}; owner_after={owner_after:?} issuer_callers={}",
            host_ref.caller
        );
    }
    let host_any = vm
        .host_mut_any()
        .expect("host must remain attached to the VM");
    let host_ref = host_any
        .downcast_mut::<WsvHost>()
        .expect("developer snippets use WsvHost");
    assert!(
        host_ref.wsv.nft_owner(&nft_id).is_none(),
        "nft should be burned by the flow"
    );
}
