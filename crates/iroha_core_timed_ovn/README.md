# Parliament timed-OVN verification

This crate owns state-free timed-OVN lifecycle replay, public casting archives,
and threshold-BLS TLE key-session and release verification. It depends on the
protocol model and cryptographic primitives without depending on validator Core.

`iroha_core` owns committed-state reads, the private-field authorization
capabilities, signer admission, and release-share custody. A decoded or assembled
public archive must pass replay validation and never grants signing authority.
Explicit Norito schema names identify the existing protocol types independently
of their Rust module location.

Run the verification suite with
`cargo iroha-fast -- test -p iroha_core_timed_ovn --lib`. The opt-in `test-utils`
feature supplies negative fixtures to Core and native bridge tests; shipping
release feature guards reject it.
