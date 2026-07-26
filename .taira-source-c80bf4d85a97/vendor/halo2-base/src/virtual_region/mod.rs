//! Trait describing the shared properties for a struct that is in charge of managing a virtual region of a circuit
//! _and_ assigning that virtual region to a "raw" Halo2 region in the "physical" circuit.
//!
//! Currently a raw region refers to a subset of columns of the circuit, and spans all rows (so it is a vertical region),
//! but this is not a requirement of the trait.

/// Shared copy constraints across different virtual regions
pub mod copy_constraints;
/// Virtual region manager for lookup tables
pub mod lookups;
/// Virtual region manager
pub mod manager;

#[cfg(test)]
mod tests;
