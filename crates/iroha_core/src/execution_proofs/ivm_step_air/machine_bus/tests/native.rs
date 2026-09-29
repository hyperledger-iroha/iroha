//! Native envelope, exact-degree and proof transcript binding controls.

use super::*;
use crate::execution_proofs::stark::{
    aggregate_stark::{
        AggregateProofLayoutV1, AggregateTraceGroupLayoutV1,
        maximum_encoded_proof_with_deep_bytes_v1,
    },
    proof_managed_note_stark::{
        degree_audit::measured_maximum_affine_degree_v1,
        prove_proof_managed_note_stark_v1_with_rng, verify_proof_managed_note_stark_v1,
    },
};
use rand::{SeedableRng, rngs::StdRng};

#[test]
fn exact_packet_layout_stays_in_native_envelope_and_every_phase_is_degree_four() {
    let bus = PublicPacketBus::new(vec![None; MAX_PACKETS]).unwrap();
    assert_eq!(bus.trace_log2, MAX_LOG);
    assert!(PublicPacketBus::new(vec![None; MAX_PACKETS + 1]).is_err());
    assert_eq!(bus.base_width_v1(), 262);
    assert_eq!(NOTE_COPY_AUX_WIDTH_V1 + bus.profile_aux_width_v1(), 134);
    let protocol = bus.protocol_v1();
    protocol.validate().unwrap();
    let layout = AggregateProofLayoutV1::new(
        protocol.parameters,
        vec![AggregateTraceGroupLayoutV1 {
            native_trace_log2: MAX_LOG,
            segment_instances: 1,
            base_width: bus.base_width_v1(),
            aux_width: NOTE_COPY_AUX_WIDTH_V1 + bus.profile_aux_width_v1(),
        }],
    )
    .unwrap();
    assert_eq!(
        maximum_encoded_proof_with_deep_bytes_v1(protocol.parameters, &layout).unwrap(),
        2_472_320,
    );
    assert_eq!(protocol.maximum_constraint_degree, 4);
    assert_eq!(protocol.parameters.query_count, 136);
    assert_eq!(protocol.parameters.maximum_proof_bytes, 4 * 1024 * 1024);
    assert_eq!(
        measured_maximum_affine_degree_v1(
            [0x73; 32],
            [
                ROW_WIDTH,
                ROW_WIDTH,
                permutation::WIDTH,
                permutation::WIDTH,
                FIXED_WIDTH
            ],
            3,
            4,
            |row, next, aux, next_aux, fixed| Ok::<_, Error>(residues(
                row,
                next,
                aux,
                next_aux,
                fixed,
                &challenges()
            )),
        ),
        4
    );
}

#[test]
fn native_packet_proof_binds_every_public_event_and_explicit_entropy() {
    let bus = fixture();
    let columns = bus.columns();
    let proof = prove_proof_managed_note_stark_v1_with_rng(
        &bus,
        &columns,
        &mut StdRng::from_seed([0x71; 32]),
    )
    .unwrap();
    let repeated = prove_proof_managed_note_stark_v1_with_rng(
        &bus,
        &columns,
        &mut StdRng::from_seed([0x71; 32]),
    )
    .unwrap();
    assert_eq!(proof, repeated);
    let randomized = prove_proof_managed_note_stark_v1_with_rng(
        &bus,
        &columns,
        &mut StdRng::from_seed([0x72; 32]),
    )
    .unwrap();
    assert_ne!(proof, randomized);
    verify_proof_managed_note_stark_v1(&bus, &proof).unwrap();
    verify_proof_managed_note_stark_v1(&bus, &randomized).unwrap();
    assert!(proof.len() <= 2_472_320);
    for change in 0..11 {
        let mut events = bus.events.clone();
        if change < 9 {
            let event = events[0].as_mut().unwrap();
            match change {
                0 => event.space = Space::Owner,
                1 => event.vm ^= 1,
                2 => event.generation ^= 1,
                3 => event.index ^= 1,
                4 => event.write = !event.write,
                5 => event.before[15] ^= 1,
                6 => event.after[15] ^= 1,
                7 => event.before_private ^= 1,
                _ => event.after_private ^= 1,
            }
        } else if change == 9 {
            events[0] = None;
        } else {
            // Move an active packet into an existing ordered hole. Reconstruct
            // its clock and the verifier-owned total through the constructor.
            events.swap(0, 1);
        }
        let changed = PublicPacketBus::new(events).unwrap();
        assert_ne!(changed.digest().unwrap(), bus.digest().unwrap());
        assert!(
            verify_proof_managed_note_stark_v1(&changed, &proof).is_err(),
            "unbound public event component {change}"
        );
    }
    let mut forged = columns;
    forged[NOTE_COPY_WIDTH_V1 + SOURCES][0] = F(2);
    assert!(
        prove_proof_managed_note_stark_v1_with_rng(
            &bus,
            &forged,
            &mut StdRng::from_seed([0x71; 32]),
        )
        .is_err()
    );
}
