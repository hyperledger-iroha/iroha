# Generated instruction record identities

`instruction_record_generated_identity_frames.json` contains immutable captures
for all 292 current `isi!` declarations: 283 nongeneric records and 39 concrete
instantiations of the nine generic declarations. Its 322 type rows preserve 357
populated values and 1,428 complete root, vector, option and map frames.

The fixture SHA-256 is
`7e69371c0072539ff3d85952169da3e4185aa66c66580967a33ce697112d95ac`.
Names and directional hashes come from actual compiler captures before adding
the independent identity declarations; no physical paths were guessed.

The original instruction capture has SHA-256
`b4e2a3f771b616193f790effa010b6a5cbdf08fb74a5540065ded01a3cebb973`.
An additional capture populated its 51 missing records; its SHA-256 is
`deb5fb4c228c1bc8860b31bd0f910f0b51042788a1f8b3843f67d444592db4c2`.
That capture passed all root and container roundtrips using the default test
stack before the declarations were applied. Its 990-file source hash manifest
is `3b680f86c86d20338e8a8933a95cd7b3d1d15729d86e73bd700f7f393899aca7`.
The create-only temporary capture writer was removed afterwards.

Musubi, KAGEMUSHA, private-settlement and Exact12 values reuse the existing
typed synthetic fixture producers. Opaque byte artifacts test preservation;
they do not attest production qualification or ledger admission.

The permanent tests decode every captured root, compare its value after a
roundtrip, and require exact re-encoding of all four frame forms. They compare
both existing directional hashes to the independent identity contract. Generic
argument tests also pin the compiler-observed marker names; `NftId` and `RoleId`
were captured as generic arguments, without separate pre-declaration root hashes.

The capture is development evidence, not a sealed release candidate. Active
codec cutover, physical model moves and complete candidate qualification remain
separate work in [the identity design](../../../../specs/norito_schema_identity.md).
