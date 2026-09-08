Touring S1 / ツーリング S1 is an isolated equal-performance class. Every entrant uses the same compiled vector; per-car multipliers and stacking are absent. This candidate has no registered proof profile, kit-ownership authorization, browser exporter or live settlement. Stock sources and the active registry are separate. A future Touring profile must commit these rules, its model, both arithmetic graphs and their dependency closure.

| Constant | Touring S1 | Stock |
| --- | ---: | ---: |
| Ticks/second | 30 | 30 |
| Maximum ticks / laps / input batch | 5400 / 3 / 6 | 5400 / 3 / 6 |
| Throttle acceleration | 44 | 40 |
| Normal / boosted speed | 2640 / 3300 | 2400 / 3000 |
| Brake / coast decrement | 100 / 12 | 100 / 12 |
| Normal / drift steering force | 18 / 28 | 18 / 28 |
| Boost capacity / cost / recharge | 1000 / 25 / 4 | 1000 / 25 / 4 |
| Off-road / contact speed loss | 90 / 120 | 90 / 120 |

The executable specification is `rules.rs`, `reference.rs` and `environment.rs`. Distances are millimetres and division truncates toward zero. Validate state and six-bit controls before each step, then work on a staged copy so rejection cannot partially mutate the caller. The two-column grid starts at x ±1800 and progress `-floor(slot/2)*4000`, with zero velocities and full boost. Practice permits one car; multiplayer requires two to eight.

Boost applies when requested and pre-tick energy is at least 25; otherwise recharge. Brake takes precedence over throttle. Clamp accelerated speed to the effective limit. Steering is right-minus-left times the normal/drift force. Rain applies `trunc(steer*3/4)`; overlapping oil then applies `trunc(steer/2)`. Update lateral velocity with `trunc((vx+steer)*15/16)` in rain or oil, otherwise `trunc((vx+steer)*7/8)`. Clamp to ±280 in rain, otherwise ±320.

Sample curvature and the track object using Euclidean wrapped **old** progress. Add velocity, `trunc(curvature*speed/120)` and `trunc(wind*speed/2400)` to x, clamp to ±9000, and subtract the off-road speed penalty strictly outside ±6000. Advance progress by the resulting speed. Apply swept tree/sign impact next, then ascending `(left_slot,right_slot)` car contacts, and finish checks last. Impact never reverses already-advanced progress.

Each of twelve cells has one object at `floor((2*cell+1)*length/24)`, alternating negative/positive sides. Tree/sign/oil x magnitudes are 7400/5600/1800. Oil includes longitudinal ±9000 and lateral ±1400 edges. Trees require lateral distance strictly below 1400; signs below 1300. A crossing from strictly before the center to at/after it triggers once. Trees lose 600 speed and push 1800 away from the object (exact-x ties left), clamped to ±9000. Signs lose 260 speed. `environment.rs` freezes all three object tables.

Rain cycles every 300 ticks: Tokyo `0010`, Harbor `0110`, Sakura `0001`. Wind cycles every 90 ticks through `[0,1,2,1,0,-1,-2,-1]`, scaled by 4/16/8. Fixed columns derive weather from public track/tick. At Touring speed 3300, combined curve/wind force is bounded by ±126: `floor(3*3300/120)+floor(32*3300/2400)`.

Car contacts require `abs(dp)<3600` and `abs(dx)<1800`. Each pushes by `ceil((1800-abs(dx))/2)`; lower x goes left, with lower slot breaking exact-x ties. Deduct speed independently for each visited contact. After all contacts, clamp completed three-lap progress and record the first finish tick. Finished and DNF cars are frozen and have no collision body or environment effects. A later DNF preserves physical finish history but removes prize eligibility.

DNF sets are nonempty, unique and ascending, with strictly increasing ticks and no repeated key removal. Terminal boundaries are tick 5400 or batch boundaries with every car finished/forfeited or fewer than two remaining multiplayer keys. Eligible earliest finishers tie; otherwise a sole survivor wins; otherwise eligible maximum distance ties at timeout. All-forfeit races refund. Prefix results expose no winners. A final removal can terminate without driving the following reserved batch.

Validated states bound progress to −12000..three-lap-finish, speed to 0..3300, velocity to ±320, energy to 0..1000 and x to ±15300 (movement clamp plus seven pushes of at most 900). Finish/removal timestamps must agree with progress and cannot be in the future. These domain checks do not prove reachability.

The whole-tick and staged arithmetic implement the same rules. The staged schedule uses `1+7*n+n*(n-1)/2` rows/tick: boundary, environment, grip, drive, curve, movement, impact, ordered contacts, finish. The staged relation checks all seven canonical car-state ranges at each car's finish row and the four scratch ranges at their environment/curve producer rows. All 20 packed inputs are equated to the initialized carry; Boolean running flags and every intervening transition remain constrained. The finite integer induction in [RANGE_SCHEDULE.md](RANGE_SCHEDULE.md) establishes each consumer's bounds without repeating two complete car checks in every microcycle. Tests cover environment edges, weather boundaries, all controls, invalid ranges, frozen cars, exact reference parity, coherent forged consumer rows, padding and full-duration traces. Arithmetic agreement is not STARK soundness, entitlement verification or release qualification; those remain required before activation.
