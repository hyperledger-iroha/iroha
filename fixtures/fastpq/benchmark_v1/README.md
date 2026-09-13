# FASTPQ V1 benchmark schema fixtures

These fixtures exercise the Rust and Python report readers against one explicit
V1 contract. Every timing and device count is synthetic. No fixture establishes
GPU execution, parity, source provenance or proof qualification.

Eight accepted cases cover both six-lane operations, Metal/CUDA producer forms
and explicit CPU/GPU modes. Rejected cases remove required copies/fields,
substitute numeric types or retain retired scalar claims. The manifest records
expected parser outcomes and exact file hashes. The Python fixture generator
uses maintained projection/validation helpers; native Rust acceptance must be
verified independently against the same bytes.
