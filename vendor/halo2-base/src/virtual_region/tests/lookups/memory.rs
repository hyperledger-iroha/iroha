use crate::{
    halo2_proofs::{
        arithmetic::Field,
        circuit::{Layouter, SimpleFloorPlanner},
        dev::MockProver,
        halo2curves::bn256::Fr,
        plonk::{keygen_pk, keygen_vk, Assigned, Circuit, ConstraintSystem, Error, FirstPhase},
    },
    virtual_region::{
        copy_constraints::EXTERNAL_CELL_TYPE_ID, lookups::basic::BasicDynLookupConfig,
    },
    AssignedValue, ContextCell,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use test_log::test;

use crate::{
    gates::{
        flex_gate::{threads::SinglePhaseCoreManager, FlexGateConfig, FlexGateConfigParams},
        GateChip, GateInstructions,
    },
    utils::{
        fs::gen_srs,
        testing::{check_proof, gen_proof},
        ScalarField,
    },
    virtual_region::manager::VirtualRegionManager,
};

#[derive(Clone, Debug)]
struct RAMConfig<F: ScalarField> {
    cpu: FlexGateConfig<F>,
    memory: BasicDynLookupConfig<2>,
}

#[derive(Clone, Default)]
struct RAMConfigParams {
    cpu: FlexGateConfigParams,
    num_lu_sets: usize,
}

struct RAMCircuit<F: ScalarField, const CYCLES: usize> {
    // private memory input
    memory: Vec<F>,
    // memory accesses
    ptrs: [usize; CYCLES],

    cpu: SinglePhaseCoreManager<F>,
    mem_access: Vec<[AssignedValue<F>; 2]>,

    params: RAMConfigParams,
}

impl<F: ScalarField, const CYCLES: usize> RAMCircuit<F, CYCLES> {
    fn new(
        memory: Vec<F>,
        ptrs: [usize; CYCLES],
        params: RAMConfigParams,
        witness_gen_only: bool,
    ) -> Self {
        let cpu = SinglePhaseCoreManager::new(witness_gen_only, Default::default());
        let mem_access = vec![];
        Self { memory, ptrs, cpu, mem_access, params }
    }

    fn compute(&mut self) {
        let gate = GateChip::default();
        let ctx = self.cpu.main();
        let mut sum = ctx.load_constant(F::ZERO);
        for &ptr in &self.ptrs {
            let value = self.memory[ptr];
            let ptr = ctx.load_witness(F::from(ptr as u64));
            let value = ctx.load_witness(value);
            self.mem_access.push([ptr, value]);
            sum = gate.add(ctx, sum, value);
        }
    }
}

impl<F: ScalarField, const CYCLES: usize> Circuit<F> for RAMCircuit<F, CYCLES> {
    type Config = RAMConfig<F>;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = RAMConfigParams;

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn without_witnesses(&self) -> Self {
        unimplemented!()
    }

    fn configure_with_params(meta: &mut ConstraintSystem<F>, params: Self::Params) -> Self::Config {
        let memory = BasicDynLookupConfig::new(meta, || FirstPhase, params.num_lu_sets);
        let cpu = FlexGateConfig::configure(meta, params.cpu);

        log::debug!("Poisoned rows: {}", meta.minimum_rows());

        RAMConfig { cpu, memory }
    }

    fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
        unreachable!()
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl Layouter<F>,
    ) -> Result<(), Error> {
        layouter.assign_region(
            || "cpu",
            |mut region| {
                self.cpu.assign_raw(
                    &(config.cpu.basic_gates[0].clone(), config.cpu.max_rows),
                    &mut region,
                );
                Ok(())
            },
        )?;

        let copy_manager = (!self.cpu.witness_gen_only()).then_some(&self.cpu.copy_manager);

        // Make purely virtual cells so we can raw assign them
        let memory = self.memory.iter().enumerate().map(|(i, value)| {
            let idx = Assigned::Trivial(F::from(i as u64));
            let idx = AssignedValue {
                value: idx,
                cell: Some(ContextCell::new(EXTERNAL_CELL_TYPE_ID, 0, i)),
            };
            let value = Assigned::Trivial(*value);
            let value =
                AssignedValue { value, cell: Some(ContextCell::new(EXTERNAL_CELL_TYPE_ID, 1, i)) };
            [idx, value]
        });

        config.memory.assign_virtual_table_to_raw(
            layouter.namespace(|| "memory"),
            memory,
            copy_manager,
        );

        config.memory.assign_virtual_to_lookup_to_raw(
            layouter.namespace(|| "memory accesses"),
            self.mem_access.clone(),
            copy_manager,
        );
        // copy constraints at the very end for safety:
        layouter.assign_region(
            || "copy constraints",
            |mut region| {
                self.cpu.copy_manager.assign_raw(&config.cpu.constants, &mut region);
                Ok(())
            },
        )
    }
}

#[test]
fn test_ram_mock() {
    let k = 5u32;
    const CYCLES: usize = 50;
    let mut rng = StdRng::seed_from_u64(0);
    let mem_len = 16usize;
    let memory: Vec<_> = (0..mem_len).map(|_| Fr::random(&mut rng)).collect();
    let ptrs = [(); CYCLES].map(|_| rng.gen_range(0..memory.len()));
    let usable_rows = 2usize.pow(k) - 11; // guess
    let params = RAMConfigParams::default();
    let mut circuit = RAMCircuit::new(memory, ptrs, params, false);
    circuit.compute();
    // auto-configuration stuff
    let num_advice = circuit.cpu.total_advice() / usable_rows + 1;
    circuit.params.cpu = FlexGateConfigParams {
        k: k as usize,
        num_advice_per_phase: vec![num_advice],
        num_fixed: 1,
    };
    circuit.params.num_lu_sets = CYCLES / usable_rows + 1;
    MockProver::run(k, &circuit, vec![]).unwrap().assert_satisfied();
}

#[test]
#[should_panic = "called `Result::unwrap()` on an `Err` value: [Lookup dynamic lookup table(index: 2) is not satisfied in Region 2 ('[BasicDynLookupConfig] Advice cells to lookup') at offset 16]"]
fn test_ram_mock_failed_access() {
    let k = 5u32;
    const CYCLES: usize = 50;
    let mut rng = StdRng::seed_from_u64(0);
    let mem_len = 16usize;
    let memory: Vec<_> = (0..mem_len).map(|_| Fr::random(&mut rng)).collect();
    let ptrs = [(); CYCLES].map(|_| rng.gen_range(0..memory.len()));
    let usable_rows = 2usize.pow(k) - 11; // guess
    let params = RAMConfigParams::default();
    let mut circuit = RAMCircuit::new(memory, ptrs, params, false);
    circuit.compute();

    // === PRANK ===
    // Try to claim memory[0] = 0
    let ctx = circuit.cpu.main();
    let ptr = ctx.load_witness(Fr::ZERO);
    let value = ctx.load_witness(Fr::ZERO);
    circuit.mem_access.push([ptr, value]);
    // === end prank ===

    // auto-configuration stuff
    let num_advice = circuit.cpu.total_advice() / usable_rows + 1;
    circuit.params.cpu = FlexGateConfigParams {
        k: k as usize,
        num_advice_per_phase: vec![num_advice],
        num_fixed: 1,
    };
    circuit.params.num_lu_sets = CYCLES / usable_rows + 1;
    MockProver::run(k, &circuit, vec![]).unwrap().verify().unwrap();
}

#[test]
fn test_ram_prover() {
    let k = 10u32;
    const CYCLES: usize = 2000;

    let mut rng = StdRng::seed_from_u64(0);
    let mem_len = 500;

    let memory = vec![Fr::ZERO; mem_len];
    let ptrs = [0; CYCLES];

    let usable_rows = 2usize.pow(k) - 11; // guess
    let params = RAMConfigParams::default();
    let mut circuit = RAMCircuit::new(memory, ptrs, params, false);
    circuit.compute();
    let num_advice = circuit.cpu.total_advice() / usable_rows + 1;
    circuit.params.cpu = FlexGateConfigParams {
        k: k as usize,
        num_advice_per_phase: vec![num_advice],
        num_fixed: 1,
    };
    circuit.params.num_lu_sets = CYCLES / usable_rows + 1;

    let params = gen_srs(k);
    let vk = keygen_vk(&params, &circuit).unwrap();
    let pk = keygen_pk(&params, vk, &circuit).unwrap();
    let circuit_params = circuit.params();
    let break_points = circuit.cpu.break_points.borrow().clone().unwrap();
    drop(circuit);

    let memory: Vec<_> = (0..mem_len).map(|_| Fr::random(&mut rng)).collect();
    let ptrs = [(); CYCLES].map(|_| rng.gen_range(0..memory.len()));
    let mut circuit = RAMCircuit::new(memory, ptrs, circuit_params, true);
    *circuit.cpu.break_points.borrow_mut() = Some(break_points);
    circuit.compute();

    let proof = gen_proof(&params, &pk, circuit);
    check_proof(&params, pk.get_vk(), &proof, true);
}
