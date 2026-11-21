//! Complete future-generation Unload relation (offline to online).
//!
//! Unload spends current proven spendable value: `balance - burned_total`,
//! with the verified predecessor's lineage-adjusted `burned_total`. It debits
//! exactly the face amount, advances `sequence` and `next_redeem`, derives the
//! domain-separated nullifier from scheme, wallet and redemption ordinal, and
//! inserts the redeem entry into the shared load/redeem recovery map. The
//! successor materializes the lineage `burned_total` and pending-outgoing root.
//!
//! The relation binds the online charge and the charge-quote digest; Native and
//! Core authenticate the exact signed quote, the credential-bound payout
//! account and the quote-bound beneficiary. Core pays only after verifying the
//! completed recursive proof of this relation (proof before payout). The type
//! `CompleteUnload` means the relation was constructed, not accepted.
//!
//! Credited evidence, timeouts and rejected envelopes have no input here: no
//! constraint restores offline value, reclaims an earlier Send or retargets a
//! committed payment. Unload spends only the current proven balance.
// Selected by the recursive Unload program in the F1 integration step
// (full.rs kinds and programs, engine and native witness owners).
#![cfg_attr(not(test), allow(dead_code))]

use ff::{Field, PrimeField};
use iroha_pasta::{Fp, poseidon};

use crate::{
    bootstrap::Receipt,
    bootstrap_context as context,
    certificate::{self, Cells},
    credential::Credential,
    fs::SpikeRng,
    full::{VerifiedPredecessor, Wiring},
    indexed_map::{self, InsertWitness},
    monetary_state::{self, StateCells},
    operation_auth::{self, CurrentCredential},
    receipt::{self, AuthenticatedReceipt, PaymentDigest},
    send_arithmetic::Source,
    statement::{self, Kind, StatementCells},
    uniform::{constant, own, plus, scaled},
};

pub const NULLIFIER_DOMAIN: u64 = u64::from_le_bytes(*b"kgwnull1");
pub const REDEEM_DOMAIN: u64 = u64::from_le_bytes(*b"kgwrdm_1");
/// Recovery-map kind of a redemption; keys are `kind * 2^128 + ordinal`
/// (Load 1, Redeem 2) in the one shared load/redeem recovery map.
pub const REDEEM_KIND: u64 = 2;
/// Core30 indices an Unload changes: balance, burned_total, sequence,
/// next_redeem, pending-outgoing root, load/redeem recovery root and nonce.
/// Every other Core field and every Rest field is preserved.
pub const CHANGED: [usize; 7] = [9, 10, 11, 14, 18, 19, 29];

#[derive(Clone)]
pub struct Witness {
    pub before_core: [Fp; 30],
    pub before_rest: [Fp; 6],
    pub after_core: [Fp; 30],
    pub after_rest: [Fp; 6],
    pub credential: Credential,
    pub statement: [Fp; 26],
    pub receipt: Receipt,
    pub recovery: InsertWitness<32>,
}

pub struct UnloadOperation<R> {
    before: StateCells,
    after: StateCells,
    statement: StatementCells,
    credential: CurrentCredential,
    receipt: R,
    lineage: [Source; context::LINEAGE_FIELDS],
    context: [Source; context::FIELDS],
    table: [Source; 4],
}
pub type CompleteUnload = UnloadOperation<AuthenticatedReceipt>;
pub type PreparedUnload = UnloadOperation<receipt::PreparedBody>;
impl<R> UnloadOperation<R> {
    pub fn before(&self) -> &StateCells {
        &self.before
    }
    pub fn after(&self) -> &StateCells {
        &self.after
    }
    pub fn statement(&self) -> &StatementCells {
        &self.statement
    }
    pub fn credential(&self) -> &CurrentCredential {
        &self.credential
    }
    pub fn receipt(&self) -> &R {
        &self.receipt
    }
    pub fn head(&self) -> Source {
        source(self.after.local, self.after.commitment)
    }
    pub fn lineage(&self) -> [Source; context::LINEAGE_FIELDS] {
        self.lineage
    }
    pub fn context(&self) -> [Source; context::FIELDS] {
        self.context
    }
    pub fn table(&self) -> [Source; 4] {
        self.table
    }
    fn with_receipt<T>(self, receipt: T) -> UnloadOperation<T> {
        UnloadOperation {
            before: self.before,
            after: self.after,
            statement: self.statement,
            credential: self.credential,
            receipt,
            lineage: self.lineage,
            context: self.context,
            table: self.table,
        }
    }
}
impl PreparedUnload {
    pub fn body(&self) -> &receipt::PreparedBody {
        &self.receipt
    }
}

fn source(array: usize, cell: usize) -> Source {
    Source {
        array,
        cell,
        instance: 0,
    }
}
fn two128() -> Fp {
    Fp::from_u128(1u128 << 127).double()
}
fn range128(c: &mut Cells, cell: usize, value: Fp) {
    let repr = value.to_repr();
    let bytes: Vec<_> = repr[..16].iter().map(|b| c.byte(*b)).collect();
    c.eq(own(cell, 1), c.bytes(&bytes));
}

/// `P(kgwnull1, [scheme_id (2), wallet_id (2), redeem_ordinal])`.
pub fn nullifier(scheme: [Fp; 2], wallet: [Fp; 2], ordinal: Fp) -> Fp {
    poseidon::hash_with_domain(
        NULLIFIER_DOMAIN,
        &[scheme[0], scheme[1], wallet[0], wallet[1], ordinal],
    )
}
/// Shared recovery-map key `2 * 2^128 + redeem_ordinal`.
pub fn recovery_key(ordinal: Fp) -> Fp {
    Fp::from(REDEEM_KIND) * two128() + ordinal
}
/// Redeem value `P(kgwrdm_1, [ordinal, nullifier, amount, online_charge])`.
pub fn recovery_value(ordinal: Fp, nullifier: Fp, amount: Fp, charge: Fp) -> Fp {
    poseidon::hash_with_domain(REDEEM_DOMAIN, &[ordinal, nullifier, amount, charge])
}

pub fn append(
    w: &mut Wiring<Fp>,
    family: usize,
    input: &Witness,
    predecessor: &VerifiedPredecessor,
) -> Result<CompleteUnload, &'static str> {
    let operation = append_relation(w, family, input, predecessor)?;
    let receipt = receipt::append_own(
        w,
        family,
        &input.receipt,
        &operation.credential,
        &operation.statement,
        Kind::Unload,
        PaymentDigest::Absent,
    )?;
    Ok(operation.with_receipt(receipt))
}

/// Exact same monetary/map/authentication relation before hardware Advance.
/// Native must also verify and retain any signed charge quote before Advance.
pub fn append_preparation(
    w: &mut Wiring<Fp>,
    family: usize,
    input: &Witness,
    predecessor: &VerifiedPredecessor,
) -> Result<PreparedUnload, &'static str> {
    let operation = append_relation(w, family, input, predecessor)?;
    let receipt = receipt::append_preparation(
        w,
        family,
        &input.receipt.body,
        &operation.credential,
        &operation.statement,
        Kind::Unload,
        PaymentDigest::Absent,
    )?;
    Ok(operation.with_receipt(receipt))
}

fn append_relation(
    w: &mut Wiring<Fp>,
    family: usize,
    input: &Witness,
    predecessor: &VerifiedPredecessor,
) -> Result<UnloadOperation<()>, &'static str> {
    let before = monetary_state::append_state(w, family, &input.before_core, &input.before_rest);
    let after = monetary_state::append_state(w, family, &input.after_core, &input.after_rest);
    let credential =
        operation_auth::append_current(w, family, &input.credential, &before, predecessor)?;
    // Statement owner: version, identities, lifecycle 1/2, u128 sequence and
    // next_load, mask bits, u128 lineage burn, nonzero lineage pending root,
    // tag 6, nonzero nullifier, u128 ordinal/amount/charge, amount > 0,
    // charge <= amount, quote zero iff charge zero, fields 22..25 zero.
    let statement = statement::append_statement(w, family, Kind::Unload, &input.statement);
    let entry = append_branch(w, family, [&before, &after], &statement, predecessor, None);
    let insertion = indexed_map::append_insert(w, family, &input.recovery);
    crate::load::join_recovery(w, family, &insertion, entry, None);
    let lineage = crate::load::lineage(&after, predecessor);
    Ok(UnloadOperation {
        before,
        after,
        statement,
        credential,
        receipt: (),
        lineage,
        context: predecessor.context(),
        table: predecessor.table(),
    })
}

pub(crate) fn append_branch(
    w: &mut Wiring<Fp>,
    family: usize,
    [before, after]: [&StateCells; 2],
    statement: &StatementCells,
    predecessor: &VerifiedPredecessor,
    enable: Option<Source>,
) -> crate::load::RecoveryEntry {
    operation_auth::ensure_range(w, family);
    let prior = predecessor.lineage();
    let bf = |i| source(before.local, before.core[i]);
    let af = |i| source(after.local, after.core[i]);
    let sf = |i| source(statement.local, statement.fields[i]);
    let ordinal = bf(14);
    let amount = sf(19);
    let charge = sf(20);
    let burned = prior[context::BURNED_TOTAL];
    let pending = prior[context::PENDING_ROOT];
    let mut c = Cells::new(family);
    c.a.name = "complete Unload monetary statement and authority joins";
    let join = |c: &mut Cells, a: crate::uniform::Lin<Fp>, b: crate::uniform::Lin<Fp>| {
        if let Some(mode) = enable {
            c.a.rows
                .push([mode.linear(), plus(&a, &scaled(&b, -Fp::ONE)), vec![]]);
        } else {
            c.eq(a, b);
        }
    };
    for (a, b) in [
        (source(before.local, before.commitment), predecessor.head()),
        (bf(0), prior[context::LIFECYCLE]),
        (bf(27), prior[context::POLICY_EPOCH]),
        (bf(22), prior[context::ENABLED_CONTROLS]),
        (sf(1), predecessor.context()[context::RELATION]),
        (sf(2), predecessor.context()[context::RELATION + 1]),
        (sf(3), af(1)),
        (sf(4), af(2)),
        (sf(5), af(3)),
        (sf(6), af(4)),
        (sf(7), af(7)),
        (sf(8), af(0)),
        (sf(9), af(11)),
        (sf(10), af(13)),
        (sf(11), bf(22)),
        (sf(12), burned),
        (sf(13), pending),
        (sf(14), source(before.local, before.commitment)),
        (sf(15), source(after.local, after.commitment)),
        (sf(18), ordinal),
        // Private Core burn/pending values can be stale after lineage
        // adjustments; the successor takes the verified lineage values.
        (af(10), burned),
        (af(18), pending),
    ] {
        join(&mut c, a.linear(), b.linear());
    }
    // Spendable bound as checked integers: C[9] = a + burned + r with a, the
    // lineage burn and r each u128 (sum < 3 * 2^128 < p), so no wraparound.
    let remaining_value = enable.map_or(Fp::ONE, |mode| mode.value(w))
        * (bf(9).value(w) - amount.value(w) - burned.value(w));
    let remaining = c.cell(remaining_value, 255);
    range128(&mut c, remaining, remaining_value);
    join(
        &mut c,
        bf(9).linear(),
        plus(
            &plus(&amount.linear(), &burned.linear()),
            &own(remaining, 1),
        ),
    );
    // Debit exactly the face amount. The online charge is withheld from the
    // online payout; it is never debited a second time offline.
    join(
        &mut c,
        plus(&af(9).linear(), &amount.linear()),
        bf(9).linear(),
    );
    for i in [11, 14] {
        join(
            &mut c,
            af(i).linear(),
            plus(&bf(i).linear(), &constant(vec![Fp::ONE])),
        );
    }
    for i in 0..30 {
        if !CHANGED.contains(&i) {
            join(&mut c, af(i).linear(), bf(i).linear());
        }
    }
    for i in 0..6 {
        join(
            &mut c,
            source(before.local, before.rest[i]).linear(),
            source(after.local, after.rest[i]).linear(),
        );
    }
    let nullifier_sources = [bf(1), bf(2), bf(5), bf(6), ordinal];
    let nullifier_value =
        poseidon::hash_with_domain(NULLIFIER_DOMAIN, &nullifier_sources.map(|s| s.value(w)));
    let nullifier_cells = [
        c.cell(Fp::ZERO, 255),
        c.cell(nullifier_value, 255),
        c.cell(Fp::ZERO, 255),
    ];
    join(&mut c, sf(17).linear(), own(nullifier_cells[1], 1));
    let key_offset = Fp::from(REDEEM_KIND) * two128();
    let key = c.cell(key_offset + ordinal.value(w), 255);
    c.eq(
        own(key, 1),
        plus(&constant(vec![key_offset]), &ordinal.linear()),
    );
    let value = recovery_value(
        ordinal.value(w),
        nullifier_value,
        amount.value(w),
        charge.value(w),
    );
    let value_cells = [
        c.cell(Fp::ZERO, 255),
        c.cell(value, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let local = w.push(c.a);
    let nullifier_items: Vec<_> = nullifier_sources
        .iter()
        .map(|s| (s.value(w), s.linear()))
        .collect();
    certificate::hash_relation(
        w,
        family,
        local,
        NULLIFIER_DOMAIN,
        &nullifier_items,
        nullifier_cells,
        &mut SpikeRng::new(b"Unload nullifier"),
    );
    let value_items = vec![
        (ordinal.value(w), ordinal.linear()),
        (nullifier_value, own(nullifier_cells[1], 1)),
        (amount.value(w), amount.linear()),
        (charge.value(w), charge.linear()),
    ];
    certificate::hash_relation(
        w,
        family,
        local,
        REDEEM_DOMAIN,
        &value_items,
        value_cells,
        &mut SpikeRng::new(b"Unload redemption recovery entry"),
    );
    // Depth-32 absence under C[19], low-leaf relink, empty slot, exact C'[19].
    crate::load::RecoveryEntry {
        old_root: bf(19),
        key: source(local, key),
        value: source(local, value_cells[1]),
        new_root: af(19),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        bootstrap,
        full::{self, WalletKey},
        indexed_map::{Leaf, Path},
        ipa::Gens,
        operation_auth::tests as harness,
        spartan,
    };
    use iroha_pasta::Eq as Vesta;
    use num_bigint::BigUint;
    use std::{collections::BTreeMap, sync::OnceLock};

    const BALANCE: u128 = 2000;
    /// Private Core burn, stale against the lineage-adjusted Omega(pred) burn.
    const CORE_BURN: u128 = 10;
    const LINEAGE_BURN: u128 = 30;
    const SPENDABLE: u128 = BALANCE - LINEAGE_BURN;
    const ORDINAL: u128 = 3;
    const QUOTE: u64 = 0x5155_4f54;
    const OPID: u64 = u64::from_le_bytes(*b"kgwopid1");

    #[derive(Clone, Copy)]
    struct Terms {
        amount: u128,
        charge: u128,
        retiring: bool,
    }
    const PARTIAL: Terms = Terms {
        amount: 500,
        charge: 0,
        retiring: false,
    };
    const CHARGED: Terms = Terms {
        amount: 500,
        charge: 7,
        retiring: false,
    };
    const FULL_RETIRING: Terms = Terms {
        amount: SPENDABLE,
        charge: 25,
        retiring: true,
    };
    fn terms(amount: u128, charge: u128) -> Terms {
        Terms {
            amount,
            charge,
            retiring: false,
        }
    }

    fn wallet() -> WalletKey {
        WalletKey::labelled("complete Unload fixture")
    }
    fn base() -> &'static (bootstrap::Context, bootstrap::Inputs) {
        static BASE: OnceLock<(bootstrap::Context, bootstrap::Inputs)> = OnceLock::new();
        BASE.get_or_init(|| bootstrap::fixture(&wallet(), &Gens::<Vesta>::new(1 << 16)))
    }
    /// Genuine charged Unload with adjusted burn/pending lineage and a signed receipt.
    pub(crate) fn control_fixture() -> (Witness, [Fp; 64], WalletKey) {
        let fixture = make(setup(terms(500, 7)));
        (fixture.input, fixture.profile, fixture.key)
    }

    /// Spend four offline units from the receiver after Refresh, with a one-unit
    /// online charge. This chain starts with an authenticated empty recovery map.
    pub(crate) fn recursive_unload(
        control: &crate::control::Witness,
        profile: [Fp; 64],
        key: &WalletKey,
    ) -> Witness {
        recursive_unload_from(control, profile, key, None, Fp::from(QUOTE))
    }

    pub(crate) fn recursive_unload_from(
        control: &crate::control::Witness,
        profile: [Fp; 64],
        key: &WalletKey,
        first_load: Option<&crate::load::Witness>,
        quote: Fp,
    ) -> Witness {
        let mut map = Map::new();
        if let Some(load) = first_load {
            map.insert(load.recovery.key, load.recovery.value);
        }
        recursive_unload_with_map(
            (
                &control.after_core,
                &control.after_rest,
                &control.credential,
                &control.statement,
                &control.receipt,
            ),
            profile,
            key,
            &map,
            1,
            quote,
        )
    }

    pub(crate) fn recursive_unload_with_map(
        state: (&[Fp; 30], &[Fp; 6], &Credential, &[Fp; 26], &Receipt),
        profile: [Fp; 64],
        key: &WalletKey,
        map: &Map,
        charge: u128,
        quote: Fp,
    ) -> Witness {
        let (core, rest, credential, statement, receipt) = state;
        assert_eq!(map.root(), core[19]);
        let mut f = Fixture {
            input: Witness {
                before_core: *core,
                before_rest: *rest,
                after_core: *core,
                after_rest: *rest,
                credential: credential.clone(),
                statement: *statement,
                receipt: receipt.clone(),
                recovery: crate::load::dummy_recovery(),
            },
            profile,
            key: key.clone(),
            map: map.clone(),
            terms: terms(4, charge),
        };
        complete(&mut f);
        f.input.statement[21] = quote;
        reseal(&mut f);
        f.input
    }

    fn integer(x: Fp) -> BigUint {
        BigUint::from_bytes_le(x.to_repr().as_ref())
    }

    /// Native depth-32 indexed map in canonical slot order (sentinel at 0).
    #[derive(Clone)]
    pub(crate) struct Map {
        nodes: BTreeMap<(usize, u32), Fp>,
        empty: Vec<Fp>,
        leaves: Vec<Leaf>,
    }
    impl Map {
        pub(crate) fn new() -> Self {
            let mut empty = vec![Fp::ZERO];
            for h in 0..32 {
                empty.push(poseidon::hash_with_domain(
                    indexed_map::NODE_DOMAIN,
                    &[empty[h]; 2],
                ));
            }
            let mut map = Self {
                nodes: BTreeMap::new(),
                empty,
                leaves: vec![],
            };
            map.put(
                0,
                Leaf {
                    key: Fp::ZERO,
                    value: Fp::ZERO,
                    next: Fp::ZERO,
                },
            );
            map
        }
        fn get(&self, height: usize, index: u32) -> Fp {
            *self
                .nodes
                .get(&(height, index))
                .unwrap_or(&self.empty[height])
        }
        fn put(&mut self, index: u32, leaf: Leaf) {
            if index as usize == self.leaves.len() {
                self.leaves.push(leaf);
            } else {
                self.leaves[index as usize] = leaf;
            }
            let mut index = index;
            self.nodes.insert((0, index), indexed_map::leaf_hash(leaf));
            for height in 0..32 {
                let pair = [self.get(height, index & !1), self.get(height, index | 1)];
                index >>= 1;
                self.nodes.insert(
                    (height + 1, index),
                    poseidon::hash_with_domain(indexed_map::NODE_DOMAIN, &pair),
                );
            }
        }
        fn path(&self, index: u32) -> Path<32> {
            Path {
                index,
                siblings: std::array::from_fn(|h| self.get(h, (index >> h) ^ 1)),
            }
        }
        pub(crate) fn root(&self) -> Fp {
            self.get(32, 0)
        }
        /// A native store's insertion witness: greatest smaller key as the low
        /// leaf, next free slot as destination. For a key already present this
        /// is the best available forgery, which the relation must refuse.
        pub(crate) fn insert(&mut self, key: Fp, value: Fp) -> InsertWitness<32> {
            let old_root = self.root();
            let low_index = (0..self.leaves.len())
                .filter(|&i| integer(self.leaves[i].key) < integer(key))
                .max_by_key(|&i| integer(self.leaves[i].key))
                .unwrap();
            let low = self.leaves[low_index];
            let low_path = self.path(low_index as u32);
            self.put(low_index as u32, Leaf { next: key, ..low });
            let slot_index = self.leaves.len() as u32;
            let slot = self.path(slot_index);
            self.put(
                slot_index,
                Leaf {
                    key,
                    value,
                    next: low.next,
                },
            );
            InsertWitness {
                old_root,
                key,
                value,
                low,
                low_path,
                slot,
            }
        }
    }

    /// Predecessor inputs before deriving the Unload: Core, the recovery map
    /// behind C[19] and the verified lineage values Omega(pred) carries.
    struct Setup {
        terms: Terms,
        before: [Fp; 30],
        map: Map,
        lineage_burn: Fp,
        lineage_pending: Fp,
    }
    struct Fixture {
        input: Witness,
        profile: [Fp; 64],
        key: WalletKey,
        map: Map,
        terms: Terms,
    }

    /// A funded wallet after one Load, three earlier redemptions, Sends and a
    /// fold whose lineage raised the burn and archived a pending entry. The
    /// genuinely signed credential and own receipts come from the Bootstrap
    /// fixture; the funded predecessor port stands in for its verified proof.
    fn setup(terms: Terms) -> Setup {
        let (_, boot) = base();
        let mut before = boot.core;
        before[0] = Fp::from(if terms.retiring { 2 } else { 1 });
        before[9] = Fp::from_u128(BALANCE);
        before[10] = Fp::from_u128(CORE_BURN);
        before[11] = Fp::from(8);
        before[12] = Fp::from(7);
        before[13] = Fp::ONE;
        before[14] = Fp::from_u128(ORDINAL);
        before[18] = indexed_map::empty_insert::<32>(Fp::from(0x77), Fp::from(0x78)).1;
        before[29] = Fp::from(4242);
        let mut map = Map::new();
        map.insert(two128(), Fp::from(0x4c));
        for j in 0..ORDINAL as u64 {
            map.insert(recovery_key(Fp::from(j)), Fp::from(0x52 + j));
        }
        Setup {
            terms,
            before,
            map,
            lineage_burn: Fp::from_u128(LINEAGE_BURN),
            lineage_pending: boot.core[18],
        }
    }
    fn make(s: Setup) -> Fixture {
        let (ctx, boot) = base();
        let mut before = s.before;
        before[19] = s.map.root();
        let mut profile = bootstrap::public_profile(
            ctx,
            &bootstrap::Inputs {
                core: before,
                ..boot.clone()
            },
        );
        profile[context::LINEAGE_IO + context::BURNED_TOTAL] = s.lineage_burn;
        profile[context::LINEAGE_IO + context::PENDING_ROOT] = s.lineage_pending;
        let mut f = Fixture {
            input: Witness {
                before_core: before,
                before_rest: boot.rest,
                after_core: before,
                after_rest: boot.rest,
                credential: boot.credential.clone(),
                statement: boot.statement,
                receipt: boot.receipt.clone(),
                recovery: indexed_map::empty_insert(Fp::ONE, Fp::ONE).0,
            },
            profile,
            key: wallet(),
            map: s.map,
            terms: s.terms,
        };
        complete(&mut f);
        f
    }
    fn complete(f: &mut Fixture) {
        let before = f.input.before_core;
        let j = before[14];
        let (a, c) = (Fp::from_u128(f.terms.amount), Fp::from_u128(f.terms.charge));
        let burn = f.profile[context::LINEAGE_IO + context::BURNED_TOTAL];
        let pending = f.profile[context::LINEAGE_IO + context::PENDING_ROOT];
        let n = nullifier([before[1], before[2]], [before[5], before[6]], j);
        let mut after = before;
        after[9] = before[9] - a;
        after[10] = burn;
        after[11] += Fp::ONE;
        after[14] += Fp::ONE;
        after[18] = pending;
        after[29] = Fp::from(5151);
        let st = &mut f.input.statement;
        st[8] = after[0];
        st[9] = after[11];
        st[10] = after[13];
        st[11] = before[22];
        st[12] = burn;
        st[13] = pending;
        st[14] = monetary_state::commitment(&before, &f.input.before_rest);
        st[16] = Fp::from(6);
        st[17] = n;
        st[18] = j;
        st[19] = a;
        st[20] = c;
        st[21] = if f.terms.charge == 0 {
            Fp::ZERO
        } else {
            Fp::from(QUOTE)
        };
        st[22..].fill(Fp::ZERO);
        f.input.after_core = after;
        reinsert(f, recovery_key(j), recovery_value(j, n, a, c));
    }
    /// Recompute the insertion from the predecessor map, then reseal.
    fn reinsert(f: &mut Fixture, key: Fp, value: Fp) {
        let mut map = f.map.clone();
        f.input.recovery = map.insert(key, value);
        f.input.after_core[19] = map.root();
        reseal(f);
    }
    /// Successor commitment from the current successor state and a fresh
    /// genuine own receipt: a mutation surviving this is a relation hole.
    fn reseal(f: &mut Fixture) {
        f.input.statement[15] =
            monetary_state::commitment(&f.input.after_core, &f.input.after_rest);
        sign(&mut f.input, &f.key, 6);
    }
    fn sign(input: &mut Witness, key: &WalletKey, tag: u64) {
        let operation = poseidon::hash_with_domain(
            OPID,
            &[
                input.before_core[5],
                input.before_core[6],
                Fp::from(tag),
                input.statement[17],
            ],
        );
        input.receipt.body[98..114].copy_from_slice(&input.statement[9].to_repr()[..16]);
        for (at, value) in [
            (114, operation),
            (146, input.statement[14]),
            (178, input.statement[15]),
            (210, statement::digest(&input.statement)),
        ] {
            input.receipt.body[at..at + 32].copy_from_slice(value.to_repr().as_ref());
        }
        input.receipt.signature = bootstrap::sign(
            bootstrap::receipt_message(&input.receipt),
            &key.d,
            "complete Unload receipt fixture",
        );
    }

    fn try_build(f: &Fixture) -> Result<harness::Built, &'static str> {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let predecessor = harness::predecessor(&mut w, family, f.profile);
        let completed = append(&mut w, family, &f.input, &predecessor)?;
        harness::expose(
            &mut w,
            family,
            4,
            &[
                completed.head(),
                source(completed.statement().local, completed.statement().digest),
                completed.receipt().digest(),
            ],
        );
        harness::expose(&mut w, family, 16, &completed.context());
        harness::expose(&mut w, family, 32, &completed.lineage());
        Ok(harness::finish(w, 16))
    }
    fn build(f: &Fixture) -> harness::Built {
        try_build(f).unwrap()
    }
    /// A construction error (no ECDSA witness under the continuing key) is a
    /// refusal too: no satisfying assignment exists for that input.
    fn holds(f: &Fixture) -> bool {
        try_build(f).is_ok_and(|b| harness::holds(&b))
    }
    fn entries(b: &harness::Built) -> Vec<(usize, usize, usize, Fp)> {
        b.0.entries(&[Fp::from(31), Fp::from(47)])
    }
    fn refuse_all(cases: Vec<(&str, Fixture)>) {
        for (name, f) in cases {
            assert!(!holds(&f), "{name}");
        }
    }

    #[test]
    fn unload_nullifier_and_redeem_entry_match_native_data_model() {
        use iroha_data_model::kagemusha::{
            KagemushaWalletRedeemLeafV1, kagemusha_wallet_unload_nullifier_v1,
        };
        let (_, boot) = base();
        let scheme: [u8; 32] = boot.credential.body[2..34].try_into().unwrap();
        let wallet_id: [u8; 32] = boot.credential.body[66..98].try_into().unwrap();
        let core = boot.core;
        for ordinal in [0, ORDINAL, u128::MAX] {
            let j = Fp::from_u128(ordinal);
            let n = nullifier([core[1], core[2]], [core[5], core[6]], j);
            let n_bytes: [u8; 32] = n.to_repr().as_ref().try_into().unwrap();
            assert_eq!(
                n_bytes,
                kagemusha_wallet_unload_nullifier_v1(&scheme, &wallet_id, ordinal)
            );
            let leaf = KagemushaWalletRedeemLeafV1 {
                ordinal,
                nullifier: n_bytes,
                amount: SPENDABLE,
                online_charge: 25,
            };
            assert_eq!(recovery_key(j).to_repr().as_ref(), &leaf.key());
            assert_eq!(
                recovery_value(j, n, Fp::from_u128(SPENDABLE), Fp::from(25))
                    .to_repr()
                    .as_ref(),
                &leaf.leaf_value().unwrap()
            );
        }
        assert_eq!(
            Map::new().root(),
            indexed_map::empty_insert::<32>(Fp::ONE, Fp::ONE).0.old_root
        );
        assert_eq!(Map::new().root(), boot.core[19], "canonical empty map");
    }

    #[test]
    fn complete_unload_partial_charged_full_and_retiring_share_one_relation() {
        let mut first = setup(PARTIAL);
        first.map = Map::new();
        first.before[14] = Fp::ZERO;
        let fixtures = [
            make(setup(PARTIAL)),
            make(setup(CHARGED)),
            make(setup(FULL_RETIRING)),
            make(setup(terms(SPENDABLE, 0))),
            make(first),
        ];
        let built: Vec<_> = fixtures.iter().map(build).collect();
        let matrix = entries(&built[0]);
        for (i, candidate) in built.iter().enumerate() {
            assert!(harness::holds(candidate), "valid Unload {i}");
            assert_eq!(entries(candidate), matrix, "one Unload program {i}");
        }
        let shape = &built[0].0.shape;
        let gens = Gens::<Vesta>::new(shape.cols());
        for candidate in [&built[1], &built[2]] {
            let (_, instance, witness) = candidate;
            let (proof, _) = spartan::prove_circuit(
                shape,
                &gens,
                instance,
                witness,
                &mut SpikeRng::new(b"complete Unload proof"),
                &built[0].0,
            );
            let bytes = proof.to_bytes();
            assert!(spartan::verify_circuit(shape, &gens, instance, &bytes, &built[0].0).is_ok());
            for at in [
                4,
                5,
                6,
                24,
                32,
                33,
                34,
                35,
                36,
                37,
                56,
                128 + 4,
                128 + 24,
                128 + 35,
                128 + 36,
            ] {
                let mut changed = instance.clone();
                changed.io[at] += Fp::ONE;
                assert!(
                    spartan::verify_circuit(shape, &gens, &changed, &bytes, &built[0].0).is_err(),
                    "public port {at}"
                );
            }
        }
    }

    #[test]
    fn unload_amount_charge_and_quote_terms_fail_closed() {
        let mut cases = vec![
            ("zero amount", make(setup(terms(0, 0)))),
            ("charge above amount", make(setup(terms(5, 6)))),
        ];
        let mut f = make(setup(CHARGED));
        f.input.statement[21] = Fp::ZERO;
        reseal(&mut f);
        cases.push(("charge without quote", f));
        let mut f = make(setup(PARTIAL));
        f.input.statement[21] = Fp::from(QUOTE);
        reseal(&mut f);
        cases.push(("quote without charge", f));
        let mut f = make(setup(CHARGED));
        f.input.after_core[9] -= Fp::from(7);
        reseal(&mut f);
        cases.push(("charge debited offline on top of the face amount", f));
        let mut f = make(setup(CHARGED));
        f.input.after_core[9] += Fp::ONE;
        reseal(&mut f);
        cases.push(("debit below the face amount", f));
        let mut f = make(setup(PARTIAL));
        let (n, j) = (f.input.statement[17], f.input.statement[18]);
        f.input.statement[19] += Fp::ONE;
        let claimed = f.input.statement[19];
        reinsert(
            &mut f,
            recovery_key(j),
            recovery_value(j, n, claimed, Fp::ZERO),
        );
        cases.push(("claimed amount above the debit", f));
        let mut f = make(setup(CHARGED));
        reinsert(
            &mut f,
            recovery_key(j),
            recovery_value(j, n, Fp::from(500), Fp::ZERO),
        );
        cases.push(("recovery entry drops the charge", f));
        let mut f = make(setup(CHARGED));
        reinsert(
            &mut f,
            recovery_key(j),
            recovery_value(j, n, Fp::from(501), Fp::from(7)),
        );
        cases.push(("recovery entry amount", f));
        refuse_all(cases);
    }

    #[test]
    fn unload_spends_only_lineage_adjusted_balance_and_cannot_reclaim_value() {
        let mut cases = vec![
            ("one above spendable", make(setup(terms(SPENDABLE + 1, 0)))),
            (
                "stale Core burn as affordability",
                make(setup(terms(BALANCE - CORE_BURN, 0))),
            ),
            (
                "burned credit value redeemed",
                make(setup(terms(BALANCE, 0))),
            ),
            ("balance overdraw", make(setup(terms(BALANCE + 1, 0)))),
        ];
        let mut f = make(setup(PARTIAL));
        f.input.after_core[10] = f.input.before_core[10];
        reseal(&mut f);
        cases.push(("stale Core burn kept in the successor", f));
        let mut f = make(setup(PARTIAL));
        f.input.after_core[10] = Fp::ZERO;
        reseal(&mut f);
        cases.push(("burn cleared to make burned value spendable", f));
        let mut f = make(setup(PARTIAL));
        f.input.after_core[18] = f.input.before_core[18];
        reseal(&mut f);
        cases.push(("archived pending entry resurrected", f));
        let mut f = make(setup(PARTIAL));
        f.input.statement[12] = f.input.before_core[10];
        reseal(&mut f);
        cases.push(("statement burn from Core instead of lineage", f));
        let mut f = make(setup(PARTIAL));
        f.input.statement[13] = f.input.before_core[18];
        reseal(&mut f);
        cases.push(("statement pending root from Core instead of lineage", f));
        let mut f = make(setup(PARTIAL));
        f.input.after_core[9] += Fp::from(100);
        reseal(&mut f);
        cases.push(("earlier Send value reclaimed into the balance", f));
        refuse_all(cases);
    }

    #[test]
    fn unload_nullifier_ordinal_and_recovery_map_bindings_fail_closed() {
        let reference = make(setup(CHARGED));
        let core = reference.input.before_core;
        let (scheme, wallet_id) = ([core[1], core[2]], [core[5], core[6]]);
        let (j, a, c) = (core[14], Fp::from(500), Fp::from(7));
        let mut cases = vec![];
        for (name, n) in [
            (
                "nullifier of the next ordinal",
                nullifier(scheme, wallet_id, j + Fp::ONE),
            ),
            (
                "foreign wallet nullifier",
                nullifier(scheme, [wallet_id[0] + Fp::ONE, wallet_id[1]], j),
            ),
            (
                "foreign scheme nullifier",
                nullifier([scheme[0], scheme[1] + Fp::ONE], wallet_id, j),
            ),
        ] {
            let mut f = make(setup(CHARGED));
            f.input.statement[17] = n;
            reinsert(&mut f, recovery_key(j), recovery_value(j, n, a, c));
            cases.push((name, f));
        }
        let mut f = make(setup(CHARGED));
        let skipped = j + Fp::ONE;
        let n = nullifier(scheme, wallet_id, skipped);
        f.input.statement[17] = n;
        f.input.statement[18] = skipped;
        f.input.after_core[14] = skipped + Fp::ONE;
        reinsert(
            &mut f,
            recovery_key(skipped),
            recovery_value(skipped, n, a, c),
        );
        cases.push(("ordinal other than next_redeem", f));
        let mut f = make(setup(CHARGED));
        f.input.after_core[14] = core[14];
        reseal(&mut f);
        cases.push(("next_redeem not advanced", f));
        let mut f = make(setup(CHARGED));
        let n = f.input.statement[17];
        reinsert(&mut f, two128() + j, recovery_value(j, n, a, c));
        cases.push(("Load-kind recovery key", f));
        let mut used = setup(CHARGED);
        used.map.insert(recovery_key(j), Fp::from(0x99));
        cases.push(("redemption identity already recorded", make(used)));
        let mut f = make(setup(CHARGED));
        f.input.recovery.slot = f.input.recovery.low_path.clone();
        cases.push(("occupied destination slot", f));
        let mut f = make(setup(CHARGED));
        f.input.recovery.low_path.siblings[3] += Fp::ONE;
        cases.push(("altered low-leaf path", f));
        let mut f = make(setup(CHARGED));
        f.input.recovery.old_root += Fp::ONE;
        cases.push(("insertion under another root", f));
        let mut f = make(setup(CHARGED));
        f.input.after_core[19] = core[19];
        reseal(&mut f);
        cases.push(("recovery entry omitted", f));
        let mut replay = make(setup(CHARGED));
        replay.input.before_core = replay.input.after_core;
        replay.profile[full::HEAD_IO] = replay.input.statement[15];
        replay.input.statement[14] = replay.input.statement[15];
        sign(&mut replay.input, &replay.key, 6);
        cases.push(("same Unload replayed from its own successor", replay));
        refuse_all(cases);
    }

    #[test]
    fn unload_preserves_every_other_state_field_even_when_resigned() {
        for index in 0..29 {
            let mut f = make(setup(CHARGED));
            let after = &mut f.input.after_core;
            after[index] = if index == 0 {
                Fp::from(2)
            } else {
                after[index] + Fp::ONE
            };
            let changed = after[index];
            // Keep each statement mirror consistent: only the transition refuses.
            match index {
                0 => f.input.statement[8] = changed,
                1..=4 => f.input.statement[2 + index] = changed,
                7 => f.input.statement[7] = changed,
                11 => f.input.statement[9] = changed,
                13 => f.input.statement[10] = changed,
                _ => {}
            }
            reseal(&mut f);
            assert!(!holds(&f), "signed bad successor core {index}");
        }
        for index in 0..6 {
            let mut f = make(setup(CHARGED));
            f.input.after_rest[index] += Fp::ONE;
            reseal(&mut f);
            assert!(!holds(&f), "signed bad successor rest {index}");
        }
        // The successor nonce is native entropy: any canonical nonzero value.
        let mut f = make(setup(CHARGED));
        f.input.after_core[29] = Fp::from(99_991);
        reseal(&mut f);
        assert!(holds(&f), "fresh nonzero nonce");
        f.input.after_core[29] = Fp::ZERO;
        reseal(&mut f);
        assert!(!holds(&f), "zero nonce");
    }

    #[test]
    fn unload_requires_exact_predecessor_credential_and_own_receipt() {
        let mut cases = vec![];
        for (name, at) in [
            ("predecessor head", full::HEAD_IO),
            ("relation", context::CONTEXT_IO + context::RELATION),
            ("scheme", context::CONTEXT_IO + context::SCHEME),
            ("asset", context::CONTEXT_IO + context::ASSET),
            ("wallet", context::CONTEXT_IO + context::WALLET),
            (
                "credential digest",
                context::CONTEXT_IO + context::CREDENTIAL,
            ),
            ("payment key", context::CONTEXT_IO + context::PAYMENT_KEY),
            ("certified table", full::TABLE_IO),
            (
                "predecessor lifecycle",
                context::LINEAGE_IO + context::LIFECYCLE,
            ),
            (
                "predecessor policy epoch",
                context::LINEAGE_IO + context::POLICY_EPOCH,
            ),
            (
                "predecessor enabled controls",
                context::LINEAGE_IO + context::ENABLED_CONTROLS,
            ),
            (
                "lineage burned_total",
                context::LINEAGE_IO + context::BURNED_TOTAL,
            ),
            (
                "lineage pending root",
                context::LINEAGE_IO + context::PENDING_ROOT,
            ),
        ] {
            let mut f = make(setup(CHARGED));
            f.profile[at] += Fp::ONE;
            cases.push((name, f));
        }
        let mut f = make(setup(CHARGED));
        f.input.credential.body[98] ^= 1;
        cases.push(("credential account digest substituted", f));
        let mut f = make(setup(CHARGED));
        f.input.statement[21] += Fp::ONE;
        cases.push(("quote digest changed after signing", f));
        let mut f = make(setup(CHARGED));
        f.input.statement[19] -= Fp::ONE;
        cases.push(("amount changed after signing", f));
        let mut f = make(setup(CHARGED));
        sign(
            &mut f.input,
            &WalletKey::labelled("another Unload wallet"),
            6,
        );
        cases.push(("receipt under a foreign key", f));
        let mut f = make(setup(CHARGED));
        sign(&mut f.input, &f.key, 2);
        cases.push(("receipt for a Load operation id", f));
        let mut f = make(setup(CHARGED));
        f.input.receipt.body[306] = 1;
        f.input.receipt.signature = bootstrap::sign(
            bootstrap::receipt_message(&f.input.receipt),
            &f.key.d,
            "Unload receipt with a Payment digest",
        );
        cases.push(("receipt carrying a Payment digest", f));
        refuse_all(cases);
    }

    #[test]
    fn unload_counter_overflow_fails_closed() {
        let mut last = setup(PARTIAL);
        last.before[14] = Fp::from_u128(u128::MAX - 1);
        assert!(holds(&make(last)), "last representable next_redeem");
        let mut sequence = setup(PARTIAL);
        sequence.before[11] = Fp::from_u128(u128::MAX);
        let mut ordinal = setup(PARTIAL);
        ordinal.before[14] = Fp::from_u128(u128::MAX);
        refuse_all(vec![
            ("sequence overflow", make(sequence)),
            ("next_redeem overflow", make(ordinal)),
        ]);
    }

    fn build_prepared(f: &Fixture) -> harness::Built {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let predecessor = harness::predecessor(&mut w, family, f.profile);
        let prepared = append_preparation(&mut w, family, &f.input, &predecessor).unwrap();
        harness::expose(
            &mut w,
            family,
            4,
            &[
                prepared.head(),
                source(prepared.statement.local, prepared.statement.digest),
                prepared.body().operation_id(),
            ],
        );
        harness::expose(&mut w, family, 32, &prepared.lineage());
        harness::finish(w, 16)
    }

    #[test]
    fn prepared_unload_applies_the_same_relation_before_advance() {
        let a = build_prepared(&make(setup(CHARGED)));
        let b = build_prepared(&make(setup(FULL_RETIRING)));
        assert!(harness::holds(&a));
        assert!(harness::holds(&b));
        assert_eq!(entries(&a), entries(&b));
        for (name, f) in [
            ("overdraw", make(setup(terms(SPENDABLE + 1, 0)))),
            (
                "stale Core burn",
                make(setup(terms(BALANCE - CORE_BURN, 0))),
            ),
        ] {
            assert!(!harness::holds(&build_prepared(&f)), "prepared {name}");
        }
    }

    /// Row census of the complete relation (run with --nocapture).
    #[test]
    fn unload_relation_census() {
        let f = make(setup(CHARGED));
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let predecessor = harness::predecessor(&mut w, family, f.profile);
        let start = w.arrays.len();
        let completed = append(&mut w, family, &f.input, &predecessor).unwrap();
        // The operation ports carry the exact authenticated values.
        let value = |s: Source| s.value(&w);
        let st = &f.input.statement;
        assert_eq!(
            value(source(
                completed.before().local,
                completed.before().commitment
            )),
            st[14]
        );
        assert_eq!(value(completed.head()), st[15]);
        assert_eq!(
            value(source(completed.after().local, completed.after().core[14])),
            st[18] + Fp::ONE
        );
        assert_eq!(value(completed.credential().digest()), st[7]);
        let table = completed.table().map(value);
        assert_eq!(table, f.profile[full::TABLE_IO..full::TABLE_IO + 4]);
        // Sponge arrays belong to the component whose host array precedes
        // them; instances are r_count times the family's instance count.
        const SPONGE: [&str; 6] = [
            "certificate sponge initial state",
            "input",
            "full round",
            "partial round 4",
            "7 partial rounds",
            "mask",
        ];
        let component = |name: &str| match name {
            "canonical monetary Core30 and Rest6" => "two Core30/Rest6 state openings",
            "operation authentication range"
            | "certificate bytes and integers"
            | "certificate issuer square"
            | "certificate issuer curve"
            | "current credential identity ports" => "current credential opening and continuity",
            "canonical26 field statement" => "Unload statement (tag 6) and digest",
            "complete Unload monetary statement and authority joins" => {
                "Unload joins, arithmetic, nullifier, redeem value"
            }
            "exact indexed-map insertion" | "complete Unload recovery joins" => {
                "depth-32 recovery-map insertion and joins"
            }
            _ => "own receipt (SHA-256, comb ECDSA, wallet table)",
        };
        let mut groups: Vec<(&str, [usize; 5])> = vec![];
        let mut current = "";
        for a in &w.arrays[start..] {
            if !SPONGE.contains(&a.name) {
                current = component(a.name);
            }
            let at = match groups.iter().position(|(n, _)| *n == current) {
                Some(at) => at,
                None => {
                    groups.push((current, [0; 5]));
                    groups.len() - 1
                }
            };
            let instances = a.r_count * w.counts[a.family];
            let g = &mut groups[at].1;
            g[0] += a.rows.len() * instances;
            g[1] += a.lookups.len() * instances;
            g[2] += a.cells() * instances;
            if a.name == "input" {
                g[3] += 1;
                g[4] += w.counts[a.family];
            }
        }
        println!("unload census: rows lookups cells sponges permutations  component");
        let mut total = [0; 5];
        for (name, g) in &groups {
            println!(
                "unload census: {:6} {:6} {:6} {:4} {:5}  {name}",
                g[0], g[1], g[2], g[3], g[4]
            );
            for i in 0..5 {
                total[i] += g[i];
            }
        }
        println!(
            "unload census: {:6} {:6} {:6} {:4} {:5}  total",
            total[0], total[1], total[2], total[3], total[4]
        );
        let built = build(&f);
        let shape = &built.0.shape;
        println!(
            "unload census: shape sx {} row_bits {} col_bits {} cols {}",
            shape.sx,
            shape.row_bits,
            shape.col_bits,
            shape.cols()
        );
        assert!(harness::holds(&built));
    }
}
