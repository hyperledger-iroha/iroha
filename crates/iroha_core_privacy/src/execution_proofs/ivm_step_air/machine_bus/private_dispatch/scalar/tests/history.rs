//! Locally valid histories joined to every original private dispatcher producer.

use super::*;

use super::super::super::super::{
    E, NOTE_COPY_WIDTH_V1, ORDERED, PublicPacketBus, ROW_WIDTH, SORTED, permutation,
    private_history,
};
use packet::Event;
fn as_event(fields: &[F; packet::WIDTH]) -> Option<Event> {
    (fields[ENABLED] == F::ONE).then(|| Event {
        space: match fields[SPACE].0 {
            2 => Space::Register,
            4 => Space::Owner,
            _ => panic!("only original scalar register/control ports"),
        },
        vm: fields[VM].0 as u8,
        generation: fields[GENERATION].0 as u16,
        index: fields[INDEX].0 as u32,
        write: fields[WRITE] == F::ONE,
        before: bytes(packet::half(fields, BEFORE, 0)),
        after: bytes(packet::half(fields, AFTER, 0)),
        before_private: fields[BEFORE_TAG].0 as u16,
        after_private: fields[AFTER_TAG].0 as u16,
    })
}
pub(super) fn accepts(fixture: &ScalarFixture, substitution: Option<(usize, bool)>) -> bool {
    let mut original = fixture.0.packets.clone();
    let mut events = original.fields.iter().map(as_event).collect::<Vec<_>>();
    if let Some((slot, missing)) = substitution {
        if missing {
            events[slot] = None;
        } else {
            let event = events[slot].as_mut().unwrap();
            let value = u64::from_le_bytes(event.after[..8].try_into().unwrap()).wrapping_add(1);
            event.after = bytes(value);
            if !event.write {
                event.before = event.after;
                let key = (event.space, event.vm, event.generation, event.index);
                let before = event.before;
                // Aliased source/destination ports address the same original
                // register. Keep this alternative history locally coherent:
                // every read sees the replacement value, and a later write
                // reads that value before producing its unchanged result.
                for alias in events.iter_mut().flatten() {
                    if (alias.space, alias.vm, alias.generation, alias.index) == key {
                        alias.before = before;
                        if !alias.write {
                            alias.after = before;
                        }
                    }
                }
            }
        }
    }
    // Candidate initializers only make the test history internally valid.
    // They have no execution/State authority and are not a proving adapter.
    let mut first = std::collections::BTreeMap::new();
    for event in events.iter().flatten() {
        first
            .entry((event.space as u8, event.generation, event.index))
            .or_insert_with(|| {
                Some(Event {
                    space: event.space,
                    vm: event.vm,
                    generation: event.generation,
                    index: event.index,
                    write: true,
                    before: [0; 16],
                    after: event.before,
                    before_private: 0,
                    after_private: event.before_private,
                })
            });
    }
    let prefix = first.len();
    let mut all = first.into_values().collect::<Vec<_>>();
    all.extend(events);
    for (slot, fields) in original.fields.iter_mut().enumerate() {
        if fields[ENABLED] == F::ONE {
            fields[CLOCK] = F((prefix + slot) as u64);
        }
    }
    let bus = PublicPacketBus::new(all).unwrap();
    let columns = bus.columns();
    let challenges = permutation::Challenges::testing(
        E::canonical([2, 1, 0, 0]).unwrap(),
        E::canonical([7, 0, 1, 0]).unwrap(),
    );
    let aux = permutation::columns(
        &columns[NOTE_COPY_WIDTH_V1 + ORDERED..NOTE_COPY_WIDTH_V1 + SORTED],
        &columns[NOTE_COPY_WIDTH_V1 + SORTED
            ..NOTE_COPY_WIDTH_V1 + super::super::super::super::PREVIOUS],
        &challenges,
        bus.size(),
    )
    .unwrap();
    let rows = (0..bus.size())
        .map(|i| {
            core::array::from_fn::<_, ROW_WIDTH, _>(|column| {
                columns[NOTE_COPY_WIDTH_V1 + column][i]
            })
        })
        .collect::<Vec<_>>();
    let aux_rows = (0..bus.size())
        .map(|i| aux.iter().map(|column| column[i]).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let schedule = private_history::Schedule::new(bus.trace_log2, 1).unwrap();
    let fixed = (0..bus.size())
        .map(|i| schedule.fixed(i).unwrap())
        .collect::<Vec<_>>();
    // Confirm each alternative history is locally consistent before testing
    // the original producer join; no malformed sorted history is the oracle.
    let mut residues = Vec::new();
    for i in 0..bus.size() {
        let next = (i + 1) % bus.size();
        let producer = rows[i][ORDERED..SORTED].try_into().unwrap();
        private_history::append_residues(
            &mut residues,
            &rows[i],
            &rows[next],
            &aux_rows[i],
            &aux_rows[next],
            &fixed[i],
            producer,
            &challenges,
        );
        assert!(
            residues.iter().all(|value| *value == F::ZERO),
            "candidate history must be valid at row {i} for substitution {substitution:?}"
        );
        residues.clear();
    }
    let windows = core::array::from_fn(|index| {
        let i = prefix * super::super::super::super::PHASES + index;
        HistoryRow {
            current: &rows[i],
            next: &rows[i + 1],
            aux: &aux_rows[i],
            next_aux: &aux_rows[i + 1],
            fixed: &fixed[i],
        }
    });
    original.append_history_residues(&mut residues, &windows, &challenges);
    residues.iter().all(|value| *value == F::ZERO)
}
