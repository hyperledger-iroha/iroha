# Norito nested serialization — 29 September 2026

The original raw benchmark bundle referenced below is absent from this checkout.
Independent replay requires that bundle or a fresh measured run.

Counted children now use exact-length scopes on one Encoder. Previously, each
child added another Encoder and ExactLengthWriter, so nested bytes repeatedly
crossed ancestor adapters. A current 31-member Sumeragi proposal profile shows
that chain during encoding and canonical decoding's required re-encode. The
post-fix profile confirms that repeated per-byte ancestor forwarding disappears
while outer exact-length, checksum and byte-comparison validation remains.

The scoped destination retains actual length measurement, the smallest enclosing
bound, successful partial-write accounting, sticky overrun rejection and bound
restoration on unwind. The public arbitrary-writer exact-length helper still
validates actual bytes. Wire formats and canonical checksum validation are
unchanged; there is no compatibility path or new configuration.

## Scoped validation

The unchanged baseline passes 1,329 Norito tests. The candidate passes 1,360 in
both normal and release test profiles, including 31 new adversarial controls and
all 12 exact-field allocation controls. Each suite has one ignored snapshot-dump
helper. The ownership checker and all 11 Python mutation/registration tests
also pass. An initial baseline fixture failure caused by the runner's working
directory is retained alongside the corrected complete pass.

Twenty actual-record benchmark processes pass: two warmups per arm and eight
alternating pairs. Every iteration compares every output byte and round-trips
through the production decoder. All eleven exported canonical frames match
between arms. Both libraries use the same compiler, features and opt-level-3
release profile. These are local library experiments with retained source and
artifact identities, not distribution-release qualification.

| Record, canonical Vec output | Median paired encoding time ratio | Median paired decoding time ratio |
|---|---:|---:|
| 4-member view-zero proposal | 0.8506 | 0.9053 |
| 31-member view-zero proposal | 0.8336 | 0.8937 |
| 4-member per-vote safety frame | 0.8740 | 0.9171 |
| 31-member per-vote safety frame | 0.8840 | 0.9344 |

Ratios are candidate/baseline; below 1 means less time. All 22 selected encoding
case/mode medians improve. Tiny sync-request streaming decoding is about 0.5%
slower, and the earlier standalone shallow arbitrary-writer case retains an
11.6% regression. All outcomes remain reported. The host had concurrent builds,
so this is not a quiescent-host estimate or a general performance guarantee.

## Evidence and limits

`dist/paper-performance-20260929/RESULTS.md` contains every case, exact commands,
raw-result locations, source/image hashes, sampling failures and limitations.
The measured HEAD is `89efeb5f734e6782f818e1bf98c21870c7888e9b` plus the retained
working-tree source manifests. Each build interval is unchanged. The broader
cross-build capture also records an independently generated lane fixture; its
identity is retained rather than silently omitted.

Signature, witness and payload contents are deterministic codec fixtures. This
does not authenticate consensus, measure storage durability, or establish SORA
Nexus payment latency, throughput or participant scaling. The current happy-day
observer's signed RS16 availability-evidence path remains unintegrated and
rejects acceptance. A current authenticated happy-day measurement and the
five-second paper target remain open. Sequential restart experiments are not
part of this result.
