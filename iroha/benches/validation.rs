use criterion::*;
use iroha::{crypto, isi::prelude::*, prelude::*};

fn accept_transaction(criterion: &mut Criterion) {
    let domain_name = "domain2";
    let create_domain = CreateDomain {
        domain_name: domain_name.to_string(),
    };
    let account_id = Id::new("account2", domain_name);
    let create_account = CreateAccount {
        account_id: account_id.clone(),
        domain_name: domain_name.to_string(),
        public_key: [63; 32],
    };
    let asset_id = Id::new("xor", domain_name);
    let create_asset = AddAssetQuantity {
        asset_id: asset_id.clone(),
        account_id: account_id.clone(),
        amount: 100,
    };
    let commands: Vec<Contract> = vec![
        create_domain.into(),
        create_account.into(),
        create_asset.into(),
    ];
    let transaction = RequestedTransaction::new(commands, Id::new("account", "domain"));
    let mut success_count = 0;
    let mut failures_count = 0;
    criterion.bench_function("accept", |b| {
        b.iter(|| match transaction.clone().accept() {
            Ok(_) => success_count += 1,
            Err(_) => failures_count += 1,
        });
    });
    println!(
        "Success count: {}, Failures count: {}",
        success_count, failures_count
    );
}

fn sign_transaction(criterion: &mut Criterion) {
    let domain_name = "domain2";
    let create_domain = CreateDomain {
        domain_name: domain_name.to_string(),
    };
    let account_id = Id::new("account2", domain_name);
    let create_account = CreateAccount {
        account_id: account_id.clone(),
        domain_name: domain_name.to_string(),
        public_key: [63; 32],
    };
    let asset_id = Id::new("xor", domain_name);
    let create_asset = AddAssetQuantity {
        asset_id: asset_id.clone(),
        account_id: account_id.clone(),
        amount: 100,
    };
    let commands: Vec<Contract> = vec![
        create_domain.into(),
        create_account.into(),
        create_asset.into(),
    ];
    let transaction = RequestedTransaction::new(commands, Id::new("account", "domain"))
        .accept()
        .expect("Failed to accept transaction.");
    let (public_key, private_key) =
        crypto::generate_key_pair().expect("Failed to generate key pair.");
    let mut success_count = 0;
    let mut failures_count = 0;
    criterion.bench_function("sign", |b| {
        b.iter(
            || match transaction.clone().sign(&public_key, &private_key) {
                Ok(_) => success_count += 1,
                Err(_) => failures_count += 1,
            },
        );
    });
    println!(
        "Success count: {}, Failures count: {}",
        success_count, failures_count
    );
}

fn validate_transaction(criterion: &mut Criterion) {
    let domain_name = "domain2";
    let create_domain = CreateDomain {
        domain_name: domain_name.to_string(),
    };
    let account_id = Id::new("account2", domain_name);
    let create_account = CreateAccount {
        account_id: account_id.clone(),
        domain_name: domain_name.to_string(),
        public_key: [63; 32],
    };
    let asset_id = Id::new("xor", domain_name);
    let create_asset = AddAssetQuantity {
        asset_id: asset_id.clone(),
        account_id: account_id.clone(),
        amount: 100,
    };
    let commands: Vec<Contract> = vec![
        create_domain.into(),
        create_account.into(),
        create_asset.into(),
    ];
    let (public_key, private_key) =
        crypto::generate_key_pair().expect("Failed to generate key pair.");
    let transaction = RequestedTransaction::new(commands, Id::new("account", "domain"))
        .accept()
        .expect("Failed to accept transaction.")
        .sign(&public_key, &private_key)
        .expect("Failed to sign transaction.");
    let mut success_count = 0;
    let mut failures_count = 0;
    let mut world_state_view = WorldStateView::new(Peer::new("127.0.0.1".to_string(), &Vec::new()));
    criterion.bench_function("validate", |b| {
        b.iter(
            || match transaction.clone().validate(&mut world_state_view) {
                Ok(_) => success_count += 1,
                Err(_) => failures_count += 1,
            },
        );
    });
    println!(
        "Success count: {}, Failures count: {}",
        success_count, failures_count
    );
}

fn chain_blocks(criterion: &mut Criterion) {
    let domain_name = "domain2";
    let create_domain = CreateDomain {
        domain_name: domain_name.to_string(),
    };
    let account_id = Id::new("account2", domain_name);
    let create_account = CreateAccount {
        account_id: account_id.clone(),
        domain_name: domain_name.to_string(),
        public_key: [63; 32],
    };
    let asset_id = Id::new("xor", domain_name);
    let create_asset = AddAssetQuantity {
        asset_id: asset_id.clone(),
        account_id: account_id.clone(),
        amount: 100,
    };
    let commands: Vec<Contract> = vec![
        create_domain.into(),
        create_account.into(),
        create_asset.into(),
    ];
    let transaction = RequestedTransaction::new(commands, Id::new("account", "domain"))
        .accept()
        .expect("Failed to accept transaction.");
    let block = PendingBlock::new(vec![transaction]);
    let mut previous_block_hash = block.clone().chain_first().hash();
    let mut success_count = 0;
    criterion.bench_function("chain_block", |b| {
        b.iter(|| {
            success_count += 1;
            let new_block = block.clone().chain(success_count, previous_block_hash);
            previous_block_hash = new_block.hash();
        });
    });
    println!("Total count: {}", success_count);
}

fn sign_blocks(criterion: &mut Criterion) {
    let domain_name = "domain2";
    let create_domain = CreateDomain {
        domain_name: domain_name.to_string(),
    };
    let account_id = Id::new("account2", domain_name);
    let create_account = CreateAccount {
        account_id: account_id.clone(),
        domain_name: domain_name.to_string(),
        public_key: [63; 32],
    };
    let asset_id = Id::new("xor", domain_name);
    let create_asset = AddAssetQuantity {
        asset_id: asset_id.clone(),
        account_id: account_id.clone(),
        amount: 100,
    };
    let commands: Vec<Contract> = vec![
        create_domain.into(),
        create_account.into(),
        create_asset.into(),
    ];
    let transaction = RequestedTransaction::new(commands, Id::new("account", "domain"))
        .accept()
        .expect("Failed to accept transaction.");
    let block = PendingBlock::new(vec![transaction]).chain_first();
    let (public_key, private_key) =
        crypto::generate_key_pair().expect("Failed to generate key pair.");
    let mut success_count = 0;
    let mut failures_count = 0;
    criterion.bench_function("sign_block", |b| {
        b.iter(|| match block.clone().sign(&public_key, &private_key) {
            Ok(_) => success_count += 1,
            Err(_) => failures_count += 1,
        });
    });
    println!(
        "Success count: {}, Failures count: {}",
        success_count, failures_count
    );
}

fn validate_blocks(criterion: &mut Criterion) {
    let domain_name = "domain2";
    let create_domain = CreateDomain {
        domain_name: domain_name.to_string(),
    };
    let account_id = Id::new("account2", domain_name);
    let create_account = CreateAccount {
        account_id: account_id.clone(),
        domain_name: domain_name.to_string(),
        public_key: [63; 32],
    };
    let asset_id = Id::new("xor", domain_name);
    let create_asset = AddAssetQuantity {
        asset_id: asset_id.clone(),
        account_id: account_id.clone(),
        amount: 100,
    };
    let commands: Vec<Contract> = vec![
        create_domain.into(),
        create_account.into(),
        create_asset.into(),
    ];
    let transaction = RequestedTransaction::new(commands, Id::new("account", "domain"))
        .accept()
        .expect("Failed to accept transaction.");
    let (public_key, private_key) =
        crypto::generate_key_pair().expect("Failed to generate key pair.");
    let block = PendingBlock::new(vec![transaction])
        .chain_first()
        .sign(&public_key, &private_key)
        .expect("Failed to sign a block.");
    let mut world_state_view = WorldStateView::new(Peer::new("127.0.0.1".to_string(), &Vec::new()));
    let mut success_count = 0;
    let mut failures_count = 0;
    criterion.bench_function("validate_block", |b| {
        b.iter(|| match block.clone().validate(&mut world_state_view) {
            Ok(_) => success_count += 1,
            Err(_) => failures_count += 1,
        });
    });
    println!(
        "Success count: {}, Failures count: {}",
        success_count, failures_count
    );
}

criterion_group!(
    transactions,
    accept_transaction,
    sign_transaction,
    validate_transaction
);
criterion_group!(blocks, chain_blocks, sign_blocks, validate_blocks);
criterion_main!(transactions, blocks);
