use crate::ff::Field;
use crate::halo2_proofs::circuit::Region;

/// A virtual region manager is responsible for managing a virtual region and assigning the
/// virtual region to a physical Halo2 region.
///
pub trait VirtualRegionManager<F: Field> {
    /// The Halo2 config with associated columns and gates describing the physical Halo2 region
    /// that this virtual region manager is responsible for.
    type Config: Clone;
    /// Return type of the `assign_raw` method. Default is `()`.
    type Assignment;

    /// Assign virtual region this is in charge of to the raw region described by `config`.
    fn assign_raw(&self, config: &Self::Config, region: &mut Region<F>) -> Self::Assignment;
}
