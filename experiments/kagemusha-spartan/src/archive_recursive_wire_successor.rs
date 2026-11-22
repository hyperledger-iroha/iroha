//! Wallet-wire evidence for the recursive ArchiveSent verifier. The exact Ω proof
//! bytes come from the same recursive verifier that decides the receiver's two curves.
use super::transcript::{Transcript, status_chain};
use super::*;
use iroha_data_model::kagemusha::{
    KAGEMUSHA_WALLET_LINEAGE_DOMAIN_V1, KAGEMUSHA_WALLET_LINEAGE_PUBLIC_TRANSCRIPT_BYTES_V1,
};

fn lineage(w: &mut Wiring<Fp>, family: usize, incoming: &ArchiveIncoming<'_>) -> Source {
    let context = incoming.facts.context();
    let state = incoming.facts.lineage();
    let mut t = Transcript::new(family);
    t.fixed(&1u16.to_le_bytes());
    for i in [bc::SCHEME, bc::SCHEME + 1, bc::RELATION, bc::RELATION + 1] {
        t.integer(w, context[i], 16);
    }
    t.integer(w, incoming.facts.head(), 32);
    for i in [bc::WALLET, bc::WALLET + 1] {
        t.integer(w, context[i], 16);
    }
    t.integer(w, context[bc::CREDENTIAL], 32);
    // SEC1 uncompressed is 04 || x || y. Each coordinate is big-endian,
    // whereas context holds the low numerical limb before the high limb.
    t.fixed(&[4]);
    for i in [1, 0, 3, 2] {
        t.big_endian_limb(w, context[bc::PAYMENT_KEY + i]);
    }
    for (index, width) in [
        (bc::LIFECYCLE, 1),
        (bc::POLICY_EPOCH, 8),
        (bc::ENABLED_CONTROLS, 4),
        (bc::BURNED_TOTAL, 16),
        (bc::PENDING_ROOT, 32),
        (bc::CREDIT_ROOT, 32),
    ] {
        t.integer(w, state[index], width);
    }
    t.digest_with_tail(
        w,
        family,
        KAGEMUSHA_WALLET_LINEAGE_DOMAIN_V1,
        KAGEMUSHA_WALLET_LINEAGE_PUBLIC_TRANSCRIPT_BYTES_V1,
        Some(incoming.exact_pair_bytes),
    )
}

/// Bind the actual ArchiveSent statement in the wallet's statement-digest slot.
/// Each field comes from the authenticated payer predecessor, constrained core
/// transition or exact receiver evidence. The branch only affects adjusted lineage.
pub(super) fn append(
    w: &mut Wiring<Fp>,
    family: usize,
    predecessor: &VerifiedPredecessor,
    incoming: &ArchiveIncoming<'_>,
    prior: &monetary_state::StateCells,
    next: &monetary_state::StateCells,
    credit: Source,
    ports: &super::receiver_signature::ReceiptResults,
) -> crate::statement::StatementCells {
    let lineage = lineage(w, family, incoming);
    let [_, _, credited] = status_chain(
        w,
        family,
        credit,
        ports.public[4],
        [
            ports.public[14],
            ports.proof_digest,
            ports.public[5],
            lineage,
        ],
        &ports.opening,
    );
    let p = |i| source(prior.local, prior.core[i]);
    let n = |i| source(next.local, next.core[i]);
    let context = predecessor.context();
    let fields = [
        (1, context[bc::RELATION]),
        (2, context[bc::RELATION + 1]),
        (3, p(1)),
        (4, p(2)),
        (5, p(3)),
        (6, p(4)),
        (7, p(7)),
        (8, p(0)),
        (9, n(11)),
        (10, n(13)),
        (11, p(22)),
        (14, source(prior.local, prior.commitment)),
        (15, source(next.local, next.commitment)),
        (17, credit),
        (18, credited),
    ];
    let mut values = [Fp::ZERO; 26];
    values[0] = Fp::ONE;
    values[16] = Fp::from(5);
    for (i, owner) in fields {
        values[i] = owner.value(w);
    }
    let statement =
        crate::statement::append_statement(w, family, crate::statement::Kind::Archive, &values);
    let mut joins = Cells::new(family);
    for (i, owner) in fields {
        joins.eq(
            source(statement.local, statement.fields[i]).linear(),
            owner.linear(),
        );
    }
    w.push(joins.a);
    expose(
        w,
        family,
        full::STATEMENT_IO,
        &[source(statement.local, statement.digest)],
    );
    statement
}
