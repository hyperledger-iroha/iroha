# Compact multiplicative-coset folding

`backend/fri_fold.rs` implements the algebra needed for arities 2, 4, 8 and 16.
It is an internal normal-library arithmetic module used by the fixed
[DEEP offline profile](fastpq_deep_protocol_contract.md), whose ordered arities
are `[16,16,8,8,4]`. That profile owns commitments, transcript, authenticated
openings, degree progression and limits. This arithmetic is not production
admission or cryptographic qualification.

For `f(X) = sum_{j=0}^{r-1} X^j f_j(X^r)`, the ordered input fiber contains
`f(x * omega^i)` for `i=0..r`, where omega has exact order r. The inverse
transform returns `r*x^j*f_j(x^r)` before normalization. Horner evaluation at
`beta/x`, followed by division by r, therefore returns
`sum_j beta^j f_j(x^r)`. Every beta coordinate remains in the existing quartic
extension. A degree bound d becomes `ceil(d/r)`; callers must authenticate both
the fiber's position and the claimed successive domains.

The checked plan validates canonical roots of exact order. Single-fiber
evaluation validates every field coordinate, the exact width and a nonzero
base point before arithmetic. Whole-layer evaluation validates the complete
input and domain before changing caller-owned output. Its strided fibers match
the multiplicative cosets, and each output position belongs to one job. The
inverse transform uses at most sixteen stack field values, base-field twiddles,
and no per-fiber allocation or inversion. At most 32 Rayon jobs use the same
kernel as the serial path. Metal/CUDA folding parity remains unqualified.

Tests compare every arity with a coefficient-based oracle, exercise every
extension challenge coordinate, preserve the existing binary fold, check
successive degree reduction, reject malformed domains/fields without changing
output, and compare one/four-worker execution for every supported arity. These
tests do not establish protocol soundness or production performance.
