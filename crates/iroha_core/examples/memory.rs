#![allow(unused)]

//! This code can be used to measure memory usage of
//! structs like `Account`, `Asset` and `Nft`.

use iroha_core::prelude::*;
use iroha_data_model::prelude::*;
use util::*;

const N: usize = 1 << 20;

fn main() {
    // measure_accounts_in_vec();
    measure_accounts_in_world();
    // measure_assets_in_world();
    // measure_nfts_in_world();
}

fn measure_accounts_in_vec() {
    let (genesis_domain, _genesis_account) = genesis_domain_and_account();

    let v = (0..N)
        .map(|_| gen_account_in(&genesis_domain.id).0)
        .map(|id| Account::new(id).into_account())
        .collect::<Vec<_>>();
    done(v);
}

fn measure_accounts_in_world() {
    let (genesis_domain, _genesis_account) = genesis_domain_and_account();

    let accounts = (0..N)
        .map(|_| gen_account_in(&genesis_domain.id).0)
        .map(|id| Account::new(id).into_account());
    let world = World::with_assets([], accounts, [], [], []);
    done(world);
}

fn measure_assets_in_world() {
    let (genesis_domain, _genesis_account) = genesis_domain_and_account();
    let asset_definition_id: AssetDefinitionId =
        format!("mandatory#{genesis_domain}").parse().unwrap();

    let assets = (0..N).map(|_| gen_asset(asset_definition_id.clone()));
    let world = World::with_assets([], [], [], assets, []);
    done(world);
}

fn measure_nfts_in_world() {
    let (genesis_domain, _genesis_account) = genesis_domain_and_account();
    let owner = gen_account_in(&genesis_domain.id).0;

    let nfts = (0..N).map(|_| gen_nft(&owner));
    let world = World::with_assets([], [], [], [], nfts);
    done(world);
}

fn print_world_memory_usage() {
    macro_rules! gen {
        ($($p:ident,)+) => {
            $(
                println!(
                    "{}Id:  {} bytes\n{}:   {} bytes",
                    stringify!($p),
                    size_of::<<$p as Identifiable>::Id>(),
                    stringify!($p),
                    size_of::<$p>()
                );
            )+
        };
    }
    gen!(Domain, Account, AssetDefinition, Asset, Nft, Role,);
}

mod util {
    use std::fmt::Display;

    use iroha_core::smartcontracts::Registrable;
    use iroha_crypto::KeyPair;
    use iroha_data_model::prelude::*;

    pub fn genesis_domain_and_account() -> (Domain, Account) {
        let genesis_public_key: PublicKey =
            "ed012003415E0E516BE83870CE5A2165605E8719216B5ECCCE4AEDFB0B2B77862B3798"
                .parse()
                .unwrap();
        let genesis_account_id =
            AccountId::new(iroha_genesis::GENESIS_DOMAIN_ID.clone(), genesis_public_key);
        let genesis_account = Account::new(genesis_account_id.clone()).build(&genesis_account_id);
        let genesis_domain =
            Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone()).build(&genesis_account.id);
        (genesis_domain, genesis_account)
    }

    pub fn gen_account_in(domain: impl Display) -> (AccountId, KeyPair) {
        let key_pair = KeyPair::random();
        let account_id = format!("{}@{}", key_pair.public_key(), domain)
            .parse()
            .unwrap();
        (account_id, key_pair)
    }

    pub fn gen_asset(asset_definition: AssetDefinitionId) -> Asset {
        let account_id = gen_account_in(&asset_definition.domain).0;
        let asset_id = AssetId::new(asset_definition, account_id);
        let value: u64 = rand::random();
        Asset::new(asset_id, value)
    }

    pub fn gen_nft(owner: &AccountId) -> Nft {
        let value: u64 = rand::random();
        let nft_id = format!("n{}${}", value, &owner.domain).parse().unwrap();
        Nft::new(nft_id, Metadata::default()).build(owner)
    }

    pub fn done<T>(value: T) {
        eprintln!("Done");
        std::thread::sleep(std::time::Duration::from_secs(86400));
        std::hint::black_box(value);
    }
}
