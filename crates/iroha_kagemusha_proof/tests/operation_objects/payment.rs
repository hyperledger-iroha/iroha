//! Exact native Payment/package digest composition and total altered transcripts.

use super::{
    semantics::{credential, parse},
    *,
};
use iroha_kagemusha_proof::operation_relation::{
    incoming_statement::IncomingStatementCells,
    objects::{
        credential::CredentialCells,
        payment::{PaymentCells, PaymentInputs},
        request::RequestCells,
    },
};
use iroha_plonk_recursion::obligation::ledger::Variant;

#[derive(Clone)]
struct Payment {
    request: ObjectCircuit,
    payer: ObjectCircuit,
    receipt: ObjectCircuit,
    statement: [Fp; 26],
    transcript: Vec<u8>,
    proof: Fp,
    provider: [Fp; 2],
    valid: bool,
}
impl Circuit<Fp> for Payment {
    type Config = Config;
    type FloorPlanner = SimpleFloorPlanner;
    type Params = ();
    fn without_witnesses(&self) -> Self {
        Self {
            request: self.request.without_witnesses(),
            payer: self.payer.without_witnesses(),
            receipt: self.receipt.without_witnesses(),
            ..self.clone()
        }
    }
    fn configure(meta: &mut ConstraintSystem<Fp>) -> Config {
        ObjectCircuit::configure(meta)
    }
    fn synthesize(&self, c: Config, mut layouter: impl Layouter<Fp>) -> Result<(), Error> {
        let mut glue = GlueChip::new(c.glue);
        let mut range = RunningSumChip::new(c.range);
        let mut hash = SpongeChip::new(c.hash);
        let mut bytes = BytesChip::new(c.bytes);
        range.load_table(&mut layouter)?;
        bytes.load_table(&mut layouter)?;
        let output = layouter.assign_region(
            || "Payment transcript",
            |mut region| {
                let mut uint = UintChip::new(&mut glue, &mut range);
                let request = parse(&self.request, &mut bytes, &mut uint, &mut hash, &mut region)?;
                let payer = parse(&self.payer, &mut bytes, &mut uint, &mut hash, &mut region)?;
                let receipt = parse(&self.receipt, &mut bytes, &mut uint, &mut hash, &mut region)?;
                let request = RequestCells::check(&mut uint, &mut hash, &mut region, &request)?;
                let payer = CredentialCells::check(&mut uint, &mut region, &payer)?;
                let value = |v: &Fp| {
                    if self.request.known {
                        Value::known(*v)
                    } else {
                        Value::unknown()
                    }
                };
                let fields = uint
                    .glue()
                    .witnesses(
                        &mut region,
                        &self.statement.iter().map(value).collect::<Vec<_>>(),
                    )?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let statement = IncomingStatementCells::constrain(
                    &mut uint,
                    &mut hash,
                    &mut region,
                    Variant::Send,
                    &fields,
                )?;
                let proof = uint.glue().witness(&mut region, value(&self.proof))?;
                let provider = uint
                    .glue()
                    .witnesses(
                        &mut region,
                        &self.provider.iter().map(value).collect::<Vec<_>>(),
                    )?
                    .try_into()
                    .map_err(|_| Error::Synthesis)?;
                let tape = self
                    .transcript
                    .iter()
                    .map(|b| {
                        if self.request.known {
                            Value::known(*b)
                        } else {
                            Value::unknown()
                        }
                    })
                    .collect::<Vec<_>>();
                let run = bytes.run(
                    &mut region,
                    &tape,
                    &PaymentCells::primary_segments(),
                    &PaymentCells::secondary_segments(),
                )?;
                let payment = PaymentCells::from_run(
                    &mut uint,
                    &mut hash,
                    &mut region,
                    &run,
                    &PaymentInputs {
                        request: &request,
                        payer: &payer,
                        statement: &statement,
                        receipt: &receipt,
                        provider: &provider,
                        proof_digest: &proof,
                    },
                )?;
                Ok([
                    payment.digest().clone(),
                    payment.package_digest().clone(),
                    payment.valid().word().clone(),
                ])
            },
        )?;
        for (i, word) in output.iter().enumerate() {
            layouter.constrain_instance(word.cell(), c.public, i)?;
        }
        Ok(())
    }
}
impl Payment {
    fn public(&self) -> Vec<Fp> {
        let mut p = vec![Fp::ZERO; 8];
        p[0] = p_bytes_native(u64::from_le_bytes(*b"kgwpay_1"), &self.transcript);
        p[1] = hash_with_domain(
            u64::from_le_bytes(*b"kgwpkg_1"),
            &[
                hash_with_domain(u64::from_le_bytes(*b"kgwstmt1"), &self.statement),
                self.proof,
                self.receipt.public(Fp::ZERO)[1],
            ],
        );
        p[2] = Fp::from(u64::from(self.valid));
        p
    }
    fn accepts(&self) -> bool {
        check_circuit(self, 13, &[self.public()], CheckMode::Strict).is_ok_and(|r| r.is_satisfied())
    }
    fn reject(mut self, reason: &str) {
        self.valid = false;
        assert!(self.accepts(), "total {reason}");
        self.valid = true;
        assert!(!self.accepts(), "forged {reason}");
    }
}
fn base() -> Payment {
    let request = cases()
        .into_iter()
        .find(|(c, ..)| c.kind == ObjectKind::Request)
        .expect("Request")
        .0;
    let receipt = cases()
        .into_iter()
        .find(|(_, _, name, _)| name == "Send receipt")
        .expect("receipt")
        .0;
    let j = fixture();
    let transcript = j["poseidon"]["large_input_digests"]
        .as_array()
        .expect("digests")
        .iter()
        .find(|v| v["domain"].as_str() == Some("kgwpay_1"))
        .expect("Payment");
    let provider = ::core::array::from_fn(|i| {
        Fp::from_u128(u128::from_le_bytes(
            receipt.bytes[66 + i * 16..82 + i * 16]
                .try_into()
                .expect("half"),
        ))
    });
    let proof = Fp::from_repr(receipt.bytes[242..274].try_into().expect("proof"))
        .into_option()
        .expect("canonical");
    let c = Payment {
        request,
        payer: credential(),
        receipt,
        provider,
        proof,
        statement: j["field_encodings"]["send_statement"]["items"]
            .as_array()
            .expect("statement")
            .iter()
            .map(|v| field(v.as_str().expect("hex")))
            .collect::<Vec<_>>()
            .try_into()
            .expect("26"),
        transcript: decode(transcript["body_hex"].as_str().expect("hex")),
        valid: true,
    };
    assert_eq!(
        c.public()[0],
        field(transcript["digest_hex"].as_str().expect("digest"))
    );
    assert_eq!(
        c.public()[1],
        field(
            j["poseidon"]["package"]["digest_hex"]
                .as_str()
                .expect("package")
        )
    );
    c
}

#[test]
fn native_payment_transcript_binds_every_byte_and_each_nested_component() {
    let c = base();
    assert!(c.accepts());
    for i in 0..PaymentCells::BYTES {
        let mut wrong = c.clone();
        wrong.transcript[i] ^= 1;
        wrong.reject(&format!("Payment byte{i}"));
    }
    for i in [
        2, 34, 66, 98, 130, 162, 194, 210, 242, 258, 290, 306, 314, 346, 354, 362, 394, 426, 458,
    ] {
        let mut wrong = c.clone();
        wrong.request.bytes[i] ^= 1;
        wrong.reject("nested Request");
    }
    for i in [2, 34, 66, 98, 130, 195, 227, 260, 300, 350, 444, 476] {
        let mut wrong = c.clone();
        wrong.payer.bytes[i] ^= 1;
        wrong.reject("nested payer credential");
    }
    for i in [0, 2, 34, 66, 98, 114, 146, 178, 210, 242, 274, 306, 338] {
        let mut wrong = c.clone();
        wrong.receipt.bytes[i] ^= 1;
        wrong.reject("nested receipt");
    }
    for i in 0..26 {
        let mut wrong = c.clone();
        wrong.statement[i] = -Fp::ONE;
        wrong.reject("nested incoming statement");
    }
    let mut wrong = c.clone();
    wrong.proof += Fp::ONE;
    wrong.reject("different proof tape digest");
    for i in 0..2 {
        let mut wrong = c.clone();
        wrong.provider[i] += Fp::ONE;
        wrong.reject("different provider");
    }
    let known = synthesize(&c, 13, None).expect("known");
    let unknown = synthesize(&c.without_witnesses(), 13, None).expect("unknown");
    assert_eq!(known.tables.fixed(), unknown.tables.fixed());
    assert_eq!(known.tables.permutation(), unknown.tables.permutation());
}
