# Compact SMT public-column projection

`backend/compact_public_columns.rs` owns the fixed layout used by the normal
[DEEP offline profile](fastpq_deep_protocol_contract.md): retain 301 columns and
reconstruct the exact 342-cell `compact_trace_columns` row before reference AIR
evaluation. This substitution does not constitute protocol, security or resource
qualification.

The ordered public indices are fixed by the current compact BLAKE2b witness,
SMT preimage and physical padding:

| Reference indices | Cells removed | Known base-row values |
| --- | ---: | --- |
| 32..35 | 4 | First sixteen bytes of `fastpq:v1:smt:node\|`, in four little-endian u32 limbs, on execution rows |
| 53..63 | 11 | Message limbs 21..31, identically zero |
| 276..299 | 24 | `present[b] = 1` precisely when `phase < 6` and `24*phase+b < 83` |
| 300 | 1 | Exact node length 83 on execution rows |
| 301 | 1 | `min(24*(phase+1),83)` on phases 0..5, then 83 on phases 6..407 |

Every hash cell is zero on physical phases 408..511. The period is 512 and
there are exactly 128 hash invocations, hence N=65,536. Message limb 4 contains
three fixed domain bytes and one variable child byte; limb 20 contains three
variable child bytes and one zero byte. Both complete limbs remain committed.
All 32 SMT port columns and every other reference column remain in increasing
reference-index order.

Source owners are `compact_trace_columns::{hash_row_cells,smt_row_cells}`,
`compact_blake2b_air::CompactHashWitness::from_bytes`,
`compact_smt_air::SmtWitness::{from_inputs,into_physical}`, and
`fixed_schedule::PeriodicSelectors`. The projection rejects any noncanonical
full-row cell or mismatch with the expected public values before removing it.
This validation establishes the projection's preconditions, not AIR validity
or statement authentication.

## Polynomial reconstruction

Let g be the checked trace generator and E_r the existing period-512 selector,
with degree N-N/512 = 65,408. At an arbitrary canonical base or Fp4 point X:

- A is the sum of E_0 through E_407.
- Domain limb i is its exact u32 value times A.
- The eleven complete padding limbs remain the zero polynomial.
- Presence cells 0..10 equal E_0+E_1+E_2+E_3; cells 11..23 equal E_0+E_1+E_2.
- Length equals 83A.
- Prefix count equals 24E_0+48E_1+72E_2+83(A-E_0-E_1-E_2).

These are linear combinations of known polynomials. Their degree remains below
N, preserving the reference AIR's existing unmasked degree assumptions. No
fixed value is chosen from an LDE index or from one coordinate of an extension
point. Even base-row Boolean or u32 values generally become arbitrary field
values off the trace subgroup.

`reconstruct_at` restores the 41 polynomial values at X and copies all retained
coordinates unchanged. `reconstruct_pair_at` evaluates the next row at gX,
using the checked trace generator. The reference AIR still owns noncyclic
boundary selectors; reconstructing a pair does not introduce a cyclic edge.

## Protocol integration and validation

The DEEP context and artifact descriptor bind the exact layout identity, ordered
projection, fixed schedule, field basis and 301-column dimensions. Its OOD AIR
check reconstructs all 342 cells at the actual extension points. Its opening and
degree argument must still establish the required retained-column guarantees;
source wiring alone does not prove them. The predecessor 342-column transcript
and codec remain test-only diagnostics and cannot decode the DEEP child frame.

Five focused tests cover the exact column partition; every physical row's
projection and every phase's reconstruction from a complete native witness;
period interpolation against full IFFT, actual LDE and Horner evaluations;
arbitrary Fp4 points, the gX pair and all 923 reference AIR outputs; and malformed
geometry, widths, public cells and all noncanonical field coordinates. These
tests require execution in the approved build lane. Staging or narrow test
success alone does not qualify the new production-size protocol.
