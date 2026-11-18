//! One fixed Control program for Load, RefreshTimeAnchor and Unload.
//!
//! Two Boolean mode bits in public IO slots 62 and 63 select a single
//! semantic operation. Their product is zero. All three modes share state
//! openings, credential continuity, a signed receipt and a depth-32 recovery
//! insertion. The selected monetary mode binds both roots, key and value.
//! Refresh preserves the recovery root. Its signature check uses the fixed
//! public dummy tuple in the two monetary modes.
//!
//! Load finality before Advance and Unload verification before online payout
//! are separate native and ledger requirements.
// Selected by the recursive Control program in the F2 integration step
// (full.rs kinds and programs, engine and native witness owners).
#![cfg_attr(not(test), allow(dead_code))]

use ff::Field;
use iroha_pasta::Fp;

use crate::{
    bootstrap::Receipt,
    bootstrap_context as context,
    certificate::Cells,
    credential::Credential,
    full::{VerifiedPredecessor, Wiring},
    indexed_map::InsertWitness,
    load::{self, Placement},
    monetary_state::{self, StateCells},
    operation_auth::{self, CurrentCredential},
    p256::Point,
    receipt::{self, AuthenticatedReceipt, Operation, PaymentDigest},
    send_arithmetic::Source,
    statement::{self, StatementCells},
    time_anchor::{self, Authority, RefreshBranch},
    uniform::{constant, plus, scaled},
};

/// The public IO offset of the Control mode pair (Unload, RefreshTimeAnchor).
pub const MODE_IO: usize = 62;
/// Fields preserved by Load that either Refresh or Unload may update.
const OTHER_MODE_CORE: [usize; 4] = [10, 14, 18, 28];
const REFRESH_ONLY_REST: [usize; 1] = [5];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Load,
    RefreshTimeAnchor,
    Unload,
}
impl Mode {
    /// Public input order on both fields of the recursive pair.
    pub fn bits(self) -> [u64; 2] {
        [
            u64::from(self == Self::Unload),
            u64::from(self == Self::RefreshTimeAnchor),
        ]
    }

    /// Decode the complete ordinary-IO mode under the selected matrix kind.
    /// Non-Control circuits admit the zero mode only.
    pub fn for_program(kind: usize, bits: [Fp; 2]) -> Result<Option<Self>, &'static str> {
        let mode = match bits {
            [u, r] if u == Fp::ZERO && r == Fp::ZERO => Self::Load,
            [u, r] if u == Fp::ZERO && r == Fp::ONE => Self::RefreshTimeAnchor,
            [u, r] if u == Fp::ONE && r == Fp::ZERO => Self::Unload,
            _ => return Err("invalid Control mode"),
        };
        match kind {
            crate::full::CONTROL => Ok(Some(mode)),
            crate::full::BOOTSTRAP | crate::full::SEND | crate::full::RECEIVE
                if mode == Self::Load =>
            {
                Ok(None)
            }
            _ => Err("invalid program kind or nonzero mode outside Control"),
        }
    }

    /// Wire statement tag for this semantic operation.
    pub fn tag(self) -> u64 {
        match self {
            Self::Load => 2,
            Self::RefreshTimeAnchor => time_anchor::REFRESH_TAG,
            Self::Unload => 6,
        }
    }
}

/// The two authenticated mode inputs, in public-input order (io62, io63).
#[derive(Clone, Copy)]
pub struct Modes {
    /// Unload selector in public slot 62.
    pub unload: Source,
    /// RefreshTimeAnchor selector in public slot 63.
    pub refresh: Source,
}
impl Modes {
    /// Authenticated semantic tag as a linear form in the two selectors.
    pub fn tag(self) -> crate::uniform::Lin<Fp> {
        plus(
            &plus(
                &constant(vec![Fp::from(2)]),
                &scaled(&self.refresh.linear(), Fp::from(5)),
            ),
            &scaled(&self.unload.linear(), Fp::from(4)),
        )
    }

    fn load(self) -> crate::uniform::Lin<Fp> {
        plus(
            &constant(vec![Fp::ONE]),
            &scaled(
                &plus(&self.refresh.linear(), &self.unload.linear()),
                -Fp::ONE,
            ),
        )
    }

    /// Mode bits in canonical public-input order.
    pub fn inputs(self) -> [Source; 2] {
        [self.unload, self.refresh]
    }
}

#[derive(Clone)]
pub struct Witness {
    pub mode: Mode,
    pub before_core: [Fp; 30],
    pub before_rest: [Fp; 6],
    pub after_core: [Fp; 30],
    pub after_rest: [Fp; 6],
    pub credential: Credential,
    pub statement: [Fp; 26],
    pub receipt: Receipt,
    /// Load branch originals; the canonical dummies in Refresh mode.
    pub ordinary: [u8; load::ORDINARY_BYTES],
    pub recovery: InsertWitness<32>,
    /// Refresh branch originals; `Authority::dummy()` in Load mode.
    pub authority: Authority,
}
impl Drop for Witness {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.before_core.zeroize();
        self.before_rest.zeroize();
        self.after_core.zeroize();
        self.after_rest.zeroize();
        self.statement.zeroize();
        self.ordinary.zeroize();
        self.recovery.zeroize();
    }
}

impl Witness {
    /// A Load: exactly the Load witness, with the inactive dummy authority.
    pub fn load(input: load::Witness) -> Self {
        Self {
            mode: Mode::Load,
            before_core: input.before_core,
            before_rest: input.before_rest,
            after_core: input.after_core,
            after_rest: input.after_rest,
            credential: input.credential,
            statement: input.statement,
            receipt: input.receipt,
            ordinary: input.ordinary,
            recovery: input.recovery,
            authority: Authority::dummy(),
        }
    }
    /// Unload uses its signed monetary originals and the common insertion.
    pub fn unload(input: crate::unload::Witness) -> Self {
        Self {
            mode: Mode::Unload,
            before_core: input.before_core,
            before_rest: input.before_rest,
            after_core: input.after_core,
            after_rest: input.after_rest,
            credential: input.credential,
            statement: input.statement,
            receipt: input.receipt,
            ordinary: load::dummy_ordinary(),
            recovery: input.recovery,
            authority: Authority::dummy(),
        }
    }

    /// A RefreshTimeAnchor, with the inactive dummy Load originals.
    #[allow(clippy::too_many_arguments)]
    pub fn refresh(
        before_core: [Fp; 30],
        before_rest: [Fp; 6],
        after_core: [Fp; 30],
        after_rest: [Fp; 6],
        credential: Credential,
        statement: [Fp; 26],
        receipt: Receipt,
        authority: Authority,
    ) -> Self {
        Self {
            mode: Mode::RefreshTimeAnchor,
            before_core,
            before_rest,
            after_core,
            after_rest,
            credential,
            statement,
            receipt,
            ordinary: load::dummy_ordinary(),
            recovery: load::dummy_recovery(),
            authority,
        }
    }
}

pub struct ControlOperation<R> {
    mode: Modes,
    before: StateCells,
    after: StateCells,
    statement: StatementCells,
    credential: CurrentCredential,
    receipt: R,
    refresh: RefreshBranch,
    lineage: [Source; context::LINEAGE_FIELDS],
    context: [Source; context::FIELDS],
    table: [Source; 4],
}
pub type CompleteControl = ControlOperation<AuthenticatedReceipt>;
pub type PreparedControl = ControlOperation<receipt::PreparedBody>;
impl<R> ControlOperation<R> {
    /// The constrained pair of mode bits in io62 and io63.
    pub fn mode(&self) -> Modes {
        self.mode
    }
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
    pub fn refresh(&self) -> &RefreshBranch {
        &self.refresh
    }
    pub fn head(&self) -> Source {
        Source {
            array: self.after.local,
            cell: self.after.commitment,
            instance: 0,
        }
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
}
impl PreparedControl {
    pub fn body(&self) -> &receipt::PreparedBody {
        &self.receipt
    }
}

/// The complete Control relation with the hardware-signed own receipt.
/// `root` is the installed scheme root, a circuit constant of this program.
pub fn append(
    w: &mut Wiring<Fp>,
    family: usize,
    root: &Point,
    input: &Witness,
    predecessor: &VerifiedPredecessor,
) -> Result<CompleteControl, &'static str> {
    append_with(
        w,
        family,
        root,
        input,
        predecessor,
        |w, credential, statement, operation| {
            receipt::append_own_operation(
                w,
                family,
                &input.receipt,
                credential,
                statement,
                operation,
                PaymentDigest::Absent,
            )
        },
    )
}

/// The same relation before hardware Advance (unsigned receipt body).
pub fn append_preparation(
    w: &mut Wiring<Fp>,
    family: usize,
    root: &Point,
    input: &Witness,
    predecessor: &VerifiedPredecessor,
) -> Result<PreparedControl, &'static str> {
    append_with(
        w,
        family,
        root,
        input,
        predecessor,
        |w, credential, statement, operation| {
            receipt::append_preparation_operation(
                w,
                family,
                &input.receipt.body,
                credential,
                statement,
                operation,
                PaymentDigest::Absent,
            )
        },
    )
}

fn append_with<R>(
    w: &mut Wiring<Fp>,
    family: usize,
    root: &Point,
    input: &Witness,
    predecessor: &VerifiedPredecessor,
    own: impl FnOnce(
        &mut Wiring<Fp>,
        &CurrentCredential,
        &StatementCells,
        Operation,
    ) -> Result<R, &'static str>,
) -> Result<ControlOperation<R>, &'static str> {
    let mut c = Cells::new(family);
    c.a.name = "Control mode selector";
    let u = c.bit(input.mode == Mode::Unload);
    let r = c.bit(input.mode == Mode::RefreshTimeAnchor);
    c.a.rows
        .push([crate::uniform::own(u, 1), crate::uniform::own(r, 1), vec![]]);
    let local = w.push(c.a);
    let port = |cell| Source {
        array: local,
        cell,
        instance: 0,
    };
    let mode = Modes {
        unload: port(u),
        refresh: port(r),
    };
    let before = monetary_state::append_state(w, family, &input.before_core, &input.before_rest);
    let after = monetary_state::append_state(w, family, &input.after_core, &input.after_rest);
    let credential =
        operation_auth::append_current(w, family, &input.credential, &before, predecessor)?;
    let statement = statement::append_control_statement(w, family, mode, &input.statement);
    let load_entry = load::append_branch(
        w,
        family,
        &input.ordinary,
        [&before, &after],
        &statement,
        predecessor,
        &Placement::Branch {
            enable: mode.load(),
            core: &OTHER_MODE_CORE,
            rest: &REFRESH_ONLY_REST,
        },
    );
    let unload_entry = crate::unload::append_branch(
        w,
        family,
        [&before, &after],
        &statement,
        predecessor,
        Some(mode.unload),
    );
    let insertion = crate::indexed_map::append_insert(w, family, &input.recovery);
    load::join_recovery(w, family, &insertion, load_entry, Some(mode.load()));
    load::join_recovery(
        w,
        family,
        &insertion,
        unload_entry,
        Some(mode.unload.linear()),
    );
    // Root-certificate authentication reuses the generator table installed by the receipt.
    let receipt = own(w, &credential, &statement, Operation::Control { mode })?;
    let refresh = time_anchor::append_branch(
        w,
        family,
        mode.refresh,
        &input.authority,
        root,
        [&before, &after],
        &statement,
        &credential,
    )?;
    let lineage = load::lineage(&after, predecessor);
    Ok(ControlOperation {
        mode,
        before,
        after,
        statement,
        credential,
        receipt,
        refresh,
        lineage,
        context: predecessor.context(),
        table: predecessor.table(),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{
        bootstrap,
        certificate::{self, Certificate, Role, field_modulus},
        fs::SpikeRng,
        full::{self, WalletKey},
        indexed_map,
        ipa::Gens,
        operation_auth::tests as harness,
        p256, spartan,
        time_anchor::TimeAnchor,
    };
    use ff::PrimeField;
    use iroha_pasta::{Eq as Vesta, poseidon};
    use num_bigint::BigUint;
    use std::sync::OnceLock;

    const OPID: u64 = u64::from_le_bytes(*b"kgwopid1");
    /// The funded predecessor's accepted-time floor (Unix ms).
    const FLOOR: u64 = 1_763_400_000_000;
    /// A held anchor digest stand-in in the funded predecessor's Rest6[5].
    const HELD: u64 = 0x6865_6c64;
    const SIGNER: &str = "Control TimeAnchor signer";

    fn wallet() -> WalletKey {
        WalletKey::labelled("Control fixture")
    }
    fn base() -> &'static (bootstrap::Context, bootstrap::Inputs) {
        static BASE: OnceLock<(bootstrap::Context, bootstrap::Inputs)> = OnceLock::new();
        BASE.get_or_init(|| bootstrap::fixture(&wallet(), &Gens::<Vesta>::new(1 << 16)))
    }
    fn root_secret() -> BigUint {
        BigUint::from_bytes_be(&[0x11; 32])
    }
    fn root() -> Point {
        base().0.root.root.clone()
    }
    fn scheme() -> [u8; 32] {
        base().1.credential.body[2..34].try_into().unwrap()
    }
    fn wallet_id() -> [u8; 32] {
        base().1.credential.body[66..98].try_into().unwrap()
    }
    fn signer(label: &str) -> (BigUint, Point) {
        let d = p256::scalar_below(label, &p256::n());
        let q = p256::mul(&p256::generator(), &d).unwrap();
        (d, q)
    }
    fn issue(role: u8, key: &Point, serial: u64, root: &BigUint) -> Certificate {
        certificate::issue_fixture(&scheme(), role, key, serial, root)
    }
    fn sign_anchor(a: &mut Authority, d: &BigUint) {
        a.anchor.signature =
            bootstrap::sign(time_anchor::message(&a.anchor), d, "time anchor fixture");
    }
    /// Point the anchor at its certificate and re-sign it.
    fn rebind(a: &mut Authority, d: &BigUint) {
        let digest = certificate::certificate_digest(&a.certificate);
        a.anchor.body[106..138].copy_from_slice(digest.to_repr().as_ref());
        sign_anchor(a, d);
    }
    /// A genuine root-certified TimeAnchor answer for this wallet at `time`.
    fn authority(time: u64, label: &str) -> (Authority, BigUint) {
        let (d, q) = signer(label);
        let certificate = issue(Role::TimeAnchor as u8, &q, 9, &root_secret());
        let mut body = [0; time_anchor::BODY_BYTES];
        body[..2].copy_from_slice(&1u16.to_le_bytes());
        body[2..34].copy_from_slice(&scheme());
        body[34..66].copy_from_slice(&wallet_id());
        body[66..98].fill(0x4e);
        body[98..106].copy_from_slice(&time.to_le_bytes());
        let mut a = Authority {
            certificate,
            anchor: TimeAnchor {
                body,
                signature: [0; 64],
            },
        };
        rebind(&mut a, &d);
        (a, d)
    }
    /// A funded predecessor that already holds an anchor and a floor.
    fn funded() -> ([Fp; 30], [Fp; 6]) {
        let (_, boot) = base();
        let mut core = boot.core;
        core[9] = Fp::from(2000);
        core[10] = Fp::from(10);
        core[11] = Fp::from(8);
        core[12] = Fp::from(7);
        core[13] = Fp::from(3);
        core[14] = Fp::from(2);
        core[28] = Fp::from(FLOOR);
        core[29] = Fp::from(4242);
        let mut rest = boot.rest;
        rest[5] = Fp::from(HELD);
        (core, rest)
    }

    struct Fixture {
        input: Witness,
        profile: [Fp; 64],
        key: WalletKey,
    }
    fn profile(core: &[Fp; 30], rest: &[Fp; 6]) -> [Fp; 64] {
        let (ctx, boot) = base();
        bootstrap::public_profile(
            ctx,
            &bootstrap::Inputs {
                core: *core,
                rest: *rest,
                ..boot.clone()
            },
        )
    }
    /// The successor an attacker would claim from these exact originals,
    /// without native authentication: the relation alone must refuse it.
    fn refresh_from(before: [Fp; 30], rest: [Fp; 6], authority: Authority) -> Fixture {
        let (_, boot) = base();
        let profile = profile(&before, &rest);
        let d = time_anchor::digest(&authority.anchor);
        let time = time_anchor::issuer_time(&authority.anchor);
        let mut after = before;
        after[11] += Fp::ONE;
        after[28] = Fp::from(time.max(u64::from_le_bytes(
            before[28].to_repr()[..8].try_into().unwrap(),
        )));
        after[29] = Fp::from(5151);
        let mut after_rest = rest;
        after_rest[5] = d;
        let relation = [
            profile[context::CONTEXT_IO + context::RELATION],
            profile[context::CONTEXT_IO + context::RELATION + 1],
        ];
        let statement =
            time_anchor::statement(relation, (&before, &rest), (&after, &after_rest), d);
        let mut f = Fixture {
            input: Witness::refresh(
                before,
                rest,
                after,
                after_rest,
                boot.credential.clone(),
                statement,
                boot.receipt.clone(),
                authority,
            ),
            profile,
            key: wallet(),
        };
        reseal(&mut f);
        f
    }
    /// A real signed Refresh extending the supplied chain wallet's state.
    pub(crate) fn recursive_refresh(
        before: &[Fp; 30],
        rest: &[Fp; 6],
        profile: &[Fp],
        key: &WalletKey,
        boot: &bootstrap::Inputs,
    ) -> Witness {
        let (mut authority, signer) = authority(FLOOR + 1, SIGNER);
        let scheme = boot.credential.body[2..34].try_into().unwrap();
        let wallet: [u8; 32] = boot.credential.body[66..98].try_into().unwrap();
        authority.anchor.body[34..66].copy_from_slice(&wallet);
        sign_anchor(&mut authority, &signer);
        let verified = time_anchor::authenticate(&authority, &scheme, &wallet, &root()).unwrap();
        let (after, after_rest) =
            time_anchor::refresh(before, rest, &verified, before[29] + Fp::ONE).unwrap();
        let offset = context::CONTEXT_IO + context::RELATION;
        let statement = time_anchor::statement(
            profile[offset..offset + 2].try_into().unwrap(),
            (before, rest),
            (&after, &after_rest),
            verified.digest,
        );
        let mut witness = Witness::refresh(
            *before,
            *rest,
            after,
            after_rest,
            boot.credential.clone(),
            statement,
            boot.receipt.clone(),
            authority,
        );
        sign(&mut witness, key);
        witness
    }

    fn refresh(time: u64) -> Fixture {
        let (before, rest) = funded();
        refresh_from(before, rest, authority(time, SIGNER).0)
    }
    /// The Load fixture of `load.rs`, under this file's wallet.
    fn load_witness(charge: u128) -> (load::Witness, [Fp; 64]) {
        let (ctx, boot) = base();
        let profile = bootstrap::public_profile(ctx, boot);
        let mut ordinary = [0; load::ORDINARY_BYTES];
        ordinary[0] = 1;
        ordinary[2..98].copy_from_slice(&boot.credential.body[2..98]);
        ordinary[98..130].fill(0x45);
        ordinary[146..162].copy_from_slice(&2000u128.to_le_bytes());
        ordinary[162..178].copy_from_slice(&charge.to_le_bytes());
        ordinary[178] = u8::from(charge != 0);
        ordinary[210..242].fill(0x56);
        ordinary[242] = 2;
        ordinary[250..282].copy_from_slice(&boot.credential.body[98..130]);
        let digest = load::ordinary_digest(&ordinary);
        let value =
            poseidon::hash_with_domain(load::RECOVERY_DOMAIN, &[Fp::ZERO, digest, Fp::from(2000)]);
        let (recovery, root) =
            indexed_map::empty_insert(Fp::from_u128(1u128 << 127).double(), value);
        let mut after = boot.core;
        after[9] = Fp::from(2000);
        after[11] = Fp::ONE;
        after[13] = Fp::ONE;
        after[19] = root;
        after[29] += Fp::ONE;
        let mut st = boot.statement;
        st[9] = Fp::ONE;
        st[10] = Fp::ONE;
        st[14] = profile[full::HEAD_IO];
        st[15] = monetary_state::commitment(&after, &boot.rest);
        st[16] = Fp::from(2);
        st[17] = digest;
        st[18] = Fp::ZERO;
        st[19] = Fp::from(2000);
        st[20] = Fp::from_u128(charge);
        st[21..].fill(Fp::ZERO);
        let mut input = Witness::load(load::Witness {
            before_core: boot.core,
            before_rest: boot.rest,
            after_core: after,
            after_rest: boot.rest,
            credential: boot.credential.clone(),
            ordinary,
            statement: st,
            receipt: boot.receipt.clone(),
            recovery,
        });
        sign(&mut input, &wallet());
        (as_load(&input), profile)
    }
    fn as_load(input: &Witness) -> load::Witness {
        load::Witness {
            before_core: input.before_core,
            before_rest: input.before_rest,
            after_core: input.after_core,
            after_rest: input.after_rest,
            credential: input.credential.clone(),
            ordinary: input.ordinary,
            statement: input.statement,
            receipt: input.receipt.clone(),
            recovery: input.recovery.clone(),
        }
    }
    fn load_fixture(charge: u128) -> Fixture {
        let (input, profile) = load_witness(charge);
        Fixture {
            input: Witness::load(input),
            profile,
            key: wallet(),
        }
    }
    /// Successor commitment from the current successor state and a fresh
    /// genuine own receipt: a mutation surviving this is a relation hole.
    fn reseal(f: &mut Fixture) {
        f.input.statement[15] =
            monetary_state::commitment(&f.input.after_core, &f.input.after_rest);
        sign(&mut f.input, &f.key);
    }
    fn sign(input: &mut Witness, key: &WalletKey) {
        let st = input.statement;
        let operand = if input.mode != Mode::RefreshTimeAnchor {
            st[17]
        } else {
            st[18]
        };
        sign_with(input, key, st[16], operand);
    }
    fn sign_with(input: &mut Witness, key: &WalletKey, tag: Fp, operand: Fp) {
        let operation = poseidon::hash_with_domain(
            OPID,
            &[input.before_core[5], input.before_core[6], tag, operand],
        );
        let st = input.statement;
        input.receipt.body[98..114].copy_from_slice(&st[9].to_repr()[..16]);
        for (at, value) in [
            (114, operation),
            (146, st[14]),
            (178, st[15]),
            (210, statement::digest(&st)),
        ] {
            input.receipt.body[at..at + 32].copy_from_slice(value.to_repr().as_ref());
        }
        input.receipt.signature = bootstrap::sign(
            bootstrap::receipt_message(&input.receipt),
            &key.d,
            "Control receipt fixture",
        );
    }

    fn try_build(f: &Fixture) -> Result<harness::Built, &'static str> {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let predecessor = harness::predecessor(&mut w, family, f.profile);
        let completed = append(&mut w, family, &root(), &f.input, &predecessor)?;
        harness::expose(
            &mut w,
            family,
            4,
            &[
                completed.head(),
                Source {
                    array: completed.statement().local,
                    cell: completed.statement().digest,
                    instance: 0,
                },
                completed.receipt().digest(),
            ],
        );
        harness::expose(&mut w, family, 16, &completed.context());
        harness::expose(&mut w, family, 32, &completed.lineage());
        harness::expose(&mut w, family, MODE_IO, &completed.mode().inputs());
        Ok(harness::finish(w, 16))
    }
    fn build(f: &Fixture) -> harness::Built {
        try_build(f).unwrap()
    }
    /// A construction error (no ECDSA witness for the own receipt) is a
    /// refusal too: no satisfying assignment exists for that input.
    fn holds(f: &Fixture) -> bool {
        try_build(f).is_ok_and(|b| harness::holds(&b))
    }
    fn load_alone(input: &load::Witness, profile: [Fp; 64]) -> harness::Built {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let predecessor = harness::predecessor(&mut w, family, profile);
        let completed = load::append(&mut w, family, input, &predecessor).unwrap();
        harness::expose(
            &mut w,
            family,
            4,
            &[
                completed.head(),
                Source {
                    array: completed.statement().local,
                    cell: completed.statement().digest,
                    instance: 0,
                },
                completed.receipt().digest(),
            ],
        );
        harness::expose(&mut w, family, 16, &completed.context());
        harness::expose(&mut w, family, 32, &completed.lineage());
        harness::finish(w, 16)
    }
    /// The canonical public program: every matrix entry per array and every
    /// fixed column (root and generator tables are circuit constants).
    fn entries(b: &harness::Built) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        for a in 0..b.0.arrays.len() {
            for (matrix, row, col, value) in b.0.entries_of(&[Fp::from(31), Fp::from(47)], Some(a))
            {
                for v in [matrix, row, col] {
                    h.update(&(v as u64).to_le_bytes());
                }
                h.update(value.to_repr().as_ref());
            }
            if b.0.arrays[a].fixed {
                for column in &b.0.arrays[a].values {
                    for value in column {
                        h.update(value.to_repr().as_ref());
                    }
                }
            }
        }
        *h.finalize().as_bytes()
    }
    fn refuse_all(cases: Vec<(&str, Fixture)>) {
        for (name, f) in cases {
            assert!(!holds(&f), "{name}");
        }
    }

    #[test]
    fn control_load_and_refresh_share_one_program() {
        let (boot_core, boot_rest) = (base().1.core, base().1.rest);
        let mut retiring = funded();
        retiring.0[0] = Fp::from(2);
        let fixtures = [
            ("Load", load_fixture(0)),
            ("charged Load", load_fixture(3)),
            ("newer anchor", refresh(FLOOR + 5_000)),
            ("older anchor", refresh(FLOOR - 5_000)),
            ("equal anchor", refresh(FLOOR)),
            ("largest u64 time", refresh(u64::MAX)),
            (
                "first anchor after Bootstrap",
                refresh_from(boot_core, boot_rest, authority(7, SIGNER).0),
            ),
            (
                "another TimeAnchor signer",
                refresh_from(
                    funded().0,
                    funded().1,
                    authority(FLOOR + 1, "second TimeAnchor signer").0,
                ),
            ),
            (
                "Retiring wallet",
                refresh_from(retiring.0, retiring.1, authority(FLOOR + 9, SIGNER).0),
            ),
        ];
        for (name, f) in &fixtures {
            assert_eq!(
                f.input.statement[16],
                Fp::from(f.input.mode.tag()),
                "semantic tag of {name}"
            );
        }
        // One built circuit at a time: each is about 2^20 constraints.
        let matrix = entries(&build(&fixtures[0].1));
        for (name, f) in &fixtures {
            let candidate = build(f);
            assert!(harness::holds(&candidate), "valid {name}");
            assert_eq!(entries(&candidate), matrix, "one Control program: {name}");
        }
        for f in [&fixtures[1].1, &fixtures[2].1] {
            let candidate = build(f);
            let (circuit, instance, witness) = &candidate;
            let shape = &circuit.shape;
            let gens = Gens::<Vesta>::new(shape.cols());
            let (proof, _) = spartan::prove_circuit(
                shape,
                &gens,
                instance,
                witness,
                &mut SpikeRng::new(b"complete Control proof"),
                circuit,
            );
            let bytes = proof.to_bytes();
            assert!(spartan::verify_circuit(shape, &gens, instance, &bytes, circuit).is_ok());
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
                MODE_IO,
                128 + 4,
                128 + 24,
                128 + 33,
            ] {
                let mut changed = instance.clone();
                changed.io[at] += Fp::ONE;
                assert!(
                    spartan::verify_circuit(shape, &gens, &changed, &bytes, circuit).is_err(),
                    "public port {at}"
                );
            }
        }
    }

    #[test]
    fn control_unload_materializes_lineage_and_preserves_unrelated_state() {
        let (input, profile, key) = crate::unload::tests::control_fixture();
        let mut f = Fixture {
            input: Witness::unload(input),
            profile,
            key,
        };
        assert!(holds(&f));
        for field in [9, 10, 14, 18, 19, 28] {
            let expected = f.input.after_core[field];
            f.input.after_core[field] += Fp::ONE;
            reseal(&mut f);
            assert!(!holds(&f), "Unload successor field {field}");
            f.input.after_core[field] = expected;
        }
        f.input.statement[21] = Fp::ZERO;
        reseal(&mut f);
        assert!(!holds(&f), "charged Unload needs the quote digest");
    }

    #[test]
    fn three_control_modes_share_exact_program_bytes() {
        let load = load_fixture(3);
        let reference = build(&load).0.standalone_program_bytes().unwrap();
        let (unload, profile, key) = crate::unload::tests::control_fixture();
        let unload = Fixture {
            input: Witness::unload(unload),
            profile,
            key,
        };
        for f in [refresh(FLOOR + 10), unload] {
            let candidate = build(&f);
            assert!(harness::holds(&candidate));
            let program = candidate.0.standalone_program_bytes().unwrap();
            assert!(
                reference == program,
                "one exact Control program for all three modes"
            );
        }
    }

    #[test]
    fn inactive_anchor_bytes_cannot_select_the_signer() {
        let mut input = load_fixture(0);
        let baseline = entries(&build(&input));
        let mut authority = authority(FLOOR + 5, SIGNER).0;
        authority.anchor.signature.fill(0xff);
        input.input.authority = authority.clone();
        let candidate = build(&input);
        assert!(harness::holds(&candidate));
        assert_eq!(entries(&candidate), baseline);
        drop(candidate);
        let (core, rest) = funded();
        assert!(!holds(&refresh_from(core, rest, authority)));
    }

    #[test]
    fn time_anchor_digests_and_transition_match_native() {
        use iroha_data_model::kagemusha::{
            KagemushaDeviceSignatureV1, KagemushaWalletTimeAnchorBodyV1,
            KagemushaWalletTimeAnchorV1,
        };
        for time in [FLOOR - 1, FLOOR, FLOOR + 1] {
            let (a, _) = authority(time, SIGNER);
            let body = &a.anchor.body;
            let model = KagemushaWalletTimeAnchorV1 {
                body: KagemushaWalletTimeAnchorBodyV1 {
                    version: 1,
                    scheme_id: body[2..34].try_into().unwrap(),
                    wallet_id: body[34..66].try_into().unwrap(),
                    nonce: body[66..98].try_into().unwrap(),
                    issuer_time_ms: time,
                    signer_certificate: body[106..138].try_into().unwrap(),
                },
                signature: KagemushaDeviceSignatureV1::from_raw_bytes(&a.anchor.signature).unwrap(),
            };
            model.validate().unwrap();
            assert_eq!(
                model.body.transcript(),
                body.to_vec(),
                "exact 138-byte body"
            );
            assert_eq!(
                model.body.signing_message(),
                time_anchor::message(&a.anchor).to_repr(),
                "signing message"
            );
            assert_eq!(
                model.time_anchor_digest(),
                time_anchor::digest(&a.anchor).to_repr(),
                "object digest"
            );
            let verified = time_anchor::authenticate(&a, &scheme(), &wallet_id(), &root()).unwrap();
            let (core, rest) = funded();
            let (after, after_rest) =
                time_anchor::refresh(&core, &rest, &verified, Fp::from(5151)).unwrap();
            // The relation's ports carry exactly the native values.
            let f = refresh(time);
            assert_eq!(f.input.after_core, after);
            assert_eq!(f.input.after_rest, after_rest);
            let mut w = Wiring::default();
            let family = w.family(0, 1);
            let predecessor = harness::predecessor(&mut w, family, f.profile);
            let completed = append(&mut w, family, &root(), &f.input, &predecessor).unwrap();
            let value = |s: Source| s.value(&w);
            let branch = completed.refresh();
            assert_eq!(value(branch.valid()), Fp::ONE);
            assert_eq!(value(branch.digest()), verified.digest);
            assert_eq!(value(branch.time()), Fp::from(time));
            assert_eq!(value(branch.floor()), Fp::from(time.max(FLOOR)));
            assert_eq!(value(completed.mode().refresh), Fp::ONE);
            assert_eq!(value(completed.head()), f.input.statement[15]);
            assert_eq!(
                value(Source {
                    array: completed.statement().local,
                    cell: completed.statement().digest,
                    instance: 0,
                }),
                statement::digest(&f.input.statement)
            );
            assert_eq!(
                value(completed.receipt().digest()),
                receipt::digest(&f.input.receipt)
            );
        }
        // The held anchor cannot be reinstalled natively either.
        let (a, _) = authority(FLOOR + 1, SIGNER);
        let verified = time_anchor::authenticate(&a, &scheme(), &wallet_id(), &root()).unwrap();
        let (core, mut rest) = funded();
        rest[5] = verified.digest;
        assert!(time_anchor::refresh(&core, &rest, &verified, Fp::from(5151)).is_err());
    }

    /// Each refusal also fails native authentication: same rules both sides.
    #[test]
    fn refresh_authority_refusals_fail_natively_and_in_circuit() {
        let root2 = BigUint::from_bytes_be(&[0x33; 32]);
        let mut cases: Vec<(&str, Authority)> = vec![];
        for (name, role) in [
            ("Enrollment-role certificate", Role::Enrollment as u8),
            (
                "RegulatoryPolicy-role certificate",
                Role::RegulatoryPolicy as u8,
            ),
            ("Artifact-role certificate", Role::Artifact as u8),
            ("invalid role 2", 2),
        ] {
            let (mut a, d) = authority(FLOOR + 1, SIGNER);
            a.certificate = issue(role, &signer(SIGNER).1, 9, &root_secret());
            rebind(&mut a, &d);
            cases.push((name, a));
        }
        let (mut a, d) = authority(FLOOR + 1, SIGNER);
        a.certificate = issue(Role::TimeAnchor as u8, &signer(SIGNER).1, 9, &root2);
        rebind(&mut a, &d);
        cases.push(("certificate under a foreign root", a));
        let (mut a, d) = authority(FLOOR + 1, SIGNER);
        let mut foreign = scheme();
        foreign[0] ^= 1;
        a.certificate = certificate::issue_fixture(
            &foreign,
            Role::TimeAnchor as u8,
            &signer(SIGNER).1,
            9,
            &root_secret(),
        );
        rebind(&mut a, &d);
        cases.push(("certificate for a foreign scheme", a));
        let (mut a, d) = authority(FLOOR + 1, SIGNER);
        a.certificate.body[0] = 2;
        a.certificate.signature =
            bootstrap::sign(certificate::message(&a.certificate), &root_secret(), "root");
        rebind(&mut a, &d);
        cases.push(("certificate version 2", a));
        let (mut a, d) = authority(FLOOR + 1, SIGNER);
        a.certificate.body[70] ^= 1;
        a.certificate.signature =
            bootstrap::sign(certificate::message(&a.certificate), &root_secret(), "root");
        rebind(&mut a, &d);
        cases.push(("certificate key off the curve", a));
        let (mut a, d) = authority(FLOOR + 1, SIGNER);
        let s = p256::n() - BigUint::from_bytes_be(&a.certificate.signature[32..]);
        a.certificate.signature[32..].copy_from_slice(&certificate::scalar_bytes(&s));
        rebind(&mut a, &d);
        cases.push(("high-S root signature", a));
        for (name, at, value) in [
            ("anchor version 2", 0, 2),
            ("anchor for a foreign scheme", 2, 0),
            ("anchor for another wallet", 34, 0),
        ] {
            let (mut a, d) = authority(FLOOR + 1, SIGNER);
            a.anchor.body[at] = if a.anchor.body[at] == value {
                value ^ 1
            } else {
                value
            };
            sign_anchor(&mut a, &d);
            cases.push((name, a));
        }
        let (mut a, d) = authority(FLOOR + 1, SIGNER);
        a.anchor.body[66..98].fill(0);
        sign_anchor(&mut a, &d);
        cases.push(("zero nonce", a));
        let (mut a, d) = authority(FLOOR + 1, SIGNER);
        a.anchor.body[106..138].copy_from_slice(&field_modulus().to_bytes_le());
        sign_anchor(&mut a, &d);
        cases.push(("noncanonical signer field", a));
        let (mut a, d) = authority(FLOOR + 1, SIGNER);
        let other = issue(
            Role::TimeAnchor as u8,
            &signer(SIGNER).1,
            10,
            &root_secret(),
        );
        a.anchor.body[106..138]
            .copy_from_slice(certificate::certificate_digest(&other).to_repr().as_ref());
        sign_anchor(&mut a, &d);
        cases.push(("signer field names another certificate", a));
        let (mut a, _) = authority(FLOOR + 1, SIGNER);
        sign_anchor(&mut a, &signer("not the certified key").0);
        cases.push(("anchor signed by an uncertified key", a));
        let (mut a, _) = authority(FLOOR + 1, SIGNER);
        sign_anchor(&mut a, &wallet().d);
        cases.push(("anchor signed by the wallet payment key", a));
        let (mut a, _) = authority(FLOOR + 1, SIGNER);
        let s = p256::n() - BigUint::from_bytes_be(&a.anchor.signature[32..]);
        a.anchor.signature[32..].copy_from_slice(&certificate::scalar_bytes(&s));
        cases.push(("high-S anchor signature", a));
        for half in [0, 1] {
            let (mut a, _) = authority(FLOOR + 1, SIGNER);
            a.anchor.signature[32 * half..32 * half + 32].fill(0);
            cases.push((["zero r", "zero s"][half], a));
        }
        let (mut a, _) = authority(FLOOR + 1, SIGNER);
        a.anchor.body[98] ^= 1;
        cases.push(("issuer time changed after signing", a));
        cases.push(("no authority at all", Authority::dummy()));
        for (name, a) in cases {
            assert!(
                time_anchor::authenticate(&a, &scheme(), &wallet_id(), &root()).is_err(),
                "native {name}"
            );
            let (before, rest) = funded();
            assert!(!holds(&refresh_from(before, rest, a)), "{name}");
        }
    }

    #[test]
    fn refresh_floor_is_the_exact_maximum() {
        for (time, floor) in [
            (FLOOR - 5_000, FLOOR),
            (FLOOR, FLOOR),
            (FLOOR + 5_000, FLOOR + 5_000),
            (0, FLOOR),
        ] {
            let f = refresh(time);
            assert_eq!(f.input.after_core[28], Fp::from(floor));
            assert!(holds(&f), "max({FLOOR}, {time})");
        }
        let mut cases = vec![];
        for (name, time, claimed) in [
            ("jump above a newer anchor", FLOOR + 5_000, FLOOR + 5_001),
            ("jump above the held floor", FLOOR - 5_000, FLOOR + 1),
            (
                "regression to an older anchor",
                FLOOR - 5_000,
                FLOOR - 5_000,
            ),
            ("newer anchor ignored", FLOOR + 5_000, FLOOR),
            ("floor reset", FLOOR + 5_000, 0),
        ] {
            let mut f = refresh(time);
            f.input.after_core[28] = Fp::from(claimed);
            f.input.statement[19] = Fp::from(claimed);
            reseal(&mut f);
            cases.push((name, f));
        }
        let mut f = refresh(FLOOR + 5_000);
        f.input.statement[19] = Fp::from(FLOOR);
        reseal(&mut f);
        cases.push(("statement floor differs from the successor", f));
        let (a, _) = authority(FLOOR + 1, SIGNER);
        let (before, mut rest) = funded();
        rest[5] = time_anchor::digest(&a.anchor);
        cases.push((
            "reinstalling the held anchor",
            refresh_from(before, rest, a),
        ));
        let (mut before, rest) = funded();
        before[11] = Fp::from_u128(u128::MAX);
        cases.push((
            "sequence overflow",
            refresh_from(before, rest, authority(FLOOR + 1, SIGNER).0),
        ));
        let mut f = refresh(FLOOR + 1);
        f.input.after_core[11] = f.input.before_core[11];
        f.input.statement[9] = f.input.after_core[11];
        reseal(&mut f);
        cases.push(("sequence not advanced", f));
        refuse_all(cases);
        let (mut before, rest) = funded();
        before[11] = Fp::from_u128(u128::MAX - 1);
        assert!(
            holds(&refresh_from(before, rest, authority(FLOOR + 1, SIGNER).0)),
            "last representable sequence"
        );
    }

    #[test]
    fn refresh_preserves_every_other_field_even_when_resigned() {
        for index in 0..29 {
            if time_anchor::CHANGED_CORE.contains(&index) {
                continue;
            }
            let mut f = refresh(FLOOR + 1);
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
                13 => f.input.statement[10] = changed,
                _ => {}
            }
            reseal(&mut f);
            assert!(!holds(&f), "signed bad successor core {index}");
        }
        for index in 0..6 {
            let mut f = refresh(FLOOR + 1);
            f.input.after_rest[index] += Fp::ONE;
            reseal(&mut f);
            assert!(!holds(&f), "signed bad successor rest {index}");
        }
        let mut f = refresh(FLOOR + 1);
        f.input.statement[18] += Fp::ONE;
        reseal(&mut f);
        assert!(!holds(&f), "statement update is not the anchor digest");
        // The successor nonce is native entropy: any canonical nonzero value.
        let mut f = refresh(FLOOR + 1);
        f.input.after_core[29] = Fp::from(99_991);
        reseal(&mut f);
        assert!(holds(&f), "fresh nonzero nonce");
        f.input.after_core[29] = Fp::ZERO;
        reseal(&mut f);
        assert!(!holds(&f), "zero nonce");
    }

    /// The anchor key's comb table is derived from the certified key, never
    /// supplied: any other table value, or another derivation input, refuses.
    #[test]
    fn signer_table_is_derived_from_the_certified_key() {
        let mut b = build(&refresh(FLOOR + 1));
        assert!(harness::holds(&b));
        let named = |b: &harness::Built, name: &str| {
            b.0.arrays.iter().position(|a| a.name == name).unwrap()
        };
        let table = named(&b, "time-anchor signer derived comb table");
        for (cell, instance) in [(1, 0), (4, 300), (6, 511)] {
            let at = b.0.cell_index(table, cell, instance);
            b.2.w[at] += Fp::ONE;
            assert!(!harness::holds(&b), "supplied table cell {cell}/{instance}");
            b.2.w[at] -= Fp::ONE;
        }
        let opening = named(&b, "total signer certificate opening");
        // Byte 40 of the certificate body is inside the certified key.
        let at = b.0.cell_index(opening, 40, 0);
        b.2.w[at] += Fp::ONE;
        assert!(!harness::holds(&b), "certificate key byte");
        b.2.w[at] -= Fp::ONE;
        assert!(harness::holds(&b));
    }

    #[test]
    fn refresh_requires_exact_predecessor_credential_and_own_receipt() {
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
        ] {
            let mut f = refresh(FLOOR + 1);
            f.profile[at] += Fp::ONE;
            cases.push((name, f));
        }
        let mut f = refresh(FLOOR + 1);
        f.input.credential.body[98] ^= 1;
        cases.push(("credential account digest substituted", f));
        let mut f = refresh(FLOOR + 1);
        f.input.statement[19] += Fp::ONE;
        cases.push(("floor changed after signing", f));
        let mut f = refresh(FLOOR + 1);
        sign(&mut f.input, &WalletKey::labelled("another Control wallet"));
        cases.push(("receipt under a foreign key", f));
        let mut f = refresh(FLOOR + 1);
        let st = f.input.statement;
        sign_with(&mut f.input, &f.key, st[16], st[17]);
        cases.push(("operation id from the Load input", f));
        let mut f = refresh(FLOOR + 1);
        let st = f.input.statement;
        sign_with(&mut f.input, &f.key, Fp::from(2), st[18]);
        cases.push(("operation id with the Load tag", f));
        let mut f = refresh(FLOOR + 1);
        f.input.receipt.body[306] = 1;
        f.input.receipt.signature = bootstrap::sign(
            bootstrap::receipt_message(&f.input.receipt),
            &f.key.d,
            "Refresh receipt with a Payment digest",
        );
        cases.push(("receipt carrying a Payment digest", f));
        refuse_all(cases);
    }

    #[test]
    fn control_mode_and_branch_mismatch_fail() {
        let mut cases = vec![];
        let mut f = refresh(FLOOR + 1);
        f.input.mode = Mode::Load;
        cases.push(("Refresh originals in Load mode", f));
        let mut f = load_fixture(3);
        f.input.mode = Mode::RefreshTimeAnchor;
        cases.push(("Load originals in Refresh mode", f));
        let mut f = load_fixture(3);
        f.input.mode = Mode::RefreshTimeAnchor;
        f.input.authority = authority(FLOOR + 1, SIGNER).0;
        cases.push(("Load originals with a valid anchor in Refresh mode", f));
        let mut f = refresh(FLOOR + 1);
        f.input.statement[16] = Fp::from(2);
        reseal(&mut f);
        cases.push(("Refresh with the Load tag", f));
        let mut f = load_fixture(3);
        f.input.statement[16] = Fp::from(7);
        reseal(&mut f);
        cases.push(("Load with the RefreshPolicy tag", f));
        for kind in [1, 2, 3, 5] {
            let mut f = refresh(FLOOR + 1);
            f.input.statement[17] = Fp::from(kind);
            reseal(&mut f);
            cases.push((
                ["", "Credential", "SchemePolicy", "QuotaShare", "", "kind 5"][kind as usize],
                f,
            ));
        }
        let mut f = refresh(FLOOR + 1);
        f.input.statement[20] = Fp::ONE;
        reseal(&mut f);
        cases.push(("Refresh statement field 20", f));
        let mut f = refresh(FLOOR + 1);
        f.input.statement[19] = Fp::from(2).pow_vartime([64]);
        f.input.after_core[28] = f.input.statement[19];
        reseal(&mut f);
        cases.push(("floor above u64", f));
        refuse_all(cases);
    }

    #[test]
    fn control_load_mode_is_exactly_load() {
        // Every Load refusal of load.rs, in Control Load mode and alone.
        let mut cases = vec![];
        for index in [9, 10, 12, 13, 17, 18, 19, 20, 21, 22, 28] {
            let mut f = load_fixture(3);
            f.input.after_core[index] += Fp::ONE;
            reseal(&mut f);
            cases.push((format!("signed bad successor core {index}"), f));
        }
        let mut replay = load_fixture(3);
        replay.input.before_core = replay.input.after_core;
        replay.profile[full::HEAD_IO] = replay.input.statement[15];
        replay.input.statement[14] = replay.input.statement[15];
        sign(&mut replay.input, &replay.key);
        cases.push(("same ordinary ordinal funds twice".into(), replay));
        let mut changed = load_fixture(3);
        changed.input.ordinary[250] ^= 1;
        cases.push(("original payer byte".into(), changed));
        for index in 0..6 {
            let mut f = load_fixture(3);
            f.input.after_rest[index] += Fp::ONE;
            reseal(&mut f);
            cases.push((format!("signed bad successor rest {index}"), f));
        }
        let mut f = load_fixture(3);
        f.input.recovery = load::dummy_recovery();
        cases.push(("recovery insertion replaced by the dummy".into(), f));
        for (name, f) in cases {
            assert!(!holds(&f), "Control Load: {name}");
            assert!(
                !harness::holds(&load_alone(&as_load(&f.input), f.profile)),
                "Load alone: {name}"
            );
        }
        // Inactive invalid authority never blocks Load, and the same bytes
        // cannot authorize a Refresh.
        let reference = entries(&build(&load_fixture(3)));
        let (mut bad, _) = authority(FLOOR + 1, SIGNER);
        bad.certificate.body[34] = Role::RegulatoryPolicy as u8;
        let s = p256::n() - BigUint::from_bytes_be(&bad.anchor.signature[32..]);
        bad.anchor.signature[32..].copy_from_slice(&certificate::scalar_bytes(&s));
        let garbage = Authority {
            certificate: Certificate {
                body: [0xff; 108],
                signature: [0xff; 64],
            },
            anchor: TimeAnchor {
                body: [0xff; time_anchor::BODY_BYTES],
                signature: [0xff; 64],
            },
        };
        for (name, authority) in [
            ("wrong-role, high-S authority", bad),
            ("all-ones authority", garbage),
            ("valid newer anchor", authority(FLOOR + 1, SIGNER).0),
        ] {
            let mut f = load_fixture(3);
            f.input.authority = authority.clone();
            let b = build(&f);
            assert!(harness::holds(&b), "Load with inactive {name}");
            assert_eq!(entries(&b), reference, "same program with inactive {name}");
            if name != "valid newer anchor" {
                let (before, rest) = funded();
                assert!(
                    !holds(&refresh_from(before, rest, authority)),
                    "{name} cannot authorize Refresh"
                );
            }
        }
        // The Load witness itself is accepted by the Load program.
        let (input, profile) = load_witness(3);
        assert!(harness::holds(&load_alone(&input, profile)));
    }

    fn build_prepared(f: &Fixture) -> harness::Built {
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let predecessor = harness::predecessor(&mut w, family, f.profile);
        let prepared = append_preparation(&mut w, family, &root(), &f.input, &predecessor).unwrap();
        harness::expose(
            &mut w,
            family,
            4,
            &[
                prepared.head(),
                Source {
                    array: prepared.statement().local,
                    cell: prepared.statement().digest,
                    instance: 0,
                },
                prepared.body().operation_id(),
            ],
        );
        harness::expose(&mut w, family, 32, &prepared.lineage());
        harness::expose(&mut w, family, MODE_IO, &prepared.mode().inputs());
        harness::finish(w, 16)
    }

    #[test]
    fn prepared_control_applies_the_same_relation_before_advance() {
        let a = build_prepared(&load_fixture(3));
        let b = build_prepared(&refresh(FLOOR + 1));
        assert!(harness::holds(&a));
        assert!(harness::holds(&b));
        assert_eq!(entries(&a), entries(&b));
        let mut jump = refresh(FLOOR - 5);
        jump.input.after_core[28] = Fp::from(FLOOR + 1);
        jump.input.statement[19] = jump.input.after_core[28];
        reseal(&mut jump);
        let mut bad = load_fixture(3);
        bad.input.after_core[9] += Fp::ONE;
        reseal(&mut bad);
        let (before, rest) = funded();
        let mut role = authority(FLOOR + 1, SIGNER);
        role.0.certificate = issue(3, &signer(SIGNER).1, 9, &root_secret());
        rebind(&mut role.0, &role.1);
        for (name, f) in [
            ("floor jump", jump),
            ("Load overcredit", bad),
            ("wrong role", refresh_from(before, rest, role.0)),
        ] {
            assert!(!harness::holds(&build_prepared(&f)), "prepared {name}");
        }
    }

    /// Census of the three Control modes and standalone Load.
    #[test]
    fn control_relation_census() {
        type Census = Vec<(&'static str, [usize; 4])>;
        const SPONGE: [&str; 6] = [
            "certificate sponge initial state",
            "input",
            "full round",
            "partial round 4",
            "7 partial rounds",
            "mask",
        ];
        fn census(
            w: &Wiring<Fp>,
            start: usize,
            component: impl Fn(&str) -> &'static str,
        ) -> Census {
            let mut groups: Census = vec![];
            let mut current = "";
            for a in &w.arrays[start..] {
                if !SPONGE.contains(&a.name) {
                    current = component(a.name);
                }
                let at = match groups.iter().position(|(n, _)| *n == current) {
                    Some(at) => at,
                    None => {
                        groups.push((current, [0; 4]));
                        groups.len() - 1
                    }
                };
                let instances = a.r_count * w.counts[a.family];
                let g = &mut groups[at].1;
                g[0] += a.rows.len() * instances;
                g[1] += a.lookups.len() * instances;
                g[2] += a.cells() * instances;
                if a.name == "input" {
                    g[3] += w.counts[a.family];
                }
            }
            groups
        }
        fn print(label: &str, groups: &Census) -> [usize; 4] {
            println!("{label} census: rows lookups cells permutations  component");
            let mut total = [0; 4];
            for (name, g) in groups {
                println!(
                    "{label} census: {:7} {:6} {:7} {:5}  {name}",
                    g[0], g[1], g[2], g[3]
                );
                for i in 0..4 {
                    total[i] += g[i];
                }
            }
            println!(
                "{label} census: {:7} {:6} {:7} {:5}  total",
                total[0], total[1], total[2], total[3]
            );
            total
        }
        // Components follow construction order; a name seen again keeps the
        // component it first belonged to only where it is unambiguous.
        fn component(phase: &std::cell::Cell<u8>, name: &str) -> &'static str {
            match name {
                "Control mode selector" => "mode selector",
                "canonical monetary Core30 and Rest6" => "two Core30/Rest6 state openings",
                "operation authentication range"
                | "certificate bytes and integers"
                | "certificate issuer square"
                | "certificate issuer curve"
                | "current credential identity ports"
                    if phase.get() == 0 =>
                {
                    "current credential opening and continuity"
                }
                "canonical26 field statement" => {
                    phase.set(1);
                    "selected statement and digest"
                }
                "exact ordinary Load receipt" => "Load: exact ordinary receipt",
                "complete Load monetary and original joins" => {
                    "Load: joins (Load-only enabled) and recovery entry"
                }
                "exact indexed-map insertion" | "monetary recovery joins" => {
                    "shared depth-32 recovery insertion and joins"
                }
                "exact authenticated own receipt" | "exact unsigned prepared receipt" => {
                    phase.set(2);
                    "own receipt (selected op, SHA-256, comb ECDSA, wallet table)"
                }
                "signer root fixed table" | "total signer certificate opening" => {
                    phase.set(3);
                    "Refresh: signer certificate opening (role 4, curve, digests)"
                }
                "certificate issuer square" | "certificate issuer curve" if phase.get() == 3 => {
                    "Refresh: signer certificate opening (role 4, curve, digests)"
                }
                "total signer-certificate root ECDSA arithmetic" => {
                    "Refresh: total root ECDSA (fixed root table)"
                }
                "total signer certificate verdict" => {
                    "Refresh: total root ECDSA (fixed root table)"
                }
                "time-anchor signer derived comb table" => {
                    phase.set(4);
                    "Refresh: signer comb table derived in circuit"
                }
                "total time-anchor opening" => {
                    phase.set(5);
                    "Refresh: anchor opening (body, m, d, T)"
                }
                "selected time-anchor signature" => {
                    phase.set(6);
                    "Refresh hard anchor signature with selected inputs"
                }
                "RefreshTimeAnchor verdict, maximum and enabled joins" => {
                    "Refresh: verdict, exact maximum, enabled joins"
                }
                _ => match phase.get() {
                    0 => "current credential opening and continuity",
                    2 => "own receipt (selected op, SHA-256, comb ECDSA, wallet table)",
                    3 => "Refresh: total root ECDSA (fixed root table)",
                    4 => "Refresh: signer comb table derived in circuit",
                    6 => "Refresh hard anchor signature with selected inputs",
                    _ => "other",
                },
            }
        }
        let mut shapes = vec![];
        let mut totals = vec![];
        let (input, profile, key) = crate::unload::tests::control_fixture();
        let unload = Fixture {
            input: Witness::unload(input),
            profile,
            key,
        };
        for (label, f) in [
            ("load-mode", load_fixture(3)),
            ("refresh-mode", refresh(FLOOR + 1)),
            ("unload-mode", unload),
        ] {
            let mut w = Wiring::default();
            let family = w.family(0, 1);
            let predecessor = harness::predecessor(&mut w, family, f.profile);
            let start = w.arrays.len();
            append(&mut w, family, &root(), &f.input, &predecessor).unwrap();
            let phase = std::cell::Cell::new(0);
            assert_eq!(
                w.arrays[start..]
                    .iter()
                    .filter(|array| array.name == "exact indexed-map insertion")
                    .count(),
                1,
                "{label} must use one recovery insertion",
            );
            let groups = census(&w, start, |name| component(&phase, name));
            totals.push(print(label, &groups));
            let b = build(&f);
            assert!(harness::holds(&b));
            shapes.push((
                label,
                b.0.shape.sx,
                b.0.shape.row_bits,
                b.0.shape.col_bits,
                b.0.shape.cols(),
            ));
        }
        assert!(
            totals.iter().all(|total| total == &totals[0]),
            "three modes have one layout"
        );
        let (input, profile) = load_witness(3);
        let mut w = Wiring::default();
        let family = w.family(0, 1);
        let predecessor = harness::predecessor(&mut w, family, profile);
        let start = w.arrays.len();
        load::append(&mut w, family, &input, &predecessor).unwrap();
        let phase = std::cell::Cell::new(0);
        let alone = print(
            "load-alone",
            &census(&w, start, |name| component(&phase, name)),
        );
        let b = load_alone(&input, profile);
        assert!(harness::holds(&b));
        shapes.push((
            "load-alone",
            b.0.shape.sx,
            b.0.shape.row_bits,
            b.0.shape.col_bits,
            b.0.shape.cols(),
        ));
        for (label, sx, row_bits, col_bits, cols) in shapes {
            println!("shape {label}: sx {sx} row_bits {row_bits} col_bits {col_bits} cols {cols}");
        }
        println!(
            "sharing cost: Control {} rows / {} lookups / {} cells vs Load alone {} / {} / {} (+{} rows, +{} lookups, +{} cells)",
            totals[0][0],
            totals[0][1],
            totals[0][2],
            alone[0],
            alone[1],
            alone[2],
            totals[0][0] - alone[0],
            totals[0][1] - alone[1],
            totals[0][2] - alone[2]
        );
    }
}
