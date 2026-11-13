//! Exact Send pending and fee-map effects, composed only by the Send owner.

use ff::Field;
use iroha_pasta::{Fp, poseidon};

use crate::{
    certificate::{self, Cells},
    fs::SpikeRng,
    full::Wiring,
    indexed_map::{self, InsertWitness},
    monetary_state::StateCells,
    request::{RequestCells, word},
    send_arithmetic::Source,
    statement::StatementCells,
    uniform::{constant, own, plus, scaled},
};

pub const PENDING_DOMAIN: u64 = u64::from_le_bytes(*b"kgwpout1");
pub const FEE_DOMAIN: u64 = u64::from_le_bytes(*b"kgwfee_1");
pub const DEPTH: usize = 32;

#[derive(Clone)]
pub struct Witness {
    pub pending: InsertWitness<DEPTH>,
    pub fee: InsertWitness<DEPTH>,
}

pub(crate) struct MapCells {
    pub pending_root: Source,
    pub fee_root: Source,
    pub has_fee: Source,
}

fn source(array: usize, cell: usize) -> Source {
    Source {
        array,
        cell,
        instance: 0,
    }
}

pub(crate) fn append(
    w: &mut Wiring<Fp>,
    family: usize,
    before: &StateCells,
    after: &StateCells,
    request: &RequestCells,
    statement: &StatementCells,
    adjusted_pending: Source,
    witness: &Witness,
) -> MapCells {
    let credit = source(statement.local, statement.fields[17]);
    let fee = source(request.local, request.words[word::FEE]);
    let schedule = source(request.local, request.words[word::FEE_SCHEDULE]);
    let pending_inputs: Vec<_> = statement.fields[17..24]
        .iter()
        .map(|k| source(statement.local, *k))
        .collect();
    let fee_inputs = [credit, fee, schedule];
    let pending_value = poseidon::hash_with_domain(
        PENDING_DOMAIN,
        &pending_inputs
            .iter()
            .map(|p| p.value(w))
            .collect::<Vec<_>>(),
    );
    let fee_value = poseidon::hash_with_domain(
        FEE_DOMAIN,
        &fee_inputs.iter().map(|p| p.value(w)).collect::<Vec<_>>(),
    );
    let mut c = Cells::new(family);
    c.a.name = "Send pending descriptor and earned fee";
    let zero = c.bit(fee.value(w) == Fp::ZERO);
    let inverse = c.cell(
        Option::<Fp>::from(fee.value(w).invert()).unwrap_or(Fp::ZERO),
        255,
    );
    c.a.rows.push([fee.linear(), own(zero, 1), vec![]]);
    c.a.rows.push([
        fee.linear(),
        own(inverse, 1),
        plus(&constant(vec![Fp::ONE]), &scaled(&own(zero, 1), -Fp::ONE)),
    ]);
    let has_fee = c.bit(fee.value(w) != Fp::ZERO);
    c.eq(
        plus(&own(zero, 1), &own(has_fee, 1)),
        constant(vec![Fp::ONE]),
    );
    let schedule_inverse = c.cell(
        if fee.value(w) == Fp::ZERO {
            Fp::ZERO
        } else {
            Option::<Fp>::from(schedule.value(w).invert()).unwrap_or(Fp::ZERO)
        },
        255,
    );
    // A nonzero fee cannot name an absent schedule.
    c.a.rows
        .push([schedule.linear(), own(schedule_inverse, 1), own(has_fee, 1)]);
    let pending_state = [
        c.cell(Fp::ZERO, 255),
        c.cell(pending_value, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let fee_state = [
        c.cell(Fp::ZERO, 255),
        c.cell(fee_value, 255),
        c.cell(Fp::ZERO, 255),
    ];
    let local = w.push(c.a);
    let mut rng = SpikeRng::new(b"complete Send map commitments");
    for (domain, inputs, outputs) in [
        (PENDING_DOMAIN, pending_inputs.as_slice(), pending_state),
        (FEE_DOMAIN, fee_inputs.as_slice(), fee_state),
    ] {
        let inputs: Vec<_> = inputs.iter().map(|p| (p.value(w), p.linear())).collect();
        certificate::hash_relation(w, family, local, domain, &inputs, outputs, &mut rng);
    }
    let pending = indexed_map::append_insert(w, family, &witness.pending);
    let fee = indexed_map::append_insert_if(
        w,
        family,
        &witness.fee,
        source(local, has_fee),
        source(before.local, before.core[20]),
        credit,
        source(local, fee_state[1]),
    );
    let pending_root = source(pending.local, pending.new_root);
    let fee_root = source(fee.local, fee.new_root);
    let mut joins = Cells::new(family);
    joins.a.name = "Send exact map root and descriptor joins";
    for (a, b) in [
        (source(pending.local, pending.old_root), adjusted_pending),
        (source(pending.local, pending.key), credit),
        (
            source(pending.local, pending.value),
            source(local, pending_state[1]),
        ),
        (pending_root, source(after.local, after.core[18])),
        (fee_root, source(after.local, after.core[20])),
    ] {
        joins.eq(a.linear(), b.linear());
    }
    w.push(joins.a);
    MapCells {
        pending_root,
        fee_root,
        has_fee: source(local, has_fee),
    }
}
