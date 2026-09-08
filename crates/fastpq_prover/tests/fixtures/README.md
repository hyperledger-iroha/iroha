# FASTPQ final V1 fixtures

`v1_raw_transcript_64.bin` is the sole binary proof regression. It binds the
six-lane commitment/transcript construction and the exact 32-byte canonical
Fp4 codec. The mixed Transfer/MetaSet batch is raw cryptographic test material;
it does not claim state-transition validity or production qualification.

The encoded proof is 1,831,049 bytes. Its transfer rows use the canonical
`FastpqBalanceKeyV1` frames. The test retains the production verifier's 512 KiB
rejection, then applies a fixed 2 MiB diagnostic cap without relaxing the
cryptographic checks. The AXT 1 MiB
payload cap also excludes this raw fixture. The retired balanced proof files
have no current consumers and are removed.

Regenerate from the current prover, then run without the update variable to
decode, verify, and reproduce the exact bytes:

```sh
FASTPQ_UPDATE_FIXTURES=1 scripts/cargo_fast.sh --stable-local-metadata --no-incremental -- test -p fastpq_prover --features dev-tools --test transcript_replay --config 'profile.test.package.fastpq_isi.opt-level=3'
scripts/cargo_fast.sh --stable-local-metadata --no-incremental -- test -p fastpq_prover --features dev-tools --test transcript_replay --config 'profile.test.package.fastpq_isi.opt-level=3'
```

`--no-incremental` is required by the installed sccache wrapper on the local
build lane and reuses the existing target directory.

The command-local optimization affects only the primitive implementation's
build profile. It does not change parameters, byte encodings, or verification
policy. `ordering_hash.json` separately pins the public BLAKE2b ordering hash.
