//! Exact current-credential continuity from a verified recursive predecessor.
//!
//! An opening alone conveys no authority. `CurrentCredential` can be constructed
//! only by joining all immutable identity/key/table ports to the sealed recursive
//! proof source. State continuity additionally binds its exact predecessor head.

use ff::{Field, PrimeField};
use iroha_pasta::Fp;
use num_bigint::BigUint;

use crate::{
    certificate::{self, Cells},
    credential::{self, Credential},
    full::{VerifiedPredecessor, Wiring},
    monetary_state::StateCells,
    nonnative, p256,
    send_arithmetic::Source,
    uniform::{Array, Lin, constant, link, own, plus, scaled},
};

/// Self-contained exact credential opening. Identity fields are issuer claims;
/// original admission and the credential's signature are authenticated either
/// at Bootstrap/renewal or by verified current/prior digest continuity.
pub struct CredentialOpening {
    local: usize,
    bytes: usize,
    digest: Source,
    valid: Source,
    scheme: [Source; 2],
    asset: [Source; 2],
    wallet: [Source; 2],
    account: [Source; 2],
    provider: [Source; 2],
    payment_key: [Source; 4],
    table: [Source; 4],
    permitted: Source,
    max_response: Source,
    lease: Source,
    renewal_sequence: Source,
}
impl CredentialOpening {
    pub fn digest(&self) -> Source {
        self.digest
    }
    /// Exact total self-contained verdict; it supplies no issuer/history trust.
    pub fn valid(&self) -> Source {
        self.valid
    }
    pub fn scheme(&self) -> [Source; 2] {
        self.scheme
    }
    pub fn asset(&self) -> [Source; 2] {
        self.asset
    }
    pub fn wallet(&self) -> [Source; 2] {
        self.wallet
    }
    pub fn account(&self) -> [Source; 2] {
        self.account
    }
    pub fn provider(&self) -> [Source; 2] {
        self.provider
    }
    /// Numeric x-low, x-high, y-low, y-high halves, as recursive public IO.
    pub fn payment_key(&self) -> [Source; 4] {
        self.payment_key
    }
    /// Numeric x-low, x-high, y-low, y-high of the certified Vesta commitment.
    pub fn table_commitment(&self) -> [Source; 4] {
        self.table
    }
    pub fn permitted(&self) -> Source {
        self.permitted
    }
    pub fn max_response(&self) -> Source {
        self.max_response
    }
    pub fn lease(&self) -> Source {
        self.lease
    }
    pub fn renewal_sequence(&self) -> Source {
        self.renewal_sequence
    }
    pub fn byte(&self, index: usize) -> Source {
        assert!(
            index < credential::BODY_BYTES + 64,
            "credential body/signature byte"
        );
        Source {
            array: self.bytes,
            cell: index,
            instance: 0,
        }
    }
    pub(crate) fn payment_point(&self, w: &Wiring<Fp>) -> p256::Point {
        let integer = |source: Source| BigUint::from_bytes_le(source.value(w).to_repr().as_ref());
        p256::Point {
            x: integer(self.payment_key[0]) + (integer(self.payment_key[1]) << 128),
            y: integer(self.payment_key[2]) + (integer(self.payment_key[3]) << 128),
        }
    }
}

/// Credential authenticated by exact digest/key/table continuity from a
/// verified predecessor. Its private constructor prevents a plain byte opening
/// from being passed to an own-receipt signature verifier as an authority.
pub struct CurrentCredential {
    opening: CredentialOpening,
}
impl std::ops::Deref for CurrentCredential {
    type Target = CredentialOpening;
    fn deref(&self) -> &Self::Target {
        &self.opening
    }
}

pub(crate) fn ensure_range(w: &mut Wiring<Fp>, family: usize) {
    if w.tables.iter().any(|(_, id)| *id == nonnative::RANGE_TABLE) {
        return;
    }
    let range = w.push(Array {
        name: "operation authentication range",
        family,
        rho: 11,
        r_count: 2048,
        fixed: true,
        lookups: vec![],
        widths: vec![11],
        rows: vec![],
        values: vec![(0..2048).map(|v| Fp::from(v as u64)).collect()],
    });
    w.tables.push((range, nonnative::RANGE_TABLE));
}
fn byte_expression(array: usize, start: usize, len: usize, reverse: bool) -> Lin<Fp> {
    (0..len).fold(vec![], |a, i| {
        plus(
            &a,
            &link(
                array,
                if reverse {
                    start + len - 1 - i
                } else {
                    start + i
                },
                vec![Some(0)],
                Fp::from(256).pow_vartime([i as u64]),
            ),
        )
    })
}
fn join(w: &mut Wiring<Fp>, host: usize, a: Source, b: Source) {
    assert_eq!(
        w.arrays[a.array].family, w.arrays[host].family,
        "same global family"
    );
    assert_eq!(
        w.arrays[b.array].family, w.arrays[host].family,
        "same global family"
    );
    w.arrays[host].rows.push([
        plus(&a.linear(), &scaled(&b.linear(), -Fp::ONE)),
        constant(vec![Fp::ONE]),
        vec![],
    ]);
}

/// Open all 532 body bytes and all 64 signature bytes with canonical curves,
/// low-S, evidence/regulatory rules, and exact native object digest. This is a
/// hard-valid opening; callers that support an invalid-input branch must select
/// a canonical dummy through constraints before calling it.
pub fn append_opening(
    w: &mut Wiring<Fp>,
    family: usize,
    credential: &Credential,
) -> CredentialOpening {
    let opening = append_opening_total(w, family, credential);
    w.arrays[opening.local].rows.push([
        plus(&opening.valid.linear(), &constant(vec![-Fp::ONE])),
        constant(vec![Fp::ONE]),
        vec![],
    ]);
    opening
}

/// Exact arbitrary original bytes and a total verdict. Invalid fields preserve
/// the same circuit shape; all hash and arithmetic helper computations remain
/// hard constrained, so a valid original cannot be falsely classified invalid.
pub fn append_opening_total(
    w: &mut Wiring<Fp>,
    family: usize,
    credential: &Credential,
) -> CredentialOpening {
    assert_eq!((w.pi[family], w.counts[family]), (0, 1));
    ensure_range(w, family);
    let contents = credential::append_contents_total(w, family, credential);
    let mut c = Cells::new(family);
    c.a.name = "current credential identity ports";
    let mut pack = |start: usize, len: usize, reverse: bool| {
        let b = &credential.body[start..start + len];
        let integer = if reverse {
            BigUint::from_bytes_be(b)
        } else {
            BigUint::from_bytes_le(b)
        };
        let k = c.cell(certificate::fp(&integer), 255);
        c.eq(
            own(k, 1),
            byte_expression(contents.local, start, len, reverse),
        );
        k
    };
    let scheme = [pack(2, 16, false), pack(18, 16, false)];
    let asset = [pack(34, 16, false), pack(50, 16, false)];
    let wallet = [pack(66, 16, false), pack(82, 16, false)];
    let account = [pack(98, 16, false), pack(114, 16, false)];
    let provider = [pack(259, 16, false), pack(275, 16, false)];
    let payment_key = [
        pack(147, 16, true),
        pack(131, 16, true),
        pack(179, 16, true),
        pack(163, 16, true),
    ];
    let table = [
        pack(195, 16, false),
        pack(211, 16, false),
        pack(227, 16, false),
        pack(243, 16, false),
    ];
    let permitted = pack(436, 4, false);
    let max_response = pack(440, 8, false);
    let lease = pack(492, 8, false);
    let renewal_sequence = pack(488, 4, false);
    let local = w.push(c.a);
    let source = |cell| Source {
        array: local,
        cell,
        instance: 0,
    };
    CredentialOpening {
        local,
        bytes: contents.local,
        digest: Source {
            array: contents.local,
            cell: contents.digest_cell,
            instance: 0,
        },
        valid: Source {
            array: contents.local,
            cell: contents.valid_cell,
            instance: 0,
        },
        scheme: scheme.map(source),
        asset: asset.map(source),
        wallet: wallet.map(source),
        account: account.map(source),
        provider: provider.map(source),
        payment_key: payment_key.map(source),
        table: table.map(source),
        permitted: source(permitted),
        max_response: source(max_response),
        lease: source(lease),
        renewal_sequence: source(renewal_sequence),
    }
}

/// Authenticate a byte opening against a sealed verified proof's current
/// credential digest, complete identity/key context and certified wallet table.
/// Applicable to the wallet predecessor and an independently verified payer Send.
pub fn bind_verified(
    w: &mut Wiring<Fp>,
    opening: CredentialOpening,
    predecessor: &VerifiedPredecessor,
) -> CurrentCredential {
    w.arrays[opening.local].rows.push([
        plus(&opening.valid.linear(), &constant(vec![-Fp::ONE])),
        constant(vec![Fp::ONE]),
        vec![],
    ]);
    let context = predecessor.context();
    let fields = opening
        .scheme
        .into_iter()
        .zip(context[..2].iter().copied())
        .chain(opening.asset.into_iter().zip(context[4..6].iter().copied()))
        .chain(
            opening
                .wallet
                .into_iter()
                .zip(context[6..8].iter().copied()),
        )
        .chain(std::iter::once((opening.digest, context[8])))
        .chain(
            opening
                .payment_key
                .into_iter()
                .zip(context[9..13].iter().copied()),
        )
        .chain(opening.table.into_iter().zip(predecessor.table()));
    for (a, b) in fields {
        join(w, opening.local, a, b);
    }
    CurrentCredential { opening }
}

/// Authenticate the current credential and exact predecessor State opening.
/// Root/issuer signatures are deliberately inherited from verified history;
/// a new or renewed credential requires its separate authority relation.
pub fn append_current(
    w: &mut Wiring<Fp>,
    family: usize,
    credential: &Credential,
    state: &StateCells,
    predecessor: &VerifiedPredecessor,
) -> Result<CurrentCredential, &'static str> {
    let opening = append_opening(w, family, credential);
    let current = bind_verified(w, opening, predecessor);
    let core = |cell| Source {
        array: state.local,
        cell: state.core[cell],
        instance: 0,
    };
    let rest = |cell| Source {
        array: state.local,
        cell: state.rest[cell],
        instance: 0,
    };
    for (a, b) in current
        .scheme()
        .into_iter()
        .zip([core(1), core(2)])
        .chain(current.asset().into_iter().zip([core(3), core(4)]))
        .chain(current.wallet().into_iter().zip([core(5), core(6)]))
        .chain([
            (current.digest(), core(7)),
            (current.permitted(), rest(0)),
            (current.max_response(), core(25)),
            (current.lease(), core(26)),
            (
                Source {
                    array: state.local,
                    cell: state.commitment,
                    instance: 0,
                },
                predecessor.head(),
            ),
        ])
    {
        join(w, current.local, a, b);
    }
    Ok(current)
}

#[cfg(test)]
#[path = "operation_auth_tests.rs"]
pub(crate) mod tests;
