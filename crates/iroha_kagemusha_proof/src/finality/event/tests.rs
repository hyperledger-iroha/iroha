//! Exact native event hash, canonical digest bytes, and typed receipt linkage.

use super::*;
use ff::PrimeField;
use iroha_plonk::{
    check::{CheckMode, check_circuit},
    cs::{Column, ConstraintSystem, Instance},
    frontend::{Circuit, Layouter, SimpleFloorPlanner, Value, synthesize},
};
use iroha_plonk_gadgets::{
    GlueConfig, LimbBits, RunningSumChip, RunningSumConfig,
    blake2b::{BLAKE2B_ADVICE_COLUMNS, Blake2bConfig},
    tamper::{Tamper, assigned_advice_cells, check_tampered},
};

#[derive(Clone)]
struct EventCircuit {
    digest: Fp,
    known: bool,
}
#[derive(Clone)]
struct Config {
    glue: GlueConfig,
    range: RunningSumConfig,
    blake: Blake2bConfig,
    public: Column<Instance>,
}
impl Circuit<Fp> for EventCircuit {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            known: false,
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Config {
        let columns = core::array::from_fn(|_| meta.advice_column());
        let fixed = meta.fixed_column();
        let glue = GlueConfig::configure(meta, columns, fixed);
        let column = meta.advice_column();
        let range = RunningSumConfig::configure(meta, column, LimbBits::new(4).unwrap());
        let columns =
            core::array::from_fn::<_, BLAKE2B_ADVICE_COLUMNS, _>(|_| meta.advice_column());
        let fixed = meta.fixed_column();
        let blake = Blake2bConfig::configure(meta, columns, fixed);
        let public = meta.instance_column(78);
        meta.enable_equality(public);
        Config {
            glue,
            range,
            blake,
            public,
        }
    }
    fn synthesize(&self, config: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut glue = GlueChip::new(config.glue);
        let mut range = RunningSumChip::new(config.range);
        let mut blake = Blake2bChip::new(&config.blake);
        range.load_table(&mut layouter)?;
        let output = layouter.assign_region(
            || "canonical typed Load event",
            |mut region| {
                let digest = glue.witness(
                    &mut region,
                    if self.known {
                        Value::known(self.digest)
                    } else {
                        Value::unknown()
                    },
                )?;
                let mut uint = UintChip::new(&mut glue, &mut range);
                let event =
                    LoadEventCells::from_digest(&mut uint, &mut blake, &mut region, &digest)?;
                let mut out = vec![digest];
                out.extend_from_slice(event.preimage());
                out.extend_from_slice(event.hash().bytes());
                Ok(out)
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), config.public, i)?;
        }
        Ok(())
    }
}
fn fixture() -> (EventCircuit, Vec<Fp>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/kagemusha/ordinary_load_receipt_v1.json");
    let fixture: norito::json::Value =
        norito::json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let decode = |name: &str| {
        let hex = fixture
            .get(name)
            .and_then(norito::json::Value::as_str)
            .unwrap();
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>()
    };
    let digest: [u8; 32] = decode("receipt_digest_hex").try_into().unwrap();
    let digest = Option::<Fp>::from(Fp::from_repr(digest)).unwrap();
    let mut public = vec![digest];
    let preimage = decode("event_box_hash_preimage_hex");
    assert_eq!(&preimage[..13], &PREFIX);
    public.extend(
        preimage
            .into_iter()
            .chain(decode("event_box_hash_hex"))
            .map(|b| Fp::from(u64::from(b))),
    );
    (
        EventCircuit {
            digest,
            known: true,
        },
        public,
    )
}
#[test]
fn native_event_hash_matches_exact_adaptive_codec_preimage() {
    let (circuit, public) = fixture();
    assert!(
        check_circuit(
            &circuit,
            14,
            std::slice::from_ref(&public),
            CheckMode::Strict
        )
        .unwrap()
        .is_satisfied()
    );
    for index in [0, 1, 8, 13, 14, 45, 46, 77] {
        let mut forged = public.clone();
        forged[index] += Fp::ONE;
        assert!(
            !check_circuit(&circuit, 14, &[forged], CheckMode::Strict)
                .unwrap()
                .is_satisfied()
        );
    }
    let mut other_receipt = circuit.clone();
    other_receipt.digest += Fp::ONE;
    assert!(
        !check_circuit(&other_receipt, 14, &[public], CheckMode::Strict)
            .unwrap()
            .is_satisfied()
    );
    assert!(synthesize(&circuit.without_witnesses(), 14, None).is_ok());
}
#[test]
fn event_digest_and_hash_assignment_reject_cell_tampering() {
    let (circuit, public) = fixture();
    let cells = assigned_advice_cells(&circuit, 14, std::slice::from_ref(&public)).unwrap();
    let stride = (cells.len() / 24).max(1);
    for &(column, row) in cells.iter().step_by(stride) {
        let tamper = Tamper {
            column,
            row,
            delta: Fp::ONE,
        };
        assert!(
            !check_tampered(&circuit, 14, std::slice::from_ref(&public), Some(tamper))
                .unwrap()
                .is_satisfied()
        );
    }
}
