//! In-circuit reconstruction of the compact CreditStatus receiver profile.

use super::{Fp, IncomingProofFacts, Wiring};
use crate::{
    bootstrap_context as bc,
    certificate::Cells,
    send_arithmetic::Source,
    uniform::{Lin, constant, plus},
};
use ff::Field;

pub(crate) struct Originals<'a> {
    pub statement: &'a [Source; 26],
    pub statement_digest: Source,
    pub receipt_digest: Source,
    pub wallet: [Source; 2],
    pub key: [Source; 4],
    pub table: [Source; 4],
    pub send_form: Source,
    pub subject: Source,
}

/// The exact statement and receipt digests enter CreditStatus. The quoted
/// credential opens the signed Request's credential digest. The six dynamic
/// lineage words enter the exact Ω transcript directly from these same leaves.
pub(crate) fn bind(
    w: &mut Wiring<Fp>,
    family: usize,
    incoming: &IncomingProofFacts,
    originals: Originals<'_>,
) {
    let s = originals.statement;
    let mut cells = Cells::new(family);
    cells.a.name = "ArchiveSent compact receiver profile";
    let mut profile: [Lin<Fp>; 64] = std::array::from_fn(|_| Vec::new());
    profile[0] = constant(vec![Fp::ONE]);
    profile[super::HEAD_IO] = s[15].linear();
    profile[super::STATEMENT_IO] = originals.statement_digest.linear();
    profile[super::RECEIPT_IO] = originals.receipt_digest.linear();
    // A renewal can change the current credential in the statement. Wallet,
    // payment key and certified table come from the original receiver quote.
    let context = [
        s[3],
        s[4],
        s[1],
        s[2],
        s[5],
        s[6],
        originals.wallet[0],
        originals.wallet[1],
        s[7],
        originals.key[0],
        originals.key[1],
        originals.key[2],
        originals.key[3],
    ];
    for (i, word) in context.into_iter().enumerate() {
        profile[bc::CONTEXT_IO + i] = word.linear();
    }
    for (i, word) in originals.table.into_iter().enumerate() {
        profile[super::TABLE_IO + i] = word.linear();
    }
    profile[super::KIND_IO] = originals.send_form.linear();
    // Receive has wire tag 4 and selector 0. A later Send has wire tag 3
    // and selector 1. The recursive slot separately enforces Booleanity.
    cells.eq(
        plus(&s[16].linear(), &originals.send_form.linear()),
        constant(vec![Fp::from(4)]),
    );
    cells.eq(originals.subject.linear(), s[17].linear());
    for (i, word) in profile.into_iter().enumerate() {
        if !(bc::LINEAGE_IO..bc::LINEAGE_IO + bc::LINEAGE_FIELDS).contains(&i) {
            cells.eq(incoming.at(i).linear(), word);
        }
    }
    w.push(cells.a);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{carrier::ProjectedInputs, full, operation_auth::tests as harness};
    use group::{Curve, Group};
    use iroha_pasta::Eq;

    #[test]
    fn both_native_profiles_bind_every_word_outside_the_omega_lineage() {
        for kind in [full::RECEIVE, full::SEND] {
            let mut statement: [Fp; 26] = std::array::from_fn(|i| Fp::from(i as u64 + 100));
            statement[16] = Fp::from(if kind == full::RECEIVE { 4 } else { 3 });
            let key = [11, 12, 13, 14].map(Fp::from);
            let wallet = [21, 22].map(Fp::from);
            let table = crate::spartan::limbs_of_point::<Eq>(&Eq::generator().to_affine());
            let context = [
                statement[3],
                statement[4],
                statement[1],
                statement[2],
                statement[5],
                statement[6],
                wallet[0],
                wallet[1],
                statement[7],
                key[0],
                key[1],
                key[2],
                key[3],
            ];
            let statement_digest = crate::statement::digest(&statement);
            let receipt_digest = Fp::from(987);
            let lineage = std::array::from_fn(|i| Fp::from(i as u64 + 50));
            let profile = ProjectedInputs::from_authenticated_fields(
                statement[15],
                statement_digest,
                receipt_digest,
                context,
                lineage,
                table,
                kind,
                [Fp::ZERO; 2],
            )
            .unwrap();
            let authenticated: [Fp; 64] = profile.primary().io.as_slice().try_into().unwrap();
            let build = |index: Option<usize>, form: usize, subject: Fp| {
                let mut values = authenticated;
                if let Some(i) = index {
                    values[i] += Fp::ONE;
                }
                let mut w = Wiring::default();
                let family = w.family(0, 1);
                let facts = IncomingProofFacts::fixture(&mut w, family, values, Fp::ONE, Fp::ONE);
                let mut owner = Cells::new(family);
                let mut put = |v: Fp| Source {
                    array: w.arrays.len(),
                    cell: owner.cell(v, 255),
                    instance: 0,
                };
                let statement_cells = statement.map(&mut put);
                let sources = Originals {
                    statement: &statement_cells,
                    statement_digest: put(statement_digest),
                    receipt_digest: put(receipt_digest),
                    wallet: wallet.map(&mut put),
                    key: key.map(&mut put),
                    table: table.map(&mut put),
                    send_form: put(Fp::from(form as u64)),
                    subject: put(subject),
                };
                w.push(owner.a);
                bind(&mut w, family, &facts, sources);
                harness::holds(&harness::finish(w, 10))
            };
            assert!(build(None, kind, statement[17]));
            assert!(!build(None, 1 - kind, statement[17]));
            assert!(!build(None, kind, statement[17] + Fp::ONE));
            for i in 0..64 {
                if !(bc::LINEAGE_IO..bc::LINEAGE_IO + bc::LINEAGE_FIELDS).contains(&i) {
                    assert!(
                        !build(Some(i), kind, statement[17]),
                        "kind {kind}, index {i}"
                    );
                }
            }
        }
    }
}
