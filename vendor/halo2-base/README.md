# `halo2-base`

`halo2-base` provides an embedded domain specific language (eDSL) for writing circuits with the [`halo2`](https://github.com/axiom-crypto/halo2) API. It simplifies circuit programming to declaring constraints over a single advice and selector column and provides built-in circuit tuning and support for multi-threaded witness generation.

For further details, see the [Rust docs](https://axiom-crypto.github.io/halo2-lib/halo2_base/).

## Virtual Region Managers

The core framework under which `halo2-base` operates is that of _virtual cell management_. We perform witness generation in a virtual region (outside of the low-level raw halo2 `Circuit::synthesize`) and only at the very end map it to a "raw/physical" region in halo2's Plonkish arithmetization.

We formalize this into a new trait `VirtualRegionManager`. Any `VirtualRegionManager` is associated with some subset of columns (more generally, a physical Halo2 region). It can manage its own virtual region however it wants, but it must provide a deterministic way to map the virtual region to the physical region.

We have the following examples of virtual region managers:

- `SinglePhaseCoreManager`: this is associated with our `BasicGateConfig` which is a simple [vertical custom gate](https://docs.axiom.xyz/zero-knowledge-proofs/getting-started-with-halo2#simplified-interface), in a single halo2 challenge phase. It manages a virtual region with a bunch of virtual columns (these are the `Context`s). One can think of all virtual columns as being concatenated into a single big column. Then given the target number of rows in the physical circuit, it will chunk the single virtual column appropriately into multiple physical columns.
- `CopyConstraintManager`: this is a global manager to allow virtual cells from different regions to be referenced. Virtual cells are referred to as `AssignedValue`. Despite the name (which is from historical reasons), these values are not actually assigned into the physical circuit. `AssignedValue`s are virtual cells. Instead they keep track of a tag for which virtual region they belong to, and some other identifying tag that loosely maps to a CPU thread. When a virtual cell is referenced and used, a copy is performed and the `CopyConstraintManager` keeps track of the equality. After the virtual cells are all physically assigned, this manager will impose the equality constraints on the physical cells.
  - This manager also keeps track of constants that are used, deduplicates them, and assigns all constants into dedicated fixed columns. It also imposes the equality constraints between advice cells and the fixed cells.
  - It is **very important** that all virtual region managers reference the same `CopyConstraintManager` to ensure that all copy constraints are managed properly. The `CopyConstraintManager` must also be raw assigned at the end of `Circuit::synthesize` to ensure the copy constraints are actually communicated to the raw halo2 API.
- `LookupAnyManager`: for any kind of lookup argument (either into a fixed table or dynamic table), we do not want to enable this lookup argument on every column of the circuit since enabling lookup is expensive. Instead, we allocate special advice columns (with no selector) where the lookup argument is always on. When we want to look up certain values, we copy them over to the special advice cells. This also means that the physical location of the cells you want to look up can be unstructured.

The virtual regions are also designed to be able to interact with raw halo2 sub-circuits. The overall architecture of a circuit that may use virtual regions managed by `halo2-lib` alongside raw halo2 sub-circuits looks as follows:

![Virtual regions with raw sub-circuit](https://user-images.githubusercontent.com/31040440/263155207-c5246cb1-f7f5-4214-920c-d4ae34c19e9c.png)

## [`BaseCircuitBuilder`](./src/gates/circuit/mod.rs)

A circuit builder in `halo2-lib` is a collection of virtual region managers with an associated raw halo2 configuration of columns and custom gates. The precise configuration of these columns and gates can potentially be tuned after witness generation has been performed. We do not yet codify the notion of a circuit builder into a trait.

The core circuit builder used throughout `halo2-lib` is the `BaseCircuitBuilder`. It is associated to `BaseConfig`, which consists of instance columns together with either `FlexGateConfig` or `RangeConfig`: `FlexGateConfig` is used when no functionality involving bit range checks (usually necessary for less than comparisons on numbers) is needed, otherwise `RangeConfig` consists of `FlexGateConfig` together with a fixed lookup table for range checks.

The basic construction of `BaseCircuitBuilder` is as follows:

```rust
let k = 10; // your circuit will have 2^k rows
let witness_gen_only = false; // constraints are ignored if set to true
let mut builder = BaseCircuitBuilder::new(witness_gen_only).use_k(k);
// If you need to use range checks, a good default is to set `lookup_bits` to 1 less than `k`
let lookup_bits = k - 1;
builder.set_lookup_bits(lookup_bits); // this can be skipped if you are not using range checks. The program will panic if `lookup_bits` is not set when you need range checks.

// this is the struct holding basic our eDSL API functions
let gate = GateChip::default();
// if you need RangeChip, construct it with:
let range = builder.range_chip(); // this will panic if `builder` did not set `lookup_bits`
{
    // basic usage:
    let ctx = builder.main(0); // this is "similar" to spawning a new thread. 0 refers to the halo2 challenge phase
    // do your computations
}
// `builder` now contains all information from witness generation and constraints of your circuit
let unusable_rows = 9; // this is usually enough, set to 20 or higher if program panics
// This tunes your circuit to find the optimal configuration
builder.calculate_params(Some(unusable_rows));

// Now you can mock prove or prove your circuit:
// If you have public instances, you must either provide them yourself or extract from `builder.assigned_instances`.
MockProver::run(k as u32, &builder, instances).unwrap().assert_satisfied();
```

### Proving mode

`witness_gen_only` is set to `true` if we only care about witness generation and not about circuit constraints, otherwise it is set to false. This should **not** be set to `true` during mock proving or **key generation**. When this flag is `true`, we perform certain optimizations that are only valid when we don't care about constraints or selectors. This should only be done in the context of real proving, when a proving key has already been created.

## [**Context**](src/lib.rs)

`Context` holds all information of an execution trace (circuit and its witness values). `Context` represents a "virtual column" that stores unassigned constraint information in the Halo2 backend. Storing the circuit information in a `Context` rather than assigning it directly to the Halo2 backend allows for the pre-computation of circuit parameters and preserves the underlying circuit information allowing for its rearrangement into multiple columns for parallelization in the Halo2 backend.

During `synthesize()`, the advice values of all `Context`s are concatenated into a single "virtual column" that is split into multiple true `Column`s at `break_points` each representing a different sub-section of the "virtual column". During circuit synthesis, all cells are assigned to Halo2 `AssignedCell`s in a single `Region` within Halo2's backend.

For parallel witness generation, multiple `Context`s are created for each parallel operation. After parallel witness generation, these `Context`'s are combined to form a single "virtual column" as above. Note that while the witness generation can be multi-threaded, the ordering of the contents in each `Context`, and the order of the `Context`s themselves, must be deterministic.

**Warning:** If you create your own `Context` in a new virtual region not provided by our libraries, you must ensure that the `type_id: &str` of the context is a globally unique identifier for the virtual region, distinct from the other `type_id` strings used to identify other virtual regions. We suggest that you either include your crate name as a prefix in the `type_id` or use [`module_path!`](https://doc.rust-lang.org/std/macro.module_path.html) to generate a prefix.
In the future we will introduce a macro to check this uniqueness at compile time.

### [**AssignedValue**](./src/lib.rs):

Despite the name, an `AssignedValue` is a **virtual cell**. It contains the actual witness value as well as a pointer to the location of the virtual cell within a virtual region. The pointer is given by type `ContextCell`. We only store the pointer when not in witness generation only mode as an optimization.

```rust ignore
pub struct AssignedValue<F: ScalarField> {
    pub value: Assigned<F>,
    pub cell: Option<ContextCell>,
}
```

### [**Assigned**](./src/plonk/assigned.rs)

`Assigned` is not a ZK or circuit-related type.
`Assigned` is a wrapper enum for a field element which stores the value as a fraction and marks it for batched inversion using [Montgomery's trick](https://zcash.github.io/halo2/background/fields.html#montgomerys-trick). Performing batched inversion allows for the computation of the inverse of all marked values with a single inversion operation.

```rust ignore
pub enum Assigned<F> {
    /// The field element zero.
    Zero,
    /// A value that does not require inversion to evaluate.
    Trivial(F),
    /// A value stored as a fraction to enable batch inversion.
    Rational(F, F),
}
```

## [**QuantumCell**](./src/lib.rs)

`QuantumCell` is a helper enum that abstracts the scenarios in which a value is assigned to the advice column in `halo2-base`. Without `QuantumCell`, assigning existing or constant values to the advice column requires manually specifying the enforced constraints on top of assigning the value leading to bloated code. `QuantumCell` handles these technical operations, all a developer needs to do is specify which enum option in `QuantumCell` the value they are adding corresponds to.

```rust ignore
pub enum QuantumCell<F: ScalarField> {
    Existing(AssignedValue<F>),
    Witness(F),
    WitnessFraction(Assigned<F>),
    Constant(F),
}
```

QuantumCell contains the following enum variants.

- **Existing**:
  Assigns a value to the advice column that exists within the advice column. The value is an existing value from some previous part of your computation already in the advice column in the form of an `AssignedValue`. When you add an existing cell into the table a new cell will be assigned into the advice column with value equal to the existing value. An equality constraint will then be added between the new cell and the "existing" cell so the Verifier has a guarantee that these two cells are always equal.

- **Witness**:
  Assigns an entirely new witness value into the advice column, such as a private input. When `assign_cell()` is called the value is wrapped in as an `Assigned::Trivial()` which marks it for exclusion from batch inversion.

- **WitnessFraction**:
  Assigns an entirely new witness value to the advice column. `WitnessFraction` exists for optimization purposes and accepts Assigned values wrapped in `Assigned::Rational()` marked for batch inverion (see [Assigned](#assigned)).

- **Constant**:
  A value that is a "known" constant. A "known" refers to known at circuit creation time to both the Prover and Verifier. When you assign a constant value there exists another secret Fixed column in the circuit constraint table whose values are fixed at circuit creation time. When you assign a Constant value, you are adding this value to the Fixed column, adding the value as a witness to the Advice column, and then imposing an equality constraint between the two corresponding cells in the Fixed and Advice columns.
