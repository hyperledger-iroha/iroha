//! Column census of the RaceV1 circuit by gadget label.
use iroha_execution_stark::games::race_v1::{self, Track};
use std::collections::BTreeMap;

fn main() {
    let circuit = race_v1::circuit(Track::NeonTokyo, 8);
    let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
    for column in &circuit.columns {
        let label = column
            .label
            .split(": ")
            .skip(1)
            .collect::<Vec<_>>()
            .join(": ");
        let kind = label.split(':').next().unwrap_or("").trim().to_owned();
        *by_kind.entry(kind).or_default() += 1;
    }
    let mut rows: Vec<_> = by_kind.into_iter().collect();
    rows.sort_by_key(|(_, c)| std::cmp::Reverse(*c));
    for (kind, count) in rows.iter().take(40) {
        println!("{count:>5}  {kind}");
    }
}
