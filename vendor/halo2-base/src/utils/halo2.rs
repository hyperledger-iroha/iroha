use std::collections::hash_map::Entry;

use crate::ff::Field;
use crate::halo2_proofs::{
    circuit::{AssignedCell, Cell, Region, Value},
    halo2curves::bn256::Bn256,
    plonk::{Advice, Assigned, Circuit, Column, Fixed},
    poly::kzg::commitment::ParamsKZG,
};
use crate::virtual_region::copy_constraints::{CopyConstraintManager, EXTERNAL_CELL_TYPE_ID};
use crate::AssignedValue;

pub use keygen::ProvingKeyGenerator;

/// Raw (physical) assigned cell in Plonkish arithmetization.
#[cfg(feature = "halo2-axiom")]
pub type Halo2AssignedCell<'v, F> = AssignedCell<&'v Assigned<F>, F>;
/// Raw (physical) assigned cell in Plonkish arithmetization.
#[cfg(not(feature = "halo2-axiom"))]
pub type Halo2AssignedCell<'v, F> = AssignedCell<Assigned<F>, F>;

/// Assign advice to physical region.
#[inline(always)]
pub fn raw_assign_advice<'v, F: Field>(
    region: &mut Region<F>,
    column: Column<Advice>,
    offset: usize,
    value: Value<impl Into<Assigned<F>>>,
) -> Halo2AssignedCell<'v, F> {
    #[cfg(feature = "halo2-axiom")]
    {
        region.assign_advice(column, offset, value)
    }
    #[cfg(feature = "halo2-pse")]
    {
        let value = value.map(|a| Into::<Assigned<F>>::into(a));
        region
            .assign_advice(
                || format!("assign advice {column:?} offset {offset}"),
                column,
                offset,
                || value,
            )
            .unwrap()
    }
}

/// Assign fixed to physical region.
#[inline(always)]
pub fn raw_assign_fixed<F: Field>(
    region: &mut Region<F>,
    column: Column<Fixed>,
    offset: usize,
    value: F,
) -> Cell {
    #[cfg(feature = "halo2-axiom")]
    {
        region.assign_fixed(column, offset, value)
    }
    #[cfg(feature = "halo2-pse")]
    {
        region
            .assign_fixed(
                || format!("assign fixed {column:?} offset {offset}"),
                column,
                offset,
                || Value::known(value),
            )
            .unwrap()
            .cell()
    }
}

/// Constrain two physical cells to be equal.
#[inline(always)]
pub fn raw_constrain_equal<F: Field>(region: &mut Region<F>, left: Cell, right: Cell) {
    #[cfg(feature = "halo2-axiom")]
    region.constrain_equal(left, right);
    #[cfg(not(feature = "halo2-axiom"))]
    region.constrain_equal(left, right).unwrap();
}

/// Constrains that `virtual_cell` is equal to `external_cell`. The `virtual_cell` must have
/// already been raw assigned with the raw assigned cell stored in `copy_manager`
/// **unless** it is marked an external-only cell with type id [EXTERNAL_CELL_TYPE_ID].
/// * When the virtual cell has already been assigned, the assigned cell is constrained to be equal to the external cell.
/// * When the virtual cell has not been assigned **and** it is marked as an external cell, it is assigned to `external_cell` and the mapping is stored in `copy_manager`.
///
/// This should only be called when `witness_gen_only` is false, otherwise it will panic.
///
/// ## Panics
/// If witness generation only mode is true.
pub fn constrain_virtual_equals_external<F: Field + Ord>(
    region: &mut Region<F>,
    virtual_cell: AssignedValue<F>,
    external_cell: Cell,
    copy_manager: &mut CopyConstraintManager<F>,
) {
    let ctx_cell = virtual_cell.cell.unwrap();
    match copy_manager.assigned_advices.entry(ctx_cell) {
        Entry::Occupied(acell) => {
            // The virtual cell has already been assigned, so we can constrain it to equal the external cell.
            region.constrain_equal(*acell.get(), external_cell);
        }
        Entry::Vacant(assigned) => {
            // The virtual cell **must** be an external cell
            assert_eq!(ctx_cell.type_id, EXTERNAL_CELL_TYPE_ID);
            // We map the virtual cell to point to the raw external cell in `copy_manager`
            assigned.insert(external_cell);
        }
    }
}

/// This trait should be implemented on the minimal circuit configuration data necessary to
/// completely determine a circuit (independent of circuit inputs).
/// This is used to generate a _dummy_ instantiation of a concrete `Circuit` type for the purposes of key generation.
/// This dummy instantiation just needs to have the correct arithmetization format, but the witnesses do not need to
/// satisfy constraints.
pub trait KeygenCircuitIntent<F: Field> {
    /// Concrete circuit type
    type ConcreteCircuit: Circuit<F>;
    /// Additional data that "pins" down the circuit. These can always to deterministically rederived from `Self`, but
    /// storing the `Pinning` saves recomputations in future proof generations.
    type Pinning;

    /// The intent must include the log_2 domain size of the circuit.
    /// This is used to get the correct trusted setup file.
    fn get_k(&self) -> u32;

    /// Builds a _dummy_ instantiation of `Self::ConcreteCircuit` for the purposes of key generation.
    /// This dummy instantiation just needs to have the correct arithmetization format, but the witnesses do not need to
    /// satisfy constraints.
    fn build_keygen_circuit(self) -> Self::ConcreteCircuit;

    /// Pinning is only fully computed after `synthesize` has been run during keygen
    fn get_pinning_after_keygen(
        self,
        kzg_params: &ParamsKZG<Bn256>,
        circuit: &Self::ConcreteCircuit,
    ) -> Self::Pinning;
}

mod keygen {
    use crate::halo2_proofs::{
        halo2curves::bn256::{Bn256, Fr, G1Affine},
        plonk::{self, ProvingKey},
        poly::{commitment::Params, kzg::commitment::ParamsKZG},
    };

    use super::KeygenCircuitIntent;

    /// Trait for creating a proving key and a pinning for a circuit from minimal circuit configuration data.
    pub trait ProvingKeyGenerator {
        /// Create proving key and pinning.
        fn create_pk_and_pinning(
            self,
            kzg_params: &ParamsKZG<Bn256>,
        ) -> (ProvingKey<G1Affine>, serde_json::Value);
    }

    impl<CI> ProvingKeyGenerator for CI
    where
        CI: KeygenCircuitIntent<Fr> + Clone,
        CI::Pinning: serde::Serialize,
    {
        fn create_pk_and_pinning(
            self,
            kzg_params: &ParamsKZG<Bn256>,
        ) -> (ProvingKey<G1Affine>, serde_json::Value) {
            assert_eq!(kzg_params.k(), self.get_k());
            let circuit = self.clone().build_keygen_circuit();
            #[cfg(feature = "halo2-axiom")]
            let pk = plonk::keygen_pk2(kzg_params, &circuit, false).unwrap();
            #[cfg(not(feature = "halo2-axiom"))]
            let pk = {
                let vk = plonk::keygen_vk_custom(kzg_params, &circuit, false).unwrap();
                plonk::keygen_pk(kzg_params, vk, &circuit).unwrap()
            };
            let pinning = self.get_pinning_after_keygen(kzg_params, &circuit);
            (pk, serde_json::to_value(pinning).unwrap())
        }
    }
}
