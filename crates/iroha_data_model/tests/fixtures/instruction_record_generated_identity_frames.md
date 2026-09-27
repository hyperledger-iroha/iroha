# Generated instruction record identities

`instruction_record_generated_identity_frames.json` contains immutable captures
for the current instruction records and concrete generic instantiations. Its 323
type rows preserve 359 populated values and 1,436 complete root, vector, option
and map frames.

The fixture SHA-256 is
`f2b1a70d690730c6015f8e7cce2d12c6b7c72240f105985edb3a199fe06cae76`.
The inventory excludes the three unimplemented citizen-bond operations; all
other captured frame bytes remain unchanged except for the explicitly recorded
current-protocol recaptures below. Names and directional hashes come from actual compiler captures before adding
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

`SettleAtomic` was captured with the current native codec on 2026-09-12,
including exact root, vector, option and map decode/re-encode checks.

`RegisterPrivacyProtocolActivationV1` was recaptured on 2026-09-17 from the current
typed fixture and its real codec roundtrip helper. Its Proposed lifecycle contains
only `proposed_at_height`; the retired automatic activation-height field is absent.
All four frame forms were decoded, compared and exactly re-encoded by the generator.
That checkpoint changed one case while preserving its nominal and directional
identities and the strict retired-field rejection tests.

On 2026-09-23, the typed current-protocol printers recaptured the Exact12
qualification and staking rows, including two distinct signed peer rebindings
and the updated consensus evidence nested in penalty cancellation. The unchanged
inventory count and complete four-frame roundtrips remain mandatory; no old
decoder or identity fallback was introduced.
The opt-in `capture_current_privacy_activation_instruction_identity_frames` test
prints the actual capture to stdout and never writes the fixture itself.

`RegisterZkAsset` was recaptured on 2026-09-25 after removal of its retired
`vk_shield` field. A temporary Rust test constructed both the no-unshield and
unshield cases and printed the actual root, vector, option and map frames. The
fixture proposal changed only those two cases; the temporary test was removed.
The type's directional identity hashes and the other 320 rows are unchanged.

`RedeemKagemushaV1` and `TopUpKagemushaV1` were recaptured on 2026-09-25
after the terminal-body commitment and hardware credential binding changed.
The temporary typed maintenance test identified exactly these two changed
populated rows among the 51 missing-record producers. It supplied all four
current frame forms for each row; the row identities, other 319 rows, and
inventory counts are unchanged. The temporary test was removed after capture.

`RegisterIdentifierPolicy`, both `ClaimIdentifier` cases, and
`FinalizeElection` were recaptured on 2026-09-25 after first-release policy,
receipt and exact `u128` tally changes. The two Claim cases were matched by
their unchanged encoded account field, preserving original case order. Only
these three rows' four frame forms changed; all declared type identities and
the 321-row, 357-case inventory remain unchanged. The temporary typed capture
tests were removed afterwards.

`RegisterCapacityDeclaration` was recaptured on 2026-09-26 after its first-release
input became the sole canonical `declaration` byte frame. Its nominal and directional
identities are unchanged; the retired caller-authored registry projections are absent.
`InitializeSorafsProviderAdmissionV1` and `AssertSorafsPublicationV1` were added from
populated typed values in the same compiler run. The opt-in
`print_capacity_declaration_record_fixture_row` maintenance test encoded, decoded,
compared and exactly re-encoded all four frame forms for each of these three values
before printing the captures. Only the capacity row changed among the existing 321
rows; the two additions raise the inventory to 323 rows, 359 values and 1,436 frames.
The initial admission material is a network-independent signed-genesis template;
this codec fixture does not establish finalized admission or certify publication.
The complete `generated_record_identity_tests` filter passed on 2026-09-26:
325 tests passed, none failed and three explicit maintenance captures were ignored.
