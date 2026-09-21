# Native fifteen-bit commitment preparation

This private arithmetic prerequisite constructs the same T256 commitment as the
existing full-scalar builder. An exact16384-element u16 owner rejects values at
or above32768 before admission; four fixed four-bit windows scan all16 public
table entries. The one-term rho builder retains all256 bits. The immutable public
basis comes only from the canonical T256 suite and remains serial, with no new
accelerator, caller-provided basis, global table cache or scalar truncation API.

The original commitment session creates its resource ledger at its sole entropy
handoff and retains it through all consuming phases. The canonical production
resource owner lives in `rns_native_resource_budget.rs`; the test-only qPCS tree
and the u15 kernel depend on it directly. Tree-specific errors wrap resource
errors, so the original session has no dependency on the prototype tree. Only the enclosing fully
replayed stored-source owner can enter the kernel phase. Table creation reserves
its actual vector/object lifetime plus construction scratch before allocation.
The lower arithmetic evaluator reserves the digit plane, named scratch and
result owner before reading digits. Retained production source owners expose
no caller-rho/digit arithmetic forwarding API; only table preparation is wired
until the actual original S/rho producer supplies its consuming transition. A result that outlives its table retains its own reservation.
The original ledger's Arc identity rejects substitution; no later stage accepts
another budget or an authority Boolean. Only pre-allocation Capacity restores
an unchanged source; allocation, malformed-input and unwind failures close it.

The public table payload is24MiB. Source-derived named scratch, owning Rust
objects and results are charged using their concrete layouts. Allocator metadata,
control allocations, compiler frames, the preexisting canonical basis and older
source allocations remain outside these new counters. The ledger retains the
canonical512MiB ceiling. Its existing work counter still describes hash work;
this slice does not invent shared units for T256 arithmetic or qualify128B work,
RSS, total I/O or complete proof resources. T256's existing serial policy and all
production admission seals remain unchanged.

This prepares no S coefficients or inventory tickets. Native40 governed source,
private uniform S ownership, actual qPCS masking/same-opening, digit membership,
remaining lookup openings and composite qualification remain open. The current
38-limb source preparation is not relabelled as native40 authority.
