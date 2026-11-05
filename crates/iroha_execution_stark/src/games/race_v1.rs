//! SORA CARS Stock racing (`RaceV1`) on the generic engine.
//!
//! [`step`] is a line-by-line port of `iroha_core_privacy::execution_proofs::race::step_race_v1`
//! and its `environment` helpers; [`circuit`] expresses the same tick with the circuit builder.
//! The prover cross-checks every tick of the circuit against [`step`], and the adapter tests in
//! `iroha_core_privacy` compare [`step`] with the node's own simulation.
//!
//! Circuit layout (one row per tick): seven registers per car (progress, lateral, speed, lateral
//! velocity, boost energy, finished flag, finish tick). Player controls, removals and weather are
//! public columns. Removals are public, so they are not registers.

use crate::circuit::{Builder, Circuit, Expr, OneHot};

/// Ticks in the longest race.
pub const MAX_TICKS: u32 = 5_400;
/// Cars in the largest grid.
pub const MAX_PLAYERS: usize = 8;
/// Laps per race.
pub const LAPS: i64 = 3;
/// Valid control bits: throttle, brake, left, right, drift, boost.
pub const CONTROL_MASK: u16 = 0x3f;
/// Trace height: 5,400 ticks plus the final state fit in 2^13 rows.
pub const LOG_ROWS: u32 = 13;

const TREE: i64 = 0;
const SIGN: i64 = 1;
const OIL: i64 = 2;
const WIND_PATTERN: [i64; 8] = [0, 1, 2, 1, 0, -1, -2, -1];
const OIL_HALF_LENGTH: i64 = 9_000;
const OIL_HALF_WIDTH: i64 = 1_400;
const WEATHER_TICKS: u32 = 300;
const WIND_TICKS: u32 = 90;

/// The three Stock tracks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Track {
    /// Neon Tokyo.
    NeonTokyo,
    /// Harbor.
    Harbor,
    /// Sakura.
    Sakura,
}

impl Track {
    /// Lap length in millimetres.
    pub const fn length(self) -> i64 {
        match self {
            Self::NeonTokyo => 2_000_000,
            Self::Harbor => 2_400_000,
            Self::Sakura => 1_800_000,
        }
    }

    /// Twelve curvature cells.
    pub const fn curvature(self) -> [i64; 12] {
        match self {
            Self::NeonTokyo => [0, 1, 2, 1, 0, -1, -2, -1, 0, 2, -2, 0],
            Self::Harbor => [0, -2, -2, 0, 1, 3, 1, 0, -1, -3, -1, 0],
            Self::Sakura => [0, 1, 1, 0, -2, -1, 0, 2, 3, 1, -2, 0],
        }
    }

    /// Object kind per cell.
    pub const fn kinds(self) -> [i64; 12] {
        match self {
            Self::NeonTokyo => [0, 1, 0, 2, 0, 1, 2, 0, 1, 0, 2, 1],
            Self::Harbor => [1, 2, 0, 1, 0, 2, 1, 0, 2, 1, 0, 2],
            Self::Sakura => [0, 0, 2, 1, 0, 2, 0, 1, 0, 2, 0, 1],
        }
    }

    /// Object lateral position per cell.
    pub fn laterals(self) -> [i64; 12] {
        let kinds = self.kinds();
        std::array::from_fn(|cell| {
            let magnitude = match kinds[cell] {
                TREE => 7_400,
                SIGN => 5_600,
                _ => 1_800,
            };
            if cell % 2 == 0 { -magnitude } else { magnitude }
        })
    }

    /// Object centre (wrapped progress) per cell.
    pub fn centers(self) -> [i64; 12] {
        std::array::from_fn(|cell| {
            (2 * i64::try_from(cell).expect("12 cells") + 1) * self.length() / 24
        })
    }

    const fn rain_pattern(self) -> [i64; 4] {
        match self {
            Self::NeonTokyo => [0, 0, 1, 0],
            Self::Harbor => [0, 1, 1, 0],
            Self::Sakura => [0, 0, 0, 1],
        }
    }

    const fn wind_strength(self) -> i64 {
        match self {
            Self::NeonTokyo => 4,
            Self::Harbor => 16,
            Self::Sakura => 8,
        }
    }

    /// `(rain, wind)` at a tick.
    pub fn weather(self, tick: u32) -> (i64, i64) {
        (
            self.rain_pattern()[((tick / WEATHER_TICKS) % 4) as usize],
            WIND_PATTERN[((tick / WIND_TICKS) % 8) as usize] * self.wind_strength(),
        )
    }

    /// Stable name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::NeonTokyo => "NeonTokyo",
            Self::Harbor => "Harbor",
            Self::Sakura => "Sakura",
        }
    }
}

/// One car's integer state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Car {
    /// Unwrapped progress, including the negative grid offset.
    pub progress: i64,
    /// Road-relative lateral position.
    pub lateral: i64,
    /// Forward displacement per tick.
    pub speed: i64,
    /// Lateral displacement per tick before curvature force.
    pub lateral_velocity: i64,
    /// Boost reserve, 0..=1000.
    pub energy: i64,
    /// Tick at which the car crossed the finish.
    pub finish_tick: Option<u32>,
    /// Tick at which consensus removed the car.
    pub dnf_tick: Option<u32>,
}

/// The whole race state.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RaceState {
    /// Completed ticks.
    pub tick: u32,
    /// Track.
    pub track: Track,
    /// Cars in slot order.
    pub cars: Vec<Car>,
}

/// A public replay.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Replay {
    /// Track.
    pub track: Track,
    /// Number of cars (1..=8).
    pub players: usize,
    /// One control mask per car per tick.
    pub frames: Vec<Vec<u16>>,
    /// `(tick, ascending slots)` removals, ascending by tick.
    pub dnf_events: Vec<(u32, Vec<u8>)>,
}

/// Invalid replay or state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RaceError {
    /// Player count outside 1..=8.
    #[error("race requires between one and eight players")]
    PlayerCount,
    /// Wrong number of controls in a frame.
    #[error("race input frame does not match the participant count")]
    ControlCount,
    /// Reserved control bits set.
    #[error("race controls contain reserved bits")]
    ControlBits,
    /// Removal events out of order or out of range.
    #[error("race frame tick is not consecutive")]
    TickSequence,
    /// The race was already terminal.
    #[error("race is already terminal")]
    Terminal,
    /// State outside the bounded domain.
    #[error("race state is outside the bounded simulation domain")]
    StateBounds,
}

/// The prescribed starting grid.
///
/// # Errors
/// Rejects player counts outside 1..=8.
pub fn initial_state(track: Track, players: usize) -> Result<RaceState, RaceError> {
    if !(1..=MAX_PLAYERS).contains(&players) {
        return Err(RaceError::PlayerCount);
    }
    Ok(RaceState {
        tick: 0,
        track,
        cars: (0..players)
            .map(|slot| Car {
                progress: -i64::try_from(slot / 2).expect("8 slots") * 4_000,
                lateral: if slot % 2 == 0 { -1_800 } else { 1_800 },
                speed: 0,
                lateral_velocity: 0,
                energy: 1_000,
                finish_tick: None,
                dnf_tick: None,
            })
            .collect(),
    })
}

fn object(track: Track, progress: i64) -> (i64, i64, i64, i64) {
    let wrapped = progress.rem_euclid(track.length());
    let cell = (wrapped * 12 / track.length()) as usize;
    (
        wrapped,
        track.centers()[cell],
        track.kinds()[cell],
        track.laterals()[cell],
    )
}

fn oil(track: Track, progress: i64, lateral: i64) -> bool {
    let (wrapped, center, kind, x) = object(track, progress);
    kind == OIL
        && (wrapped - center).abs() <= OIL_HALF_LENGTH
        && (lateral - x).abs() <= OIL_HALF_WIDTH
}

fn lateral_velocity(
    track: Track,
    tick: u32,
    progress: i64,
    lateral: i64,
    velocity: i64,
    steer: i64,
) -> i64 {
    let rain = track.weather(tick).0 != 0;
    let oil = oil(track, progress, lateral);
    let steer = if rain { steer * 3 / 4 } else { steer };
    let steer = if oil { steer / 2 } else { steer };
    let velocity = if rain || oil {
        (velocity + steer) * 15 / 16
    } else {
        (velocity + steer) * 7 / 8
    };
    let limit = if rain { 280 } else { 320 };
    velocity.clamp(-limit, limit)
}

fn impact(track: Track, old_progress: i64, progress: i64, lateral: i64, speed: i64) -> (i64, i64) {
    let (wrapped, center, kind, x) = object(track, old_progress);
    let radius = if kind == TREE { 1_400 } else { 1_300 };
    if kind == OIL
        || wrapped >= center
        || wrapped + (progress - old_progress) < center
        || (lateral - x).abs() >= radius
    {
        return (lateral, speed);
    }
    if kind == TREE {
        let pushed = x + if lateral > x { 1_800 } else { -1_800 };
        (pushed.clamp(-9_000, 9_000), (speed - 600).max(0))
    } else {
        (lateral, (speed - 260).max(0))
    }
}

fn validate(state: &RaceState) -> Result<(), RaceError> {
    if state.cars.is_empty() || state.cars.len() > MAX_PLAYERS {
        return Err(RaceError::PlayerCount);
    }
    let finish = state.track.length() * LAPS;
    if state.tick > MAX_TICKS
        || state.cars.iter().any(|car| {
            !(-12_000..=finish).contains(&car.progress)
                || !(-16_200..=16_200).contains(&car.lateral)
                || !(0..=3_000).contains(&car.speed)
                || !(-320..=320).contains(&car.lateral_velocity)
                || !(0..=1_000).contains(&car.energy)
                || car
                    .finish_tick
                    .is_some_and(|tick| tick == 0 || tick > state.tick)
                || (car.finish_tick.is_some() && car.progress != finish)
                || car.dnf_tick.is_some_and(|tick| tick > state.tick)
        })
    {
        return Err(RaceError::StateBounds);
    }
    Ok(())
}

/// Whether the race may not take another step.
pub fn is_terminal(state: &RaceState) -> bool {
    state.tick == MAX_TICKS
        || (state.tick.is_multiple_of(6)
            && (state
                .cars
                .iter()
                .all(|car| car.finish_tick.is_some() || car.dnf_tick.is_some())
                || state.cars.len() >= 2
                    && state
                        .cars
                        .iter()
                        .filter(|car| car.dnf_tick.is_none())
                        .count()
                        < 2))
}

/// Advances one exact tick (port of `step_race_v1`).
///
/// # Errors
/// Rejects invalid states, terminal races, wrong control counts and reserved bits.
pub fn step(state: &mut RaceState, controls: &[u16]) -> Result<(), RaceError> {
    validate(state)?;
    if is_terminal(state) {
        return Err(RaceError::Terminal);
    }
    if controls.len() != state.cars.len() {
        return Err(RaceError::ControlCount);
    }
    if controls.iter().any(|c| c & !CONTROL_MASK != 0) {
        return Err(RaceError::ControlBits);
    }
    let track = state.track;
    let length = track.length();
    let curves = track.curvature();
    let (_, wind) = track.weather(state.tick);
    for (car, control) in state.cars.iter_mut().zip(controls) {
        if car.finish_tick.is_some() || car.dnf_tick.is_some() {
            continue;
        }
        let boost = control & 32 != 0 && car.energy >= 25;
        car.energy = if boost {
            car.energy - 25
        } else {
            (car.energy + 4).min(1_000)
        };
        let maximum_speed = if boost { 3_000 } else { 2_400 };
        let acceleration = if control & 2 != 0 {
            -100
        } else if control & 1 != 0 {
            40
        } else {
            -12
        };
        car.speed = (car.speed + acceleration).clamp(0, maximum_speed);
        let steer = i64::from(control & 8 != 0) - i64::from(control & 4 != 0);
        let steering_force = if control & 16 != 0 { 28 } else { 18 };
        car.lateral_velocity = lateral_velocity(
            track,
            state.tick,
            car.progress,
            car.lateral,
            car.lateral_velocity,
            steer * steering_force,
        );
        let segment = (car.progress.rem_euclid(length) * 12 / length) as usize;
        let curvature_force = curves[segment] * car.speed / 120 + wind * car.speed / 2400;
        car.lateral = (car.lateral + car.lateral_velocity + curvature_force).clamp(-9_000, 9_000);
        if car.lateral.abs() > 6_000 {
            car.speed = (car.speed - 90).max(0);
        }
        let old_progress = car.progress;
        car.progress += car.speed;
        (car.lateral, car.speed) =
            impact(track, old_progress, car.progress, car.lateral, car.speed);
    }
    for left_slot in 0..state.cars.len() {
        for right_slot in left_slot + 1..state.cars.len() {
            let (left, right) = state.cars.split_at_mut(right_slot);
            let a = &mut left[left_slot];
            let b = &mut right[0];
            if a.finish_tick.is_some()
                || b.finish_tick.is_some()
                || a.dnf_tick.is_some()
                || b.dnf_tick.is_some()
                || (a.progress - b.progress).abs() >= 3_600
                || (a.lateral - b.lateral).abs() >= 1_800
            {
                continue;
            }
            let push = (1_800 - (a.lateral - b.lateral).abs() + 1) / 2;
            if a.lateral <= b.lateral {
                a.lateral -= push;
                b.lateral += push;
            } else {
                a.lateral += push;
                b.lateral -= push;
            }
            a.speed = (a.speed - 120).max(0);
            b.speed = (b.speed - 120).max(0);
        }
    }
    state.tick += 1;
    let finish = length * LAPS;
    for car in &mut state.cars {
        if car.finish_tick.is_none() && car.dnf_tick.is_none() && car.progress >= finish {
            car.progress = finish;
            car.finish_tick = Some(state.tick);
        }
    }
    Ok(())
}

/// Applies consensus removals before a tick (port of `apply_race_dnf_v1`).
///
/// # Errors
/// Rejects empty, unordered, out-of-range or repeated removals.
pub fn apply_dnf(state: &mut RaceState, slots: &[u8]) -> Result<(), RaceError> {
    validate(state)?;
    if slots.is_empty()
        || slots.windows(2).any(|pair| pair[0] >= pair[1])
        || slots.iter().any(|slot| {
            usize::from(*slot) >= state.cars.len()
                || state.cars[usize::from(*slot)].dnf_tick.is_some()
        })
    {
        return Err(RaceError::StateBounds);
    }
    for slot in slots {
        state.cars[usize::from(*slot)].dnf_tick = Some(state.tick);
    }
    Ok(())
}

/// Replays the race, returning the state before every tick and the state after the last one (with
/// a final removal applied, as `replay_race_v1` does).
///
/// # Errors
/// Rejects any replay the node's simulation rejects.
pub fn replay_states(replay: &Replay) -> Result<Vec<RaceState>, RaceError> {
    if replay.frames.len() > MAX_TICKS as usize {
        return Err(RaceError::Terminal);
    }
    let mut state = initial_state(replay.track, replay.players)?;
    if replay
        .dnf_events
        .windows(2)
        .any(|pair| pair[0].0 >= pair[1].0)
        || replay
            .dnf_events
            .iter()
            .any(|(tick, _)| *tick as usize > replay.frames.len())
    {
        return Err(RaceError::TickSequence);
    }
    let mut events = replay.dnf_events.iter().peekable();
    let mut states = Vec::with_capacity(replay.frames.len() + 1);
    for frame in &replay.frames {
        if events.peek().is_some_and(|(tick, _)| *tick == state.tick) {
            apply_dnf(&mut state, &events.next().expect("peeked").1)?;
        }
        states.push(state.clone());
        step(&mut state, frame)?;
    }
    if let Some((_, slots)) = events.next() {
        apply_dnf(&mut state, slots)?;
    }
    states.push(state);
    Ok(states)
}

/// Registers per car.
pub const REGISTERS_PER_CAR: usize = 7;

/// Flattens a state into register values (removals are public and excluded).
pub fn registers(state: &RaceState) -> Vec<i64> {
    state
        .cars
        .iter()
        .flat_map(|car| {
            [
                car.progress,
                car.lateral,
                car.speed,
                car.lateral_velocity,
                car.energy,
                i64::from(car.finish_tick.is_some()),
                i64::from(car.finish_tick.unwrap_or(0)),
            ]
        })
        .collect()
}

/// Rebuilds a state from register values plus the public removal schedule.
pub fn state_from_registers(
    track: Track,
    tick: u32,
    values: &[i64],
    dnf_ticks: &[Option<u32>],
) -> RaceState {
    RaceState {
        tick,
        track,
        cars: values
            .chunks_exact(REGISTERS_PER_CAR)
            .zip(dnf_ticks)
            .map(|(v, dnf)| Car {
                progress: v[0],
                lateral: v[1],
                speed: v[2],
                lateral_velocity: v[3],
                energy: v[4],
                finish_tick: (v[5] == 1).then(|| u32::try_from(v[6]).expect("bounded")),
                dnf_tick: *dnf,
            })
            .collect(),
    }
}

struct CarExprs {
    progress: Expr,
    lateral: Expr,
    speed: Expr,
    velocity: Expr,
    energy: Expr,
    finished: Expr,
    finish_tick: Expr,
}

struct CarPublic {
    dnf: Expr,
    boost_request: Expr,
    acceleration: Expr,
    steer: Expr,
    steer_half: Expr,
}

/// Per-car results of the motion phase, before contacts.
struct Moved {
    live: Expr,
    progress: Expr,
    lateral: Expr,
    speed: Expr,
    velocity: Expr,
    energy: Expr,
}

fn cell_tables(track: Track) -> ([i64; 12], [i64; 12], [i64; 12], [i64; 12]) {
    let laterals = track.laterals();
    let push_left = std::array::from_fn(|c| (laterals[c] - 1_800).clamp(-9_000, 9_000));
    let push_right = std::array::from_fn(|c| (laterals[c] + 1_800).clamp(-9_000, 9_000));
    let radius = std::array::from_fn(|c| {
        if track.kinds()[c] == TREE {
            1_400
        } else {
            1_300
        }
    });
    let penalty = std::array::from_fn(|c| match track.kinds()[c] {
        TREE => 600,
        SIGN => 260,
        _ => 0,
    });
    (push_left, push_right, radius, penalty)
}

#[allow(clippy::too_many_arguments)]
fn motion(
    b: &mut Builder,
    track: Track,
    car: &CarExprs,
    input: &CarPublic,
    rain: &Expr,
    wind: &Expr,
    wind_negative: &Expr,
    slot: usize,
) -> Moved {
    let length = track.length();
    let label = |s: &str| format!("car {slot}: {s}");
    // live = (1 - finished)(1 - removed)
    let live = b.and(&car.finished.not(), &input.dnf.not(), &label("live"));

    // Boost and energy.
    let can_boost = b.ge(&car.energy, &Expr::constant(25), &label("energy >= 25"));
    let boost = b.and(&input.boost_request, &can_boost, &label("boost"));
    let recharged = b.min(
        &car.energy.offset(4),
        &Expr::constant(1_000),
        &label("recharge"),
    );
    let energy = b.select(
        &boost,
        &car.energy.offset(-25),
        &recharged,
        &label("energy"),
    );
    let energy = b.narrow(&energy, 0, 1_000, &label("energy floor"));

    // Speed.
    let maximum = Expr::constant(2_400).add(&boost.scale(600));
    let accelerated = car.speed.add(&input.acceleration);
    let speed1 = b.clamp(
        &accelerated,
        &Expr::constant(0),
        &maximum,
        &label("speed clamp"),
    );

    // Position decomposition: lap = floor(p / L), cell = floor(12 * wrapped / L).
    // The lap index is pinned by the cell's remainder check: 0 <= 12*wrapped - L*cell < L with
    // cell in 0..=11 forces 0 <= wrapped < L, hence lap = floor(p / L).
    let lap = b.one_hot_quotient_unchecked(&car.progress, length, -1, 3, &label("lap"));
    let wrapped = car.progress.sub(&lap.index().scale(length));
    let cell = b.one_hot_quotient(&wrapped.scale(12), length, 0, 11, &label("cell"));
    let wrapped = b.assume_interval(&wrapped, 0, length - 1, &label("wrapped"));
    let center = cell.select_table(&track.centers());
    let object_x = cell.select_table(&track.laterals());
    let curvature = cell.select_table(&track.curvature());
    let kinds = track.kinds();
    let is_oil = cell.indicator(|c| kinds[c as usize] == OIL);
    let is_tree = cell.indicator(|c| kinds[c as usize] == TREE);
    let is_solid = is_oil.not();
    let curvature_negative = cell.indicator(|c| track.curvature()[c as usize] < 0);
    let (push_left, push_right, radius, penalty) = cell_tables(track);

    // Oil at the old position and lateral.
    let along = wrapped.sub(&center);
    let near_long = within(b, &along, OIL_HALF_LENGTH, &label("oil along"));
    let across = car.lateral.sub(&object_x);
    let near_across = within(b, &across, OIL_HALF_WIDTH, &label("oil across"));
    let near = b.and(&near_long, &near_across, &label("oil near"));
    let on_oil = b.and(&near, &is_oil, &label("on oil"));

    // Lateral velocity.
    let steer = b.select(
        &on_oil,
        &input.steer_half,
        &input.steer,
        &label("oil steer"),
    );
    let slippery = b.or(rain, &on_oil, &label("rain or oil"));
    let sum = car.velocity.add(&steer);
    let scaled = b.mul(&sum, &Expr::constant(14).add(&slippery), &label("damping"));
    let damped = b.div_trunc(&scaled, 16, &label("damping /16"));
    let limit = Expr::constant(320).sub(&rain.scale(40));
    let velocity = b.clamp(&damped, &limit.neg(), &limit, &label("velocity clamp"));

    // Curvature and wind.
    let curve_term = b.mul(&curvature, &speed1, &label("curve * speed"));
    let curve_force =
        b.div_trunc_with_sign(&curve_term, 120, &curvature_negative, &label("curve /120"));
    let wind_term = b.mul(wind, &speed1, &label("wind * speed"));
    let wind_force = b.div_trunc_with_sign(&wind_term, 2_400, wind_negative, &label("wind /2400"));
    let drift = car
        .lateral
        .add(&velocity)
        .add(&curve_force)
        .add(&wind_force);
    let lateral2 = b.clamp(
        &drift,
        &Expr::constant(-9_000),
        &Expr::constant(9_000),
        &label("lateral clamp"),
    );

    // Off-road slowdown.
    let off_right = b.gt(&lateral2, &Expr::constant(6_000), &label("off right"));
    let off_left = b.lt(&lateral2, &Expr::constant(-6_000), &label("off left"));
    let off_road = b.assume_boolean(&off_right.add(&off_left), &label("off road"));
    let slowed = b.max(
        &speed1.offset(-90),
        &Expr::constant(0),
        &label("off-road slow"),
    );
    let speed2 = b.select(&off_road, &slowed, &speed1, &label("speed after off-road"));
    let progress = car.progress.add(&speed2);

    // Solid objects: swept crossing of the old cell's centre.
    let before_center = b.lt(&wrapped, &center, &label("before centre"));
    let reaches_center = b.ge(&wrapped.add(&speed2), &center, &label("reaches centre"));
    let side_offset = lateral2.sub(&object_x);
    let radius_expr = cell.select_table(&radius);
    let close_right = b.lt(&side_offset, &radius_expr, &label("radius right"));
    let close_left = b.lt(&radius_expr.neg(), &side_offset, &label("radius left"));
    let crossing = b.and(&before_center, &reaches_center, &label("crossing"));
    let close = b.and(&close_right, &close_left, &label("close"));
    let crossing_close = b.and(&crossing, &close, &label("crossing close"));
    let hit = b.and(&crossing_close, &is_solid, &label("hit"));
    let tree_hit = b.and(&hit, &is_tree, &label("tree hit"));
    let right_side = b.lt(&object_x, &lateral2, &label("right of object"));
    let pushed = b.select(
        &right_side,
        &cell.select_table(&push_right),
        &cell.select_table(&push_left),
        &label("tree push"),
    );
    let lateral3 = b.select(
        &tree_hit,
        &pushed,
        &lateral2,
        &label("lateral after impact"),
    );
    let penalized = b.max(
        &speed2.sub(&cell.select_table(&penalty)),
        &Expr::constant(0),
        &label("impact slow"),
    );
    let speed3 = b.select(&hit, &penalized, &speed2, &label("speed after impact"));
    let _ = slot;
    Moved {
        live,
        progress,
        lateral: lateral3,
        speed: speed3,
        velocity,
        energy,
    }
}

/// `|value| <= bound` as a Boolean (two comparisons).
fn within(b: &mut Builder, value: &Expr, bound: i64, label: &str) -> Expr {
    let below = b.lt(value, &Expr::constant(bound + 1), &format!("{label} upper"));
    let above = b.lt(
        &Expr::constant(-bound - 1),
        value,
        &format!("{label} lower"),
    );
    b.and(&below, &above, label)
}

/// The canonical RaceV1 circuit for one track and grid size (capacity 5,400 ticks).
///
/// # Panics
/// Panics if `players` is outside 1..=8.
pub fn circuit(track: Track, players: usize) -> Circuit {
    circuit_with_capacity(track, players, LOG_ROWS)
}

/// The RaceV1 circuit with `2^log_rows` rows (`2^log_rows - 1` ticks). Smaller capacities have
/// their own circuit identity; they exist for tests and short exhibition races.
///
/// # Panics
/// Panics if `players` is outside 1..=8 or `log_rows` outside 8..=13.
pub fn circuit_with_capacity(track: Track, players: usize, log_rows: u32) -> Circuit {
    assert!((1..=MAX_PLAYERS).contains(&players));
    assert!((8..=LOG_ROWS).contains(&log_rows));
    let name = if log_rows == LOG_ROWS {
        format!("sora-cars RaceV1 stock {} x{players} v1", track.name())
    } else {
        format!(
            "sora-cars RaceV1 stock {} x{players} v1 capacity 2^{log_rows}",
            track.name()
        )
    };
    let mut b = Builder::new(name, log_rows);
    b.set_lookups_per_helper(4);
    let finish = track.length() * LAPS;
    let rain = b.public("rain", 0, 1);
    let wind = b.public("wind", -32, 32);
    let wind_negative = b.public("wind < 0", 0, 1);
    let terminal_check = b.public("six-tick terminal check", 0, 1);
    let inputs: Vec<CarPublic> = (0..players)
        .map(|slot| CarPublic {
            dnf: b.public(format!("car {slot}: removed"), 0, 1),
            boost_request: b.public(format!("car {slot}: boost"), 0, 1),
            acceleration: b.public(format!("car {slot}: acceleration"), -100, 40),
            steer: b.public(format!("car {slot}: steer"), -28, 28),
            steer_half: b.public(format!("car {slot}: steer / 2"), -14, 14),
        })
        .collect();
    let cars: Vec<CarExprs> = (0..players)
        .map(|slot| CarExprs {
            progress: b.register(format!("car {slot}: progress"), -12_000, finish),
            lateral: b.register(format!("car {slot}: lateral"), -16_200, 16_200),
            speed: b.register(format!("car {slot}: speed"), 0, 3_000),
            velocity: b.register(format!("car {slot}: lateral velocity"), -320, 320),
            energy: b.register(format!("car {slot}: energy"), 0, 1_000),
            finished: b.register(format!("car {slot}: finished"), 0, 1),
            finish_tick: b.register(format!("car {slot}: finish tick"), 0, (1 << log_rows) - 1),
        })
        .collect();

    // Six-tick termination: some car must still be live when a batch boundary is stepped.
    let moved: Vec<Moved> = cars
        .iter()
        .zip(&inputs)
        .enumerate()
        .map(|(slot, (car, input))| {
            motion(
                &mut b,
                track,
                car,
                input,
                &rain,
                &wind,
                &wind_negative,
                slot,
            )
        })
        .collect();
    let live_count = moved
        .iter()
        .fold(Expr::constant(0), |acc, m| acc.add(&m.live));
    b.assert_nonzero_when(
        &terminal_check,
        &live_count,
        "terminal: a car must still be racing",
    );

    // Ordered contacts; slowing is deferred: max(speed - 120 k, 0) equals k saturating steps.
    let mut laterals: Vec<Expr> = moved.iter().map(|m| m.lateral.clone()).collect();
    let mut contacts: Vec<Expr> = vec![Expr::constant(0); players];
    for i in 0..players {
        for j in i + 1..players {
            let label = |s: &str| format!("pair {i}-{j}: {s}");
            let both_live = b.and(&moved[i].live, &moved[j].live, &label("both live"));
            let gap = moved[i].progress.sub(&moved[j].progress);
            let near_long = b.lt(&gap, &Expr::constant(3_600), &label("gap upper"));
            let near_long2 = b.lt(&Expr::constant(-3_600), &gap, &label("gap lower"));
            let separation = laterals[i].sub(&laterals[j]);
            let left_of = b.lt(&separation, &Expr::constant(1), &label("left of"));
            let distance = b.select(
                &left_of,
                &separation.neg(),
                &separation,
                &label("lateral distance"),
            );
            let distance = b.assume_interval(
                &distance,
                0,
                i64::try_from(distance.hi()).expect("bounded"),
                &label("distance"),
            );
            let near_side = b.lt(&distance, &Expr::constant(1_800), &label("side"));
            let near_both = b.and(&near_long, &near_long2, &label("near"));
            let near_all = b.and(&near_both, &near_side, &label("near all"));
            let contact = b.and(&near_all, &both_live, &label("contact"));
            let overlap = b.mul(
                &contact,
                &Expr::constant(1_801).sub(&distance),
                &label("overlap"),
            );
            let overlap = b.narrow(&overlap, 0, 1_801, &label("overlap range"));
            let push = b.div_floor(&overlap, 2, &label("push"));
            let direction = Expr::constant(1).sub(&left_of.scale(2));
            let shift = b.mul(&push, &direction, &label("shift"));
            laterals[i] = laterals[i].add(&shift);
            laterals[j] = laterals[j].sub(&shift);
            contacts[i] = contacts[i].add(&contact);
            contacts[j] = contacts[j].add(&contact);
        }
    }

    for (slot, ((car, m), (lateral, count))) in cars
        .iter()
        .zip(&moved)
        .zip(laterals.iter().zip(&contacts))
        .enumerate()
    {
        let label = |s: &str| format!("car {slot}: {s}");
        let speed = b.max(
            &m.speed.sub(&count.scale(120)),
            &Expr::constant(0),
            &label("contact slow"),
        );
        let below_finish = b.lt(&m.progress, &Expr::constant(finish), &label("below finish"));
        let capped = b.select(
            &below_finish,
            &m.progress,
            &Expr::constant(finish),
            &label("finish cap"),
        );
        let capped = b.assume_interval(&capped, -12_000, finish, &label("capped progress"));
        // Padding rows never finish a car: their hypothetical step would stamp tick >= 8192.
        let active = b.active();
        let live_and_active = b.and(&m.live, &active, &label("live and active"));
        let crossed = b.and(&live_and_active, &below_finish.not(), &label("crossed"));
        let next_progress = b.select(&m.live, &capped, &car.progress, &label("progress"));
        let next_lateral = b.select(&m.live, lateral, &car.lateral, &label("lateral"));
        let next_lateral = b.narrow(&next_lateral, -16_200, 16_200, &label("lateral bound"));
        let next_speed = b.select(&m.live, &speed, &car.speed, &label("speed"));
        let next_velocity = b.select(&m.live, &m.velocity, &car.velocity, &label("velocity"));
        let next_energy = b.select(&m.live, &m.energy, &car.energy, &label("energy"));
        let next_finished = b.or(&car.finished, &crossed, &label("finished"));
        let next_tick = b.step_index().offset(1);
        let finish_tick = b.mul(&crossed, &next_tick, &label("finish tick"));
        let next_finish_tick = car.finish_tick.add(&finish_tick);
        let next_finish_tick = b.narrow(
            &next_finish_tick,
            0,
            (1 << log_rows) - 1,
            &label("finish tick bound"),
        );
        b.set_next(&car.progress, &next_progress);
        b.set_next(&car.lateral, &next_lateral);
        b.set_next(&car.speed, &next_speed);
        b.set_next(&car.velocity, &next_velocity);
        b.set_next(&car.energy, &next_energy);
        b.set_next(&car.finished, &next_finished);
        b.set_next(&car.finish_tick, &next_finish_tick);
    }
    b.finish()
}

/// The game's public columns for a replay (rows beyond the replay use zero controls).
///
/// # Errors
/// Rejects malformed replays.
pub fn public_columns(replay: &Replay) -> Result<Vec<Vec<i64>>, RaceError> {
    public_columns_with_capacity(replay, LOG_ROWS)
}

/// [`public_columns`] for a circuit of `2^log_rows` rows.
///
/// # Errors
/// Rejects malformed replays and replays longer than the capacity.
pub fn public_columns_with_capacity(
    replay: &Replay,
    log_rows: u32,
) -> Result<Vec<Vec<i64>>, RaceError> {
    let rows = 1usize << log_rows;
    if replay.frames.len() >= rows {
        return Err(RaceError::Terminal);
    }
    let players = replay.players;
    if !(1..=MAX_PLAYERS).contains(&players) {
        return Err(RaceError::PlayerCount);
    }
    if replay.frames.iter().any(|f| f.len() != players) {
        return Err(RaceError::ControlCount);
    }
    let steps = replay.frames.len();
    let mut dnf_from = vec![None; players];
    for (tick, slots) in &replay.dnf_events {
        for slot in slots {
            let entry = dnf_from
                .get_mut(usize::from(*slot))
                .ok_or(RaceError::StateBounds)?;
            if entry.is_none() {
                *entry = Some(*tick);
            }
        }
    }
    let mut columns = vec![vec![0i64; rows]; 4 + 5 * players];
    for row in 0..rows {
        let tick = row as u32;
        let (rain, wind) = replay.track.weather(tick);
        columns[0][row] = rain;
        columns[1][row] = wind;
        columns[2][row] = i64::from(wind < 0);
        columns[3][row] = i64::from(row < steps && tick.is_multiple_of(6));
        for slot in 0..players {
            let control = if row < steps {
                replay.frames[row][slot]
            } else {
                0
            };
            let base = 4 + 5 * slot;
            columns[base][row] = i64::from(dnf_from[slot].is_some_and(|t| t <= tick));
            columns[base + 1][row] = i64::from(control & 32 != 0);
            columns[base + 2][row] = if control & 2 != 0 {
                -100
            } else if control & 1 != 0 {
                40
            } else {
                -12
            };
            let steer = i64::from(control & 8 != 0) - i64::from(control & 4 != 0);
            let force = if control & 16 != 0 { 28 } else { 18 };
            let raw = steer * force;
            let steer = if rain != 0 { raw * 3 / 4 } else { raw };
            columns[base + 3][row] = steer;
            columns[base + 4][row] = steer / 2;
        }
    }
    Ok(columns)
}

/// Register vectors before every row (`steps + 1` entries), built from the native replay.
///
/// # Errors
/// Rejects replays the native simulation rejects.
pub fn register_states(replay: &Replay) -> Result<Vec<Vec<i64>>, RaceError> {
    let states = replay_states(replay)?;
    Ok(states.iter().map(registers).collect())
}

/// Helper used by [`circuit`]: re-export of the builder's one-hot type.
pub type CellSelector = OneHot;

/// Checks everything about a replay that does not need the simulation: grid size, frame shapes and
/// control bits, `T <= 5400`, removal events (ascending ticks `<= T`, nonempty ascending in-range
/// slots, no slot removed twice), and the public half of the six-tick terminal rule (with two or
/// more cars, at least two must remain unremoved at every six-tick boundary that is stepped). The
/// proof covers the rest: the integer simulation and the "every car finished or removed" rule.
///
/// # Errors
/// Returns the native error the node's simulation would raise for the same defect.
pub fn validate_replay(replay: &Replay) -> Result<(), RaceError> {
    if !(1..=MAX_PLAYERS).contains(&replay.players) {
        return Err(RaceError::PlayerCount);
    }
    let steps = replay.frames.len();
    if steps > MAX_TICKS as usize {
        return Err(RaceError::Terminal);
    }
    for frame in &replay.frames {
        if frame.len() != replay.players {
            return Err(RaceError::ControlCount);
        }
        if frame.iter().any(|c| c & !CONTROL_MASK != 0) {
            return Err(RaceError::ControlBits);
        }
    }
    if replay
        .dnf_events
        .windows(2)
        .any(|pair| pair[0].0 >= pair[1].0)
        || replay
            .dnf_events
            .iter()
            .any(|(tick, _)| *tick as usize > steps)
    {
        return Err(RaceError::TickSequence);
    }
    let mut removed_at = vec![None; replay.players];
    for (tick, slots) in &replay.dnf_events {
        if slots.is_empty() || slots.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(RaceError::StateBounds);
        }
        for slot in slots {
            let entry = removed_at
                .get_mut(usize::from(*slot))
                .ok_or(RaceError::StateBounds)?;
            if entry.is_some() {
                return Err(RaceError::StateBounds);
            }
            *entry = Some(*tick);
        }
    }
    if replay.players >= 2 {
        for tick in (0..steps as u32).step_by(6) {
            let remaining = removed_at
                .iter()
                .filter(|r| r.is_none_or(|t| t > tick))
                .count();
            if remaining < 2 {
                return Err(RaceError::Terminal);
            }
        }
    }
    Ok(())
}

/// Removal ticks per slot from a validated replay.
pub fn removal_ticks(replay: &Replay) -> Vec<Option<u32>> {
    let mut out = vec![None; replay.players];
    for (tick, slots) in &replay.dnf_events {
        for slot in slots {
            if let Some(entry) = out.get_mut(usize::from(*slot)) {
                entry.get_or_insert(*tick);
            }
        }
    }
    out
}

/// Proving or verification failure of a race.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RaceProofError {
    /// The replay is invalid.
    #[error(transparent)]
    Replay(#[from] RaceError),
    /// Proving failed.
    #[error(transparent)]
    Prove(#[from] crate::ProveError),
    /// Verification failed.
    #[error(transparent)]
    Verify(#[from] crate::VerifyError),
    /// The claimed final registers are malformed.
    #[error("claimed final state is malformed")]
    FinalState,
}

/// Proves a replay with the canonical circuit for its track and grid.
///
/// # Errors
/// Fails if the replay is invalid or proving fails.
pub fn prove_replay(
    replay: &Replay,
    params: &crate::Params,
) -> Result<(crate::Statement, crate::Proof), RaceProofError> {
    prove_replay_with_capacity(replay, params, LOG_ROWS)
}

/// [`prove_replay`] with the circuit of `2^log_rows` rows.
///
/// # Errors
/// Fails if the replay is invalid or proving fails.
pub fn prove_replay_with_capacity(
    replay: &Replay,
    params: &crate::Params,
    log_rows: u32,
) -> Result<(crate::Statement, crate::Proof), RaceProofError> {
    validate_replay(replay)?;
    let circuit = circuit_with_capacity(replay.track, replay.players, log_rows);
    let public = public_columns_with_capacity(replay, log_rows)?;
    let states = register_states(replay)?;
    let statement = statement(replay)?;
    let proof = crate::prove(&circuit, params, &statement, &public, &states)?;
    Ok((statement, proof))
}

/// Verifies a race proof against a public replay and the claimed final registers, returning the
/// proven final state with removal ticks (including removals at the last tick). The caller decides
/// whether the state must be terminal (settlement) and derives the outcome from it.
///
/// # Errors
/// Fails on an invalid replay, malformed claim or invalid proof.
pub fn verify_replay(
    replay: &Replay,
    final_registers: &[i64],
    proof: &crate::Proof,
    params: &crate::Params,
) -> Result<RaceState, RaceProofError> {
    verify_replay_with_capacity(replay, final_registers, proof, params, LOG_ROWS)
}

/// [`verify_replay`] against the circuit of `2^log_rows` rows.
///
/// # Errors
/// Fails on an invalid replay, malformed claim or invalid proof.
pub fn verify_replay_with_capacity(
    replay: &Replay,
    final_registers: &[i64],
    proof: &crate::Proof,
    params: &crate::Params,
    log_rows: u32,
) -> Result<RaceState, RaceProofError> {
    validate_replay(replay)?;
    if final_registers.len() != REGISTERS_PER_CAR * replay.players {
        return Err(RaceProofError::FinalState);
    }
    let circuit = circuit_with_capacity(replay.track, replay.players, log_rows);
    let initial = registers(&initial_state(replay.track, replay.players)?);
    let steps = replay.frames.len();
    let statement = crate::Statement {
        steps,
        initial,
        final_state: final_registers.to_vec(),
        checkpoint: None,
        binding: replay_binding(replay),
    };
    let public = public_columns_with_capacity(replay, log_rows)?;
    crate::verify(&circuit, params, &statement, &public, proof)?;
    let state = state_from_registers(
        replay.track,
        u32::try_from(steps).expect("validated"),
        final_registers,
        &removal_ticks(replay),
    );
    validate(&state)?;
    Ok(state)
}

/// Deterministic replay generator for tests and benchmarks: throttle-heavy drivers with random
/// steering, drift and boost, an optional removal, stopping when the race becomes terminal.
pub fn sample_replay(
    track: Track,
    players: usize,
    seed: u64,
    throttle_percent: u64,
    max_ticks: u32,
) -> Replay {
    let mut state = seed ^ 0x9e37_79b9_7f4a_7c15;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    let mut race = initial_state(track, players).expect("valid grid");
    let mut frames = Vec::new();
    let mut dnf_events = Vec::new();
    let removal = (players >= 3 && next() % 2 == 0)
        .then(|| ((next() % 3000) as u32, (next() % players as u64) as u8));
    while frames.len() < max_ticks as usize && !is_terminal(&race) {
        if let Some((tick, slot)) = removal {
            if tick == race.tick && race.cars[usize::from(slot)].dnf_tick.is_none() {
                apply_dnf(&mut race, &[slot]).expect("valid removal");
                dnf_events.push((tick, vec![slot]));
                if is_terminal(&race) {
                    break;
                }
            }
        }
        let controls: Vec<u16> = (0..players)
            .map(|_| {
                let r = next();
                let mut c = 0u16;
                if r % 100 < throttle_percent {
                    c |= 1;
                } else if r % 100 < throttle_percent + 5 {
                    c |= 2;
                }
                match (r >> 8) % 5 {
                    0 => c |= 4,
                    1 => c |= 8,
                    _ => {}
                }
                if (r >> 16) % 7 == 0 {
                    c |= 16;
                }
                if (r >> 24) % 9 == 0 {
                    c |= 32;
                }
                c
            })
            .collect();
        step(&mut race, &controls).expect("valid step");
        frames.push(controls);
    }
    Replay {
        track,
        players,
        frames,
        dnf_events,
    }
}

/// Builds the statement for a replay (the final state comes from the native simulation).
///
/// # Errors
/// Rejects replays the native simulation rejects.
pub fn statement(replay: &Replay) -> Result<crate::Statement, RaceError> {
    let states = register_states(replay)?;
    Ok(crate::Statement {
        steps: replay.frames.len(),
        initial: states[0].clone(),
        final_state: states.last().expect("nonempty").clone(),
        checkpoint: None,
        binding: replay_binding(replay),
    })
}

/// Canonical replay digest bound into the transcript.
pub fn replay_binding(replay: &Replay) -> Vec<u8> {
    let mut hasher = crate::hash::FramedHash::new(b"sora-cars RaceV1 standalone replay binding v1");
    hasher.update(replay.track.name().as_bytes());
    hasher.update(&(replay.players as u64).to_le_bytes());
    hasher.update(&(replay.frames.len() as u64).to_le_bytes());
    for frame in &replay.frames {
        hasher.update(&(frame.len() as u64).to_le_bytes());
        for control in frame {
            hasher.update(&control.to_le_bytes());
        }
    }
    hasher.update(&(replay.dnf_events.len() as u64).to_le_bytes());
    for (tick, slots) in &replay.dnf_events {
        hasher.update(&tick.to_le_bytes());
        hasher.update(&(slots.len() as u64).to_le_bytes());
        hasher.update(slots);
    }
    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Params, check_trace, prove, verify};

    const TRACKS: [Track; 3] = [Track::NeonTokyo, Track::Harbor, Track::Sakura];

    #[test]
    fn replay_binding_commits_each_field_and_frame_boundary() {
        let replay = Replay {
            track: Track::NeonTokyo,
            players: 2,
            frames: vec![vec![1, 2], vec![3, 4]],
            dnf_events: vec![],
        };
        let binding = replay_binding(&replay);
        assert_eq!(binding.len(), crate::hash::DIGEST_BYTES);
        let mut different_track = replay.clone();
        different_track.track = Track::Harbor;
        let mut different_players = replay.clone();
        different_players.players = 3;
        let mut different_control = replay.clone();
        different_control.frames[0][0] ^= 1;
        let mut different_event = replay.clone();
        different_event.dnf_events.push((1, vec![0]));
        let mut different_boundary = replay.clone();
        different_boundary.frames = vec![vec![1], vec![2, 3, 4]];
        for changed in [
            different_track,
            different_players,
            different_control,
            different_event,
            different_boundary,
        ] {
            assert_ne!(replay_binding(&changed), binding);
        }
    }

    #[test]
    fn native_port_runs_full_races() {
        for (i, track) in TRACKS.iter().enumerate() {
            for players in 1..=MAX_PLAYERS {
                let replay =
                    sample_replay(*track, players, (i * 10 + players) as u64, 92, MAX_TICKS);
                let states = replay_states(&replay).unwrap();
                assert_eq!(states.len(), replay.frames.len() + 1);
            }
        }
    }

    /// Unoptimized builds check short races on a 2^10-row circuit (identical constraints);
    /// optimized builds check full-length races on the canonical capacity.
    fn test_capacity() -> (u32, u32) {
        if cfg!(debug_assertions) {
            (10, 1_000)
        } else {
            (LOG_ROWS, MAX_TICKS)
        }
    }

    #[test]
    fn circuit_matches_native_simulation_on_every_tick() {
        let (log_rows, ticks) = test_capacity();
        let players_list: &[usize] = if cfg!(debug_assertions) {
            &[1, 8]
        } else {
            &[1, 2, 5, 8]
        };
        for (i, track) in TRACKS.iter().enumerate() {
            for &players in players_list {
                for (seed, throttle) in [(1u64, 95u64), (2, 55)] {
                    let replay = sample_replay(
                        *track,
                        players,
                        seed * 100 + (i * 10 + players) as u64,
                        throttle,
                        ticks,
                    );
                    let circuit = circuit_with_capacity(*track, players, log_rows);
                    let public = public_columns_with_capacity(&replay, log_rows).unwrap();
                    let states = register_states(&replay).unwrap();
                    let statement = statement(&replay).unwrap();
                    check_trace(&circuit, &statement, &public, &states)
                        .unwrap_or_else(|e| panic!("{} x{players} seed {seed}: {e}", track.name()));
                }
            }
        }
    }

    /// Recomputing a row's witness from a forged successor state cannot satisfy the transition:
    /// the first violated identity is at the forged step (or, for padding, the frozen row).
    #[test]
    fn forged_transitions_violate_the_circuit() {
        let log_rows = 10;
        let replay = sample_replay(Track::NeonTokyo, 4, 9, 95, 700);
        let circuit = circuit_with_capacity(Track::NeonTokyo, 4, log_rows);
        let public = public_columns_with_capacity(&replay, log_rows).unwrap();
        let states = register_states(&replay).unwrap();
        let statement = statement(&replay).unwrap();
        let steps = statement.steps;
        let engine_public = crate::trace::public_columns(&circuit, steps, None, &public).unwrap();
        let honest =
            crate::trace::generate_inner(&circuit, &engine_public, steps, &states, false).unwrap();
        crate::prover::check_main_trace(&circuit, &statement, &engine_public, &honest).unwrap();
        // Registers per car: progress, lateral, speed, velocity, energy, finished, finish tick.
        for (name, row, register, delta) in [
            ("progress", 40, 0, 1),
            ("lateral push", 300, 1, 900),
            ("speed", 120, 2, 1),
            ("velocity", 200, 3, -1),
            ("energy", 10, 4, -1),
            ("finish flag", 250, 5, 1),
            ("finish tick", 260, 6, 1),
            ("second car", 500, 7, 1),
        ] {
            let mut forged = states.clone();
            for state in &mut forged[row + 1..] {
                state[register] += delta;
            }
            let trace =
                crate::trace::generate_inner(&circuit, &engine_public, steps, &forged, false)
                    .unwrap();
            let mut forged_statement = statement.clone();
            forged_statement.final_state = forged[steps].clone();
            let error = crate::prover::check_main_trace(
                &circuit,
                &forged_statement,
                &engine_public,
                &trace,
            )
            .expect_err(name);
            assert!(error.starts_with(&format!("row {row}:")), "{name}: {error}");
        }
        // A frozen padding row that moves.
        let rows = circuit.rows();
        let mut padded: Vec<Vec<i64>> = (0..rows).map(|r| states[r.min(steps)].clone()).collect();
        for state in &mut padded[steps + 5..] {
            state[0] += 1;
        }
        let trace =
            crate::trace::generate_inner(&circuit, &engine_public, steps, &padded, false).unwrap();
        let error = crate::prover::check_main_trace(&circuit, &statement, &engine_public, &trace)
            .expect_err("padding movement");
        assert!(
            error.starts_with(&format!("row {}:", steps + 4)),
            "padding: {error}"
        );
    }

    #[test]
    fn proofs_stay_within_the_size_bound() {
        let params = Params::standard();
        for players in [2usize, 8] {
            let circuit = circuit(Track::Harbor, players);
            let bound = crate::max_proof_bytes(&circuit, &params).unwrap();
            assert!(bound < 3_250_000, "{players} cars: {bound}");
        }
        let replay = sample_replay(Track::Sakura, 3, 4, 92, 300);
        let (_, proof) = prove_replay_with_capacity(&replay, &Params::standard(), 10).unwrap();
        let circuit = circuit_with_capacity(Track::Sakura, 3, 10);
        assert!(
            proof.to_bytes().len()
                <= crate::max_proof_bytes(&circuit, &Params::standard()).unwrap()
        );
    }

    /// A car that times out just short of the line: padding rows must not "finish" it (a zero
    /// control step from the frozen final state would cross the line at a tick beyond the trace).
    #[test]
    fn timeout_just_short_of_the_finish_is_provable() {
        let track = Track::NeonTokyo;
        let finish = track.length() * LAPS;
        let mut race = initial_state(track, 1).unwrap();
        let mut frames = Vec::new();
        while race.tick < MAX_TICKS {
            let remaining = i64::from(MAX_TICKS - race.tick);
            let car = race.cars[0];
            let target = finish - 1_200 - car.progress;
            let wanted = target / remaining.max(1);
            let control: u16 = if car.speed < wanted { 1 } else { 2 };
            let steer = if car.lateral > 1_500 {
                4
            } else if car.lateral < -1_500 {
                8
            } else {
                0
            };
            let controls = vec![control | steer];
            step(&mut race, &controls).unwrap();
            frames.push(controls);
        }
        let car = race.cars[0];
        assert!(
            car.finish_tick.is_none(),
            "controller finished early: {car:?}"
        );
        let next_speed = (car.speed - 12).max(0);
        assert!(
            car.progress + next_speed >= finish,
            "not close enough to the line: {car:?}"
        );
        let replay = Replay {
            track,
            players: 1,
            frames,
            dnf_events: Vec::new(),
        };
        let circuit = circuit(track, 1);
        let public = public_columns(&replay).unwrap();
        let states = register_states(&replay).unwrap();
        let statement = statement(&replay).unwrap();
        let params = Params {
            queries: 20,
            grinding_bits: 2,
            ..Params::standard()
        };
        let proof = prove(&circuit, &params, &statement, &public, &states).unwrap();
        verify(&circuit, &params, &statement, &public, &proof).unwrap();
    }

    #[test]
    fn adapter_round_trip_and_public_terminal_rule() {
        let replay = sample_replay(Track::Sakura, 4, 3, 90, 600);
        let params = Params {
            queries: 20,
            grinding_bits: 2,
            ..Params::standard()
        };
        let (statement, proof) = prove_replay_with_capacity(&replay, &params, 10).unwrap();
        let state =
            verify_replay_with_capacity(&replay, &statement.final_state, &proof, &params, 10)
                .unwrap();
        // The canonical circuit is a different statement: the same proof does not verify there.
        assert!(verify_replay(&replay, &statement.final_state, &proof, &params).is_err());
        assert_eq!(state, replay_states(&replay).unwrap().pop().unwrap());

        // Codex's counterexample: two cars, slot 0 removed at tick 0, then a step. The native rules
        // reject it as terminal; the circuit alone would allow it, so the adapter must reject it.
        let bad = Replay {
            track: Track::NeonTokyo,
            players: 2,
            frames: vec![vec![0, 1]],
            dnf_events: vec![(0, vec![0])],
        };
        assert_eq!(validate_replay(&bad), Err(RaceError::Terminal));
        assert_eq!(replay_states(&bad), Err(RaceError::Terminal));
        let repeated = Replay {
            dnf_events: vec![(0, vec![2]), (3, vec![2])],
            ..sample_replay(Track::Harbor, 3, 1, 90, 10)
        };
        assert_eq!(validate_replay(&repeated), Err(RaceError::StateBounds));
    }

    #[test]
    fn short_race_proves_and_rejects_altered_inputs() {
        let replay = sample_replay(Track::Harbor, 3, 7, 90, 400);
        let circuit = circuit_with_capacity(Track::Harbor, 3, 10);
        let public = public_columns_with_capacity(&replay, 10).unwrap();
        let states = register_states(&replay).unwrap();
        let statement = statement(&replay).unwrap();
        let params = Params {
            queries: 30,
            grinding_bits: 4,
            ..Params::standard()
        };
        let proof = prove(&circuit, &params, &statement, &public, &states).unwrap();
        verify(&circuit, &params, &statement, &public, &proof).unwrap();

        let mut altered = replay.clone();
        altered.frames[100][1] ^= 8;
        let altered_public = public_columns_with_capacity(&altered, 10).unwrap();
        let mut altered_statement = statement.clone();
        altered_statement.binding = replay_binding(&altered);
        assert!(
            verify(
                &circuit,
                &params,
                &altered_statement,
                &altered_public,
                &proof
            )
            .is_err()
        );
        let mut wrong = statement.clone();
        wrong.final_state[0] += 1;
        assert!(verify(&circuit, &params, &wrong, &public, &proof).is_err());
    }
}
