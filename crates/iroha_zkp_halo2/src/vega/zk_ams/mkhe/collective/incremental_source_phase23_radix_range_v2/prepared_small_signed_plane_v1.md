# Small signed source preparation

`prepare_next_small_signed_plane_v1` consumes the original radix owner only
after beta/m completion. The retained session admits 1,032 `SmallSigned`
commitments at physical inventory ordinals `25,112..26,144`, followed by 1,032
`SmallNegativeMagnitude` commitments at `26,144..27,176`. The corresponding
logical comparator/sign ordinals are `7,224..9,288`. The next inventory purpose
is `QMaskDigit`; this implementation does not produce that purpose.

The source is the already authenticated compact signed snapshot retained by
the original replay evidence. Its slot is `((record*3+role)*8+plane)`, with roles
`r,e0,e1`. Each plane's coefficient is `k=1024*local_block+i`; unlike D packing,
this order does not transpose coefficients and blocks. The replay evidence,
source receipt, snapshot shape/root and exact session stage are checked before
reading. Every compact byte must represent an integer in `[-1,1]` for `r` or
`[-2,2]` for either error role before scalar allocation.

The signed plane embeds `x` arithmetically in T256. The negative plane contains
`n=max(-x,0)`. Every commitment is an actual secret MSM over the existing
16,384-coordinate generator basis with a nonzero rho sampled from the original
retained RNG. The exact next empty shared inventory slot receives that point
before any value emission. There is no independent positive commitment:
`Cplus=Csigned+Cnegative`, with opening `x+n` and mask `rho_x+rho_n`. A derived
identity point rejects the entire consuming owner; it does not trigger a
replacement rho or point.

The original source, D/S, top, delta and beta/m masks remain in their existing
owners, along with the sole opening append permit. New signed/negative masks
occupy 66,048 bytes within the unchanged full-inventory allowance. A single
16,384-byte compact chunk and 524,288-byte prepared scalar vector are used for
each plane; emitted chunks contain 512 scalars. The two compact passes add
33,849,600 authenticated file-read bytes. These named payload/read counts do
not include the existing validation, MSM, allocator, provider or kernel costs
and do not qualify peak RSS or whole-proof lifecycle resources.

All 32 chunks must be emitted in order before the source returns. Wrong stage,
ordinal, occupied slot, malformed source, read failure, entropy failure, MSM
failure, incomplete emission or unwind consumes the relevant outer owner.
There is no retry capability or detached statement/point adoption interface.

The focused test filter is:

```sh
scripts/cargo_fast.sh --stable-local-metadata --incremental -- test --locked -p iroha_zkp_halo2 --lib prepared_small_signed -- --nocapture
```

Tests cover the shared coordinates against the source-packing mapping,
independent integer embedding vectors, role-bound violations, actual tiny-file
authenticated reads, first/last signed and negative MSMs, derived-positive
opening arithmetic and identity refusal, and original retained dispatch.
Earlier commitment inventories in those tests are synthetic fixtures with
original-session rho samples. Tiny files and selected actual MSMs do not
establish a complete authenticated 43-record source, full 2,064-MSM execution,
production context authority, or resource qualification.

TODO: connect each consuming value sequence to its canonical stored-opening
tail. Complete small positive/negative membership and inverse relations,
source-packing same-opening proof with the derived-mask owner, Q-mask
production, governed source authority and composite admission remain separate
unfinished obligations. No qualification or release flag changes here.
