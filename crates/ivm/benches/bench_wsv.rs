//! Benchmarks for world state view (WSV) operations.
use std::{convert::TryFrom, sync::Arc};

use criterion::Criterion;
use dashmap::{DashMap, DashSet};
use iroha_crypto::KeyPair;
use iroha_primitives::numeric::Numeric;
use ivm::{
    mock_wsv::{AssetDefinitionId, DomainId, Mintable, Name, ScopedAccountId},
    parallel::{Block, Scheduler, StateAccessSet, Transaction, TxResult},
};

const TRANSFER_ROUTES: [[(usize, usize); 3]; 4] = [
    [(0, 1), (1, 3), (3, 7)],
    [(0, 1), (1, 4), (4, 8)],
    [(0, 2), (2, 5), (5, 9)],
    [(0, 2), (2, 6), (6, 9)],
];

#[derive(Clone)]
struct AssetDefinition {
    mintable: Mintable,
    total_supply: Numeric,
}

impl AssetDefinition {
    fn new(mintable: Mintable) -> Self {
        Self {
            mintable,
            total_supply: Numeric::zero(),
        }
    }
}

#[derive(Default)]
struct ConcurrentWSV {
    domains: DashSet<DomainId>,
    accounts: DashSet<ScopedAccountId>,
    asset_definitions: DashMap<AssetDefinitionId, AssetDefinition>,
    balances: DashMap<(ScopedAccountId, AssetDefinitionId), Numeric>,
}

// DashMap and DashSet use interior mutability through `RwLock`, which prevents
// automatic implementation of `RefUnwindSafe`. The benchmark does not rely on
// unwind safety of these fields, so it is safe to assert this manually.
impl std::panic::RefUnwindSafe for ConcurrentWSV {}

impl ConcurrentWSV {
    fn register_domain(&self, id: DomainId) -> bool {
        self.domains.insert(id)
    }

    fn register_account(&self, id: ScopedAccountId) -> bool {
        if !self.domains.contains(id.domain()) {
            return false;
        }
        self.accounts.insert(id)
    }

    fn register_asset_definition(&self, id: AssetDefinitionId, mintable: Mintable) -> bool {
        if !self.domains.contains(id.domain()) {
            return false;
        }
        self.asset_definitions
            .insert(id, AssetDefinition::new(mintable))
            .is_none()
    }

    fn mint(
        &self,
        account_id: ScopedAccountId,
        asset_id: AssetDefinitionId,
        amount: Numeric,
    ) -> bool {
        if !self.accounts.contains(&account_id) {
            return false;
        }
        if amount.mantissa().is_negative() {
            return false;
        }
        let mut def = match self.asset_definitions.get_mut(&asset_id) {
            Some(def) => def,
            None => return false,
        };
        if def.mintable.consume_one().is_err() {
            return false;
        }
        let total = match def.total_supply.clone().checked_add(amount.clone()) {
            Some(val) => val,
            None => return false,
        };
        def.total_supply = total;
        drop(def);
        let mut bal = self
            .balances
            .entry((account_id, asset_id))
            .or_insert_with(Numeric::zero);
        let next = match bal.clone().checked_add(amount) {
            Some(val) => val,
            None => return false,
        };
        *bal = next;
        true
    }

    fn transfer(
        &self,
        from: ScopedAccountId,
        to: ScopedAccountId,
        asset_id: AssetDefinitionId,
        amount: Numeric,
    ) -> bool {
        if !self.accounts.contains(&from) || !self.accounts.contains(&to) {
            return false;
        }
        if amount.mantissa().is_negative() {
            return false;
        }
        let mut from_bal = self
            .balances
            .entry((from.clone(), asset_id.clone()))
            .or_insert_with(Numeric::zero);
        let remaining = match from_bal.clone().checked_sub(amount.clone()) {
            Some(val) => val,
            None => return false,
        };
        if remaining.mantissa().is_negative() {
            return false;
        }
        *from_bal = remaining;
        drop(from_bal);
        let mut to_bal = self
            .balances
            .entry((to, asset_id))
            .or_insert_with(Numeric::zero);
        let next = match to_bal.clone().checked_add(amount) {
            Some(val) => val,
            None => return false,
        };
        *to_bal = next;
        true
    }
}

const TOTAL_ASSETS: u64 = 1_000_000;
const BATCH_SIZE: usize = 1000;

fn bench_massive_wsv(c: &mut Criterion) {
    c.bench_function("create_1m_assets_transfer_multi_account", |b| {
        let cores = num_cpus::get_physical();
        let scheduler = Scheduler::new(cores);
        let domain: Arc<DomainId> = Arc::new("domain".parse().unwrap());
        let precomputed_accounts: Arc<Vec<ScopedAccountId>> = Arc::new(
            (0..10)
                .map(|_| {
                    let kp = KeyPair::random();
                    ScopedAccountId::new(domain.as_ref().clone(), kp.public_key().clone())
                })
                .collect(),
        );

        b.iter(|| {
            let wsv = Arc::new(ConcurrentWSV {
                domains: DashSet::with_capacity(1),
                accounts: DashSet::with_capacity(precomputed_accounts.len()),
                asset_definitions: DashMap::with_capacity(TOTAL_ASSETS as usize),
                balances: DashMap::with_capacity((TOTAL_ASSETS as usize) * 4),
            });
            let domain_id = domain.as_ref().clone();
            wsv.register_domain(domain_id.clone());
            let accounts = Arc::clone(&precomputed_accounts);
            for acc in accounts.iter() {
                wsv.register_account(acc.clone());
            }
            let mut batch = Vec::with_capacity(BATCH_SIZE);
            let flush_batch = |batch: &mut Vec<Transaction>| {
                if batch.is_empty() {
                    return;
                }
                let mut transactions = Vec::with_capacity(batch.len());
                transactions.append(batch);
                let block = Block { transactions };
                let wsv_ref = &wsv;
                let accounts_ref = &accounts;
                let domain_ref = &domain_id;
                scheduler.schedule_block_conflict_free(block, |tx: Transaction| {
                    let idx = u64::from_le_bytes(tx.code[..8].try_into().unwrap());
                    let asset_name =
                        Name::try_from(format!("asset{idx}")).expect("valid asset name");
                    let asset_id = AssetDefinitionId::new(domain_ref.clone(), asset_name);
                    wsv_ref.register_asset_definition(asset_id.clone(), Mintable::Infinitely);
                    assert!(wsv_ref.mint(
                        accounts_ref[0].clone(),
                        asset_id.clone(),
                        Numeric::from(1_u64),
                    ));
                    let route = &TRANSFER_ROUTES[(idx % TRANSFER_ROUTES.len() as u64) as usize];
                    for &(from_idx, to_idx) in route {
                        assert!(wsv_ref.transfer(
                            accounts_ref[from_idx].clone(),
                            accounts_ref[to_idx].clone(),
                            asset_id.clone(),
                            Numeric::from(1_u64),
                        ));
                    }
                    TxResult {
                        success: true,
                        gas_used: 0,
                    }
                });
            };

            for asset in 0..TOTAL_ASSETS {
                let mut access = StateAccessSet::new();
                access.write_keys.insert(format!("asset{asset}"));
                batch.push(Transaction {
                    code: asset.to_le_bytes().to_vec(),
                    gas_limit: 0,
                    access,
                });
                if batch.len() == BATCH_SIZE {
                    flush_batch(&mut batch);
                }
            }
            if !batch.is_empty() {
                flush_batch(&mut batch);
            }
        })
    });
}

/// Entry point for the benchmark binary.
fn main() {
    let mut c = Criterion::default().configure_from_args();
    bench_massive_wsv(&mut c);
    c.final_summary();
}
