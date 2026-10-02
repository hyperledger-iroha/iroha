//! Randomised stream-layer soak: arbitrary sizes, join points, loss and lane subsets.
#![allow(clippy::cast_possible_truncation)]

use iroha_petal::lanes::{Lane, decode_lane};
use iroha_petal::prng::Xorshift32;
use iroha_petal::stream::{
    AssemblerLimits, StreamAssembler, StreamEncoder, is_beacon_frame, parse_atom_lane, parse_d_lane,
};

fn feed(assembler: &mut StreamAssembler, encoder: &StreamEncoder, frame: u16, lanes: &[Lane]) {
    let (p, k, d) = encoder.words(frame);
    for lane in lanes {
        let word = match lane {
            Lane::P => &p,
            Lane::K => &k,
            Lane::D => &d,
        };
        let data = decode_lane(*lane, word, &[]).expect("clean lane decodes");
        match lane {
            Lane::D => assembler.push_d_lane(&parse_d_lane(&data).expect("d lane")),
            _ => assembler.push_atoms(&parse_atom_lane(*lane, &data).expect("atoms")),
        }
    }
}

#[test]
fn random_streams_always_complete_and_never_deliver_wrong_data() {
    let mut rng = Xorshift32::new(0xC0FF_EE11);
    for trial in 0..400 {
        // sizes cover K = 1, a few atoms, and a few hundred atoms
        let len = match trial % 8 {
            0 => 1 + (rng.next_u32() % 16) as usize,
            1 => 17 + (rng.next_u32() % 100) as usize,
            _ => 1 + (rng.next_u32() % 3_000) as usize,
        };
        let payload: Vec<u8> = (0..len).map(|_| rng.next_byte()).collect();
        let kind = rng.next_byte();
        let encoder = StreamEncoder::new(&payload, kind).expect("encoder");
        let loss_percent = rng.next_u32() % 70;
        let lanes: &[Lane] = match rng.next_u32() % 5 {
            0 => &[Lane::P],
            1 => &[Lane::D, Lane::P],
            2 => &[Lane::K, Lane::D],
            _ => &[Lane::P, Lane::K, Lane::D],
        };
        let mut assembler = StreamAssembler::new(AssemblerLimits::default());
        let mut frame = (rng.next_u32() & 0xFFFF) as u16;
        let mut shown = 0u32;
        while !assembler.progress().complete {
            if rng.next_u32() % 100 >= loss_percent {
                // only lane D carries the beacon, so a receiver must read it at least
                // once; offer it on beacon frames whatever else is readable
                let mut readable = lanes.to_vec();
                if is_beacon_frame(frame) && !readable.contains(&Lane::D) {
                    readable.push(Lane::D);
                }
                feed(&mut assembler, &encoder, frame, &readable);
            }
            frame = frame.wrapping_add(1);
            shown += 1;
            let budget = 40 + 8 * (len as u32 / 13 + 2) * 100 / (100 - loss_percent);
            assert!(
                shown < budget,
                "trial {trial}: {len} bytes, loss {loss_percent} %, lanes {lanes:?} exceeded {budget} frames"
            );
        }
        let done = assembler.take_completed().expect("complete");
        assert_eq!(done.payload, payload, "trial {trial}");
        assert_eq!(done.meta.kind, kind);
    }
}

#[test]
fn counter_wraparound_keeps_atom_ids_consistent() {
    let payload: Vec<u8> = (0..2_000u32).map(|i| (i * 7 + 3) as u8).collect();
    let encoder = StreamEncoder::new(&payload, 1).expect("encoder");
    let mut assembler = StreamAssembler::new(AssemblerLimits::default());
    // start a few frames before the 16-bit counter wraps and run across it
    let mut frame = 65_530u16;
    for _ in 0..200 {
        feed(
            &mut assembler,
            &encoder,
            frame,
            &[Lane::P, Lane::K, Lane::D],
        );
        frame = frame.wrapping_add(1);
        if assembler.progress().complete {
            break;
        }
    }
    assert_eq!(
        assembler.take_completed().expect("complete").payload,
        payload
    );
}
