//! The same immutable environment compiled into bounded integer equations.

use super::{
    environment as env,
    integer_air::{IntegerAirV1, Value},
};
use iroha_data_model::execution_proofs::RaceTrackV1;

fn object(a: &mut IntegerAirV1, track: RaceTrackV1, progress: Value) -> [Value; 4] {
    let length = track.length_mm();
    let shifted = a.add_constant(progress, length);
    let (_, wrapped) = a.divmod_unsigned(shifted, length);
    let scaled = a.mul_constant(wrapped, 12);
    let (cell, _) = a.divmod_unsigned(scaled, length);
    let columns = a.lookup_columns(
        cell,
        &[
            &env::centers(track),
            &env::kinds(track),
            &env::laterals(track),
        ],
    );
    [wrapped, columns[0], columns[1], columns[2]]
}
pub(super) fn lateral_velocity(
    a: &mut IntegerAirV1,
    track: RaceTrackV1,
    rain: Value,
    progress: Value,
    lateral: Value,
    velocity: Value,
    steer: Value,
) -> [Value; 2] {
    let geometry = geometry(a, track, progress);
    lateral_velocity_from_geometry(a, rain, geometry, lateral, velocity, steer)
}
pub(super) fn geometry(a: &mut IntegerAirV1, track: RaceTrackV1, progress: Value) -> [Value; 3] {
    let [wrapped, center, kind, x] = object(a, track, progress);
    [a.sub(wrapped, center), kind, x]
}
pub(super) fn lateral_velocity_from_geometry(
    a: &mut IntegerAirV1,
    rain: Value,
    [relative, kind, x]: [Value; 3],
    lateral: Value,
    velocity: Value,
    steer: Value,
) -> [Value; 2] {
    let oil = a.less(Value::constant(1), kind);
    let distance = a.absolute(relative);
    let inside_length = a.less(distance, Value::constant(env::OIL_HALF_LENGTH + 1));
    let distance = a.sub(lateral, x);
    let distance = a.absolute(distance);
    let inside_width = a.less(distance, Value::constant(env::OIL_HALF_WIDTH + 1));
    let inside = a.and(inside_length, inside_width);
    let oil = a.and(oil, inside);
    let rain_steer = a.mul_constant(steer, 3);
    let rain_steer = a.divide(rain_steer, 4);
    let steer = a.select(rain, rain_steer, steer);
    let oil_steer = a.divide(steer, 2);
    let steer = a.select(oil, oil_steer, steer);
    let steered = a.add(velocity, steer);
    let slippery = a.or(rain, oil);
    let dry = a.mul_constant(steered, 7);
    let dry = a.divide(dry, 8);
    let wet = a.mul_constant(steered, 15);
    let wet = a.divide(wet, 16);
    let damped = a.select(slippery, wet, dry);
    let lower = a.select(rain, Value::constant(-280), Value::constant(-320));
    let upper = a.select(rain, Value::constant(280), Value::constant(320));
    let positive = a.maximum(damped, lower);
    [a.minimum(positive, upper), oil]
}
pub(super) fn impact(
    a: &mut IntegerAirV1,
    track: RaceTrackV1,
    old_progress: Value,
    progress: Value,
    lateral: Value,
    speed: Value,
) -> [Value; 4] {
    let geometry = geometry(a, track, old_progress);
    let distance = a.sub(progress, old_progress);
    impact_from_geometry(a, geometry, distance, lateral, speed)
}
pub(super) fn impact_from_geometry(
    a: &mut IntegerAirV1,
    [relative, kind, x]: [Value; 3],
    distance: Value,
    lateral: Value,
    speed: Value,
) -> [Value; 4] {
    let solid = a.less(kind, Value::constant(2));
    let tree = a.less(kind, Value::constant(1));
    let before = a.less(relative, Value::constant(0));
    let advanced = a.add(relative, distance);
    let before_after = a.less(advanced, Value::constant(0));
    let crossed = a.not(before_after);
    let crossed = a.and(before, crossed);
    let radius = a.select(tree, Value::constant(1400), Value::constant(1300));
    let delta = a.sub(lateral, x);
    let distance = a.absolute(delta);
    let touching = a.less(distance, radius);
    let contact = a.and(solid, crossed);
    let contact = a.and(contact, touching);
    let tree_contact = a.and(contact, tree);
    let to_right = a.less(x, lateral);
    let push = a.select(to_right, Value::constant(1800), Value::constant(-1800));
    let pushed = a.add(x, push);
    let pushed = a.clamp(pushed, -9000, 9000);
    let lateral = a.select(tree_contact, pushed, lateral);
    let loss = a.select(tree, Value::constant(600), Value::constant(260));
    let slowed = a.sub(speed, loss);
    let slowed = a.maximum(slowed, Value::constant(0));
    [lateral, a.select(contact, slowed, speed), contact, kind]
}

#[cfg(test)]
mod tests {
    use super::super::integer_air::{Source, field};
    use super::*;

    fn integer(value: Value, row: &[i64]) -> i64 {
        match value.source {
            Source::Column(index) => row[index],
            Source::Constant(value) => value,
            _ => panic!("witness output"),
        }
    }
    #[test]
    fn every_track_object_edge_and_wet_control_matches_native_with_tamper_rejection() {
        for track in [
            RaceTrackV1::NeonTokyo,
            RaceTrackV1::Harbor,
            RaceTrackV1::Sakura,
        ] {
            let mut a = IntegerAirV1::default();
            let progress = a.input(0, track.length_mm());
            let lateral = a.input(-9000, 9000);
            let velocity = a.input(-320, 320);
            let steer = a.input(-28, 28);
            let rain = a.input(0, 1);
            let distance = a.input(0, 3000);
            let speed = a.input(0, 3000);
            let [vx, oil] =
                lateral_velocity(&mut a, track, rain, progress, lateral, velocity, steer);
            let next_progress = a.add(progress, distance);
            let impact = impact(&mut a, track, progress, next_progress, lateral, speed);
            for cell in 0..12 {
                for offset in [-9001, -9000, -1, 0, 9000, 9001] {
                    for lateral_offset in [-1401, -1400, 0, 1300, 1400, 1401] {
                        let x =
                            (env::laterals(track)[cell] + lateral_offset).clamp(-9000, 9000) as i32;
                        let p = env::centers(track)[cell] + offset;
                        for wet in [0, 1] {
                            let row =
                                a.witness(&[p, i64::from(x), -319, -28, wet, 1500, 1500], &[]);
                            let tick = env::rain_pattern(track)
                                .iter()
                                .position(|rain| *rain == wet)
                                .unwrap() as u32
                                * env::WEATHER_TICKS;
                            assert_eq!(
                                integer(vx, &row),
                                i64::from(env::lateral_velocity(track, tick, p, x, -319, -28))
                            );
                            assert_eq!(integer(oil, &row), i64::from(env::oil(track, p, x)));
                            let expected = env::impact(track, p, p + 1500, x, 1500);
                            assert_eq!(
                                [integer(impact[0], &row), integer(impact[1], &row)],
                                [i64::from(expected.0), i64::from(expected.1)]
                            );
                            let fields = row.iter().copied().map(field).collect::<Vec<_>>();
                            assert!(
                                a.residues(&fields, &fields, &[])
                                    .iter()
                                    .all(|r| *r == field(0))
                            );
                            for output in [vx, oil, impact[0], impact[1], impact[2]] {
                                let Source::Column(index) = output.source else {
                                    panic!("computed output")
                                };
                                let mut forged = fields.clone();
                                forged[index] = forged[index].add(field(1));
                                assert!(
                                    a.residues(&forged, &forged, &[])
                                        .iter()
                                        .any(|r| *r != field(0))
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
