//! Non-shipping isolated-device fixture. Its fixed public issuer is never
//! production trust. Actual device custody still performs every hardware receipt.
use super::*;
use ff::{Field, PrimeField};
use iroha_data_model::{
    kagemusha::*,
    sumeragi_finality::{SumeragiCommitVerifierV1, test_fixtures::NativeFinalityFixture},
};

/// Exact public scheme from the canonical wallet vectors, under test root 0x11.
pub fn scheme() -> KagemushaWalletSchemeV1 {
    let root = p256::mul(&p256::generator(), &BigUint::from_bytes_be(&[0x11; 32])).unwrap();
    let mut q = [0u8; 65];
    q[0] = 4;
    for (offset, value) in [(1, root.x), (33, root.y)] {
        let bytes = value.to_bytes_be();
        q[offset + 32 - bytes.len()..offset + 32].copy_from_slice(&bytes);
    }
    KagemushaWalletSchemeV1 {
        version: 1,
        network_id: [0x0b; 32],
        scheme_root_key: KagemushaDevicePublicKeyV1::try_from(q.as_slice()).unwrap(),
        relation_id: [
            0x91, 0xb6, 0x44, 0xa7, 0xaa, 0x99, 0xff, 0xf2, 0x44, 0x9f, 0x09, 0xa5, 0x3d, 0xf3,
            0x3d, 0x0d, 0xcd, 0xd6, 0x7b, 0x65, 0x2f, 0x73, 0x47, 0x27, 0x0f, 0x7d, 0xed, 0x6d,
            0x93, 0x9a, 0x73, 0xd6,
        ],
        provider_contract: kagemusha_wallet_provider_contract_v1(),
    }
}
/// Authenticate the complete real program bundle under the independent fixture root.
pub fn install(
    bundle: &[u8],
    manifest: &[u8],
    signature: [u8; 64],
) -> Result<AuthenticatedParameters> {
    let fixture = NativeFinalityFixture::new_with_explicit_parameters();
    let finality = SumeragiCommitVerifierV1::new(&fixture.verifier())
        .map_err(|e| Error::Rejected(e.to_string()))?;
    AuthenticatedParameters::install(bundle, manifest, signature, &scheme(), finality)
}
/// Issue only fixture-root-authorized future credential bytes for the actual
/// provider's enrolled intent and actual nonexportable public key. The receipt
/// remains unsigned until the provider durably selects its complete capsule.
pub fn bootstrap(
    parameters: &AuthenticatedParameters,
    challenge: &KagemushaWalletEnrollmentChallengeV1,
    key: &KagemushaDevicePublicKeyV1,
) -> Result<ExactOperationWitness> {
    if parameters.inner.scheme != scheme().scheme_id()
        || challenge.scheme_id != parameters.inner.scheme
    {
        return Err(Error::AuthorityMismatch("isolated fixture scheme"));
    }
    let hardware = crate::full::WalletKey {
        d: BigUint::from(1u8),
        q: super::point(key.as_ref())?,
    };
    issue(
        parameters,
        &hardware,
        &[
            (34, challenge.asset_digest),
            (66, challenge.wallet_id(key)),
            (98, challenge.account_digest),
            (404, challenge.app_policy),
            (448, challenge.enrollment_id(key)),
        ],
    )
}

/// Fixture-issuer credential and unsigned Bootstrap inputs for `key`'s public
/// point. Each `(offset, bytes)` replaces one 32-byte credential field before
/// the fixture issuer signs; the state, statement and receipt body follow from
/// the signed bytes.
fn issue(
    parameters: &AuthenticatedParameters,
    key: &crate::full::WalletKey,
    fields: &[(usize, [u8; 32])],
) -> Result<ExactOperationWitness> {
    let (context, mut input) = crate::bootstrap::fixture(key, &parameters.inner.generators.p);
    if context.relation != parameters.id {
        return Err(Error::AuthorityMismatch("fixture relation"));
    }
    let c = &mut input.credential;
    for (offset, bytes) in fields {
        c.body[*offset..*offset + 32].copy_from_slice(bytes);
    }
    c.signature = crate::bootstrap::sign(
        crate::credential::message(c),
        &BigUint::from_bytes_be(&[0x26; 32]),
        "isolated hardware fixture issuer",
    );
    let limbs = |bytes: &[u8]| -> [Fp; 2] {
        [
            crate::certificate::fp(&BigUint::from_bytes_le(&bytes[..16])),
            crate::certificate::fp(&BigUint::from_bytes_le(&bytes[16..])),
        ]
    };
    for (cell, offset) in [(1, 2), (3, 34), (5, 66)] {
        input.core[cell..cell + 2].copy_from_slice(&limbs(&c.body[offset..offset + 32]));
    }
    input.core[7] = crate::credential::digest(c);
    let head = crate::monetary_state::commitment(&input.core, &input.rest);
    input.statement[3..7].copy_from_slice(&input.core[1..5]);
    input.statement[7] = input.core[7];
    input.statement[15] = head;
    input.statement[17..19].copy_from_slice(&limbs(&c.body[448..480]));
    let operation = iroha_pasta::poseidon::hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[
            input.core[5],
            input.core[6],
            Fp::ONE,
            input.statement[17],
            input.statement[18],
        ],
    );
    input.receipt.body[34..66].copy_from_slice(&c.body[66..98]);
    input.receipt.body[114..146].copy_from_slice(&operation.to_repr());
    input.receipt.body[178..210].copy_from_slice(&head.to_repr());
    let statement =
        iroha_pasta::poseidon::hash_with_domain(u64::from_le_bytes(*b"kgwstmt1"), &input.statement);
    input.receipt.body[210..242].copy_from_slice(&statement.to_repr());
    input.receipt.signature.fill(0);
    Ok(ExactOperationWitness::Bootstrap(Box::new(
        BootstrapWitness {
            scheme: context.root.scheme,
            root: context.root.root,
            relation: context.relation,
            input,
        },
    )))
}

fn unsigned_receipt(c: &Credential, fields: &[Fp; 26], payment: Fp) -> bootstrap::Receipt {
    let mut receipt = bootstrap::Receipt {
        body: [0; 338],
        signature: [0; 64],
    };
    receipt.body[0] = 1;
    for (to, from) in [(2, 2), (34, 66), (66, 259)] {
        receipt.body[to..to + 32].copy_from_slice(&c.body[from..from + 32]);
    }
    receipt.body[98..114].copy_from_slice(&fields[9].to_repr()[..16]);
    let limbs = |at| crate::certificate::fp(&BigUint::from_bytes_le(&c.body[at..at + 16]));
    let operation = iroha_pasta::poseidon::hash_with_domain(
        u64::from_le_bytes(*b"kgwopid1"),
        &[limbs(66), limbs(82), fields[16], fields[17]],
    );
    for (at, value) in [
        (114, operation),
        (146, fields[14]),
        (178, fields[15]),
        (210, crate::statement::digest(fields)),
        (306, payment),
    ] {
        receipt.body[at..at + 32].copy_from_slice(&value.to_repr());
    }
    receipt
}

/// Genuine fixture-genesis BLS finality for a synthetic isolated Load100 event.
/// No production ledger transition or spendable funds are created by this fixture.
pub fn load100(bootstrap: &ExactOperationWitness) -> Result<ExactOperationWitness> {
    use iroha_data_model::{
        events::{
            EventBox,
            data::{DataEvent, kagemusha::KagemushaLoadCommittedV1},
        },
        isi::kagemusha_wallet::KagemushaWalletLoadReceiptV1,
        prelude::{HashOf, MerkleTree},
        sumeragi_finality::SumeragiCommitCertificateV1,
    };
    let ExactOperationWitness::Bootstrap(boot) = bootstrap else {
        return Err(Error::Malformed("fixture Load requires Bootstrap input"));
    };
    let input = &boot.input;
    let c = &input.credential;
    // Reserve the first post-genesis block for this exact Load event. The
    // `new` fixture already certifies H2 and would make next_header() select H3.
    let mut consensus =
        NativeFinalityFixture::start_with_explicit_parameters("portable-native-fixture");
    let header = consensus.next_header();
    let receipt = KagemushaWalletLoadReceiptV1 {
        version: 1,
        scheme_id: c.body[2..34].try_into().unwrap(),
        asset_digest: c.body[34..66].try_into().unwrap(),
        wallet_id: c.body[66..98].try_into().unwrap(),
        request_id: [0x45; 32],
        ordinal: 0,
        amount: 100,
        online_charge: 0,
        charge_quote: [0; 32],
        transaction_hash: [0x56; 32],
        block_height: header.height.get(),
        payer_account_digest: c.body[98..130].try_into().unwrap(),
    };
    let events = [EventBox::Data(
        DataEvent::KagemushaLoadCommitted(
            KagemushaLoadCommittedV1::from_receipt(&receipt)
                .map_err(|e| Error::Rejected(e.to_string()))?,
        )
        .into(),
    )];
    let tree: MerkleTree<EventBox> = events.iter().map(HashOf::new).collect();
    let block = consensus.block_with_submitted_work(header);
    let proof = consensus.certify_with_events(block, &events);
    let finality = KagemushaWalletLoadFinalityV1 {
        version: 1,
        receipt_digest: receipt
            .receipt_digest()
            .map_err(|e| Error::Rejected(e.to_string()))?,
        certificate: SumeragiCommitCertificateV1::from_proof(&proof)
            .map_err(|e| Error::Rejected(e.to_string()))?,
        event_proof: tree
            .get_proof(0)
            .ok_or(Error::Malformed("fixture Load event"))?,
    };
    let ordinary = receipt
        .transcript()
        .map_err(|e| Error::Rejected(e.to_string()))?;
    let digest = crate::load::ordinary_digest(&ordinary);
    let recovery_value = iroha_pasta::poseidon::hash_with_domain(
        crate::load::RECOVERY_DOMAIN,
        &[Fp::ZERO, digest, Fp::from(100)],
    );
    let (recovery, root) =
        crate::indexed_map::empty_insert(Fp::from_u128(1u128 << 127).double(), recovery_value);
    let mut after = input.core;
    after[9] = Fp::from(100);
    after[11] = Fp::ONE;
    after[13] = Fp::ONE;
    after[19] = root;
    after[29] += Fp::ONE;
    let mut fields = input.statement;
    fields[9] = Fp::ONE;
    fields[10] = Fp::ONE;
    fields[14] = crate::monetary_state::commitment(&input.core, &input.rest);
    fields[15] = crate::monetary_state::commitment(&after, &input.rest);
    fields[16] = Fp::from(2);
    fields[17] = digest;
    fields[18] = Fp::ZERO;
    fields[19] = Fp::from(100);
    fields[20..].fill(Fp::ZERO);
    Ok(ExactOperationWitness::Load {
        step: Box::new(crate::load::Witness {
            before_core: input.core,
            before_rest: input.rest,
            after_core: after,
            after_rest: input.rest,
            credential: c.clone(),
            ordinary,
            statement: fields,
            receipt: unsigned_receipt(c, &fields, Fp::ZERO),
            recovery,
        }),
        receipt_original: receipt
            .to_canonical_bytes()
            .map_err(|e| Error::Rejected(e.to_string()))?
            .into(),
        finality_original: finality
            .to_canonical_bytes()
            .map_err(|e| Error::Rejected(e.to_string()))?
            .into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLoadReceiptV1;

    #[test]
    fn load_fixture_verifies_exact_receipt_height_and_event() {
        let chain = crate::chain_fixture::new(&crate::ipa::Gens::<iroha_pasta::Eq>::new(4096));
        let bootstrap = ExactOperationWitness::Bootstrap(Box::new(BootstrapWitness {
            scheme: chain.sender_context.root.scheme,
            root: chain.sender_context.root.root.clone(),
            relation: chain.sender_context.relation,
            input: chain.sender_bootstrap,
        }));
        let load = load100(&bootstrap).expect("actual fixture Load");
        let ExactOperationWitness::Load {
            step,
            receipt_original,
            finality_original,
        } = &load
        else {
            unreachable!()
        };
        let receipt = KagemushaWalletLoadReceiptV1::decode_canonical(&receipt_original).unwrap();
        let evidence = KagemushaWalletLoadFinalityV1::decode_canonical(&finality_original).unwrap();
        let fixture = NativeFinalityFixture::new_with_explicit_parameters();
        let mut verifier = SumeragiCommitVerifierV1::new(&fixture.verifier()).unwrap();
        evidence
            .verify_with(&mut verifier, &receipt)
            .expect("real checkpoint-rooted BLS and exact event inclusion");
        assert_eq!(receipt.block_height, 2);
        assert_eq!(step.ordinary, receipt.transcript().unwrap());
        assert_eq!(step.ordinary, chain.load.ordinary);
        assert_eq!(step.after_core, chain.load.after_core);
        assert_eq!(step.after_rest, chain.load.after_rest);
        let mut changed = receipt;
        changed.block_height += 1;
        let mut changed_evidence = evidence;
        changed_evidence.receipt_digest = changed.receipt_digest().unwrap();
        let mut fresh = SumeragiCommitVerifierV1::new(&fixture.verifier()).unwrap();
        let error = changed_evidence
            .verify_with(&mut fresh, &changed)
            .unwrap_err();
        assert!(
            error.to_string().contains("certificate height differs"),
            "{error}"
        );
    }

    /// `chain_fixture` builds its two-wallet Send independently and its own
    /// tests prove that Send against the complete relation. A fixture Send10
    /// to another wallet must reproduce it: the send chain and the statement
    /// name the Request's receiver wallet, not the payer's.
    #[test]
    fn send10_to_another_wallet_matches_the_independent_chain_fixture() {
        let chain = crate::chain_fixture::new(&crate::ipa::Gens::<iroha_pasta::Eq>::new(4096));
        let bootstrap = ExactOperationWitness::Bootstrap(Box::new(BootstrapWitness {
            scheme: chain.sender_context.root.scheme,
            root: chain.sender_context.root.root.clone(),
            relation: chain.sender_context.relation,
            input: chain.sender_bootstrap.clone(),
        }));
        let load = load100(&bootstrap).unwrap();
        let receiver = &chain.receiver_bootstrap.credential;
        assert_eq!(
            request_to(&load, receiver).unwrap(),
            chain.send.request.body.to_vec()
        );
        let send = send10_to(&load, receiver, chain.send.request.signature).unwrap();
        let ExactOperationWitness::Send(step) = &send else {
            unreachable!()
        };
        assert_eq!(step.request, chain.send.request);
        assert_eq!(step.before_core, chain.send.before_core);
        assert_eq!(step.before_rest, chain.send.before_rest);
        assert_eq!(step.after_core, chain.send.after_core);
        assert_eq!(step.after_rest, chain.send.after_rest);
        // The chain fixture states a wider time interval (upper 120); the
        // controls-off device Send states its single floor (110).
        assert_eq!(step.statement[..25], chain.send.statement[..25]);
        assert_eq!(step.statement[18..20], chain.receiver_bootstrap.core[5..7]);
        assert_ne!(step.statement[18..20], step.before_core[5..7]);
        // The self-transfer keeps its previous bytes: receiver and payer coincide.
        let own = send10(&load, [0; 64]).unwrap();
        let ExactOperationWitness::Send(own) = &own else {
            unreachable!()
        };
        assert_eq!(own.statement[18..20], own.before_core[5..7]);
    }

    fn device_key(q: &p256::Point) -> KagemushaDevicePublicKeyV1 {
        let mut sec1 = [0u8; 65];
        sec1[0] = 4;
        for (offset, value) in [(1, &q.x), (33, &q.y)] {
            let bytes = value.to_bytes_be();
            sec1[offset + 32 - bytes.len()..offset + 32].copy_from_slice(&bytes);
        }
        KagemushaDevicePublicKeyV1::try_from(sec1.as_slice()).unwrap()
    }

    /// Two wallets through the real engine under the authenticated program
    /// bundle of a complete compiler run: A Bootstrap, A Load100, B Bootstrap,
    /// B-signed Request, A Send10 to B, B Receive. Wallet A is issued through
    /// the `bootstrap` entry a device uses, with a software key standing in
    /// for its hardware boundary. No platform custody is exercised here.
    #[test]
    #[ignore = "requires the authenticated full-backend-chain/native-device bundle"]
    fn two_wallet_send_and_receive_from_bundle() {
        fn run() -> Result<()> {
            use std::time::Instant;
            let started = Instant::now();
            let path = std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| {
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("output/full-backend-chain/native-device")
                });
            let read = |name: &str| {
                std::fs::read(path.join(name)).map_err(|e| Error::Rejected(e.to_string()))
            };
            let signature: [u8; 64] = read("parameter-signature.bin")?
                .try_into()
                .map_err(|_| Error::Malformed("test parameter signature"))?;
            let parameters = install(
                &read("program-bundle.norito")?,
                &read("parameter-manifest.norito")?,
                signature,
            )?;
            eprintln!(
                "two-wallet timing install: {:.3}s",
                started.elapsed().as_secs_f64()
            );
            let engine = NativeEngine::default();
            let advance = |label: &str,
                           sign: &dyn Fn(&PreparedOperation) -> Result<HardwareAdvanceReceipt>,
                           head: &VerifiedWalletHead,
                           witness: &ExactOperationWitness|
             -> Result<CompletedOperation> {
                let begun = Instant::now();
                let prepared = engine.prepare(witness, head, &parameters)?;
                let prepare = begun.elapsed().as_secs_f64();
                let receipt = sign(&prepared)?;
                let begun = Instant::now();
                let completed = engine.finalize(prepared, &receipt, &parameters)?;
                eprintln!(
                    "two-wallet timing {label}: prepare={prepare:.3}s finalize={:.3}s",
                    begun.elapsed().as_secs_f64()
                );
                Ok(completed)
            };

            // Wallet A, issued as on a device from an enrollment challenge and
            // a public payment key. Its asset differs from the vector's.
            let a_key = crate::full::WalletKey::labelled("two-wallet fixture wallet A");
            let a_sign = |prepared: &PreparedOperation| {
                sign_selected(prepared, &a_key.d, "two-wallet fixture wallet A receipt")
            };
            let challenge = KagemushaWalletEnrollmentChallengeV1 {
                version: 1,
                scheme_id: scheme().scheme_id(),
                asset_digest: [0xa1; 32],
                account_digest: [0xa2; 32],
                app_policy: [0xa3; 32],
                enrollment_policy: [0xa4; 32],
                issuer_nonce: [0xa5; 32],
            };
            let a_bootstrap = bootstrap(&parameters, &challenge, &device_key(&a_key.q))?;
            let ExactOperationWitness::Bootstrap(a_input) = &a_bootstrap else {
                unreachable!()
            };
            let a_origin = engine.enrollment_origin(a_input, [0x41; 32], &parameters)?;
            let a_enrolled = advance("A Bootstrap", &a_sign, &a_origin, &a_bootstrap)?;
            let a_load = load100(&a_bootstrap)?;
            let a_loaded = advance("A Load100", &a_sign, a_enrolled.head(), &a_load)?;

            // Wallet B, the software counterparty under A's scheme and asset.
            let b = Counterparty::new(&parameters, a_bootstrap.credential())?;
            let b_sign = |prepared: &PreparedOperation| b.sign(prepared);
            let b_origin = b.origin(&engine, &parameters)?;
            let b_enrolled = advance("B Bootstrap", &b_sign, &b_origin, b.bootstrap())?;

            // B makes the Request; A pays it.
            let request = request_to(&a_load, b.credential())?;
            let send = send10_to(&a_load, b.credential(), b.sign_request(&request)?)?;
            let sent = advance("A Send10 to B", &a_sign, a_loaded.head(), &send)?;
            let bytes = sent.completion().output_part(1)?;
            eprintln!(
                "two-wallet complete Payment: {} B, blake3 {}",
                bytes.len(),
                blake3::hash(&bytes).to_hex()
            );
            if let Some(output) = std::env::var_os("KAGEMUSHA_SPARTAN_TEST_OUTPUT") {
                let output = std::path::PathBuf::from(output);
                std::fs::create_dir_all(&output).map_err(|e| Error::Rejected(e.to_string()))?;
                std::fs::write(output.join("two-wallet-payment.ipm1"), &bytes)
                    .map_err(|e| Error::Rejected(e.to_string()))?;
            }

            // B verifies the exact Payment bytes and receives them.
            let begun = Instant::now();
            let payment = engine.verify_payment(&bytes, &parameters)?;
            eprintln!(
                "two-wallet timing B verifies Payment: {:.3}s",
                begun.elapsed().as_secs_f64()
            );
            let receive = receive_from(b.bootstrap(), b_enrolled.head().profile(), &payment)?;
            let received = advance("B Receive", &b_sign, b_enrolled.head(), &receive)?;
            eprintln!(
                "two-wallet timing total: {:.3}s",
                started.elapsed().as_secs_f64()
            );

            let ExactOperationWitness::Send(sent_step) = &send else {
                unreachable!()
            };
            let ExactOperationWitness::Receive {
                step: received_step,
                ..
            } = &receive
            else {
                unreachable!()
            };
            let a_wallet = &a_bootstrap.credential().body[66..98];
            let b_wallet = &b.credential().body[66..98];
            assert_ne!(a_wallet, b_wallet, "two distinct wallets");
            assert_eq!(&payment.request().body[66..98], a_wallet, "A pays");
            assert_eq!(&payment.request().body[130..162], b_wallet, "B is paid");
            assert_eq!(
                payment.payer().body[34..66],
                [0xa1; 32],
                "A's enrolled asset"
            );
            assert_eq!(sent_step.after_core[9], Fp::from(90), "A keeps 90");
            assert_eq!(received_step.after_core[9], Fp::from(10), "B holds 10");
            let lineage = crate::bootstrap_context::LINEAGE_IO;
            let burned = lineage + crate::bootstrap_context::BURNED_TOTAL;
            let credit = lineage + crate::bootstrap_context::CREDIT_ROOT;
            assert_eq!(sent.head().profile()[burned], Fp::ZERO, "A burned nothing");
            assert_eq!(
                received.head().profile()[burned],
                Fp::ZERO,
                "B credited the Payment without Burn"
            );
            assert_ne!(
                received.head().profile()[credit],
                b_enrolled.head().profile()[credit],
                "B recorded the credit"
            );
            assert_eq!(
                received.head().profile()[crate::full::HEAD_IO],
                received_step.statement[15],
                "B's head is its Receive successor"
            );
            assert!(
                received.completion().output_part(1)?.is_empty(),
                "Receive emits no Payment"
            );
            assert!(
                bytes.len() <= crate::payment_wire::QR_BUDGET,
                "complete Payment {} exceeds {} bytes",
                bytes.len(),
                crate::payment_wire::QR_BUDGET
            );
            Ok(())
        }
        run().unwrap();
    }
}

/// Exact Request body for an isolated self-transfer. The caller obtains its
/// signature from the enrolled hardware through the canonical Request domain.
///
/// The completed-Request relation refuses payer == receiver wallet
/// (`request::append_completed`), so a Send built from this body is refused by
/// native preparation. Successor and two-wallet fixtures use [`request_to`].
pub fn request_self(load: &ExactOperationWitness) -> Result<Vec<u8>> {
    let ExactOperationWitness::Load { step, .. } = load else {
        return Err(Error::Malformed("fixture Request requires Load"));
    };
    request_to(load, &step.credential)
}

/// The exact wallet state a fixture Send10 spends from: the Core and Rest its
/// predecessor committed, that predecessor's statement (only the fixed context
/// fields are reused), the payer credential, and the adjusted burned total the
/// predecessor's head exposes as public lineage. The Send relation reads that
/// total from the verified predecessor, never from the Core, so the builder
/// must be given it: it becomes the statement's burned value and the
/// successor Core's burn field. This is witness material only; it confers no
/// authority.
#[derive(Clone, Copy)]
pub struct SendOrigin<'a> {
    core: &'a [Fp; 30],
    rest: &'a [Fp; 6],
    statement: &'a [Fp; 26],
    credential: &'a crate::credential::Credential,
    burned: Fp,
}
impl<'a> SendOrigin<'a> {
    /// The fixture Load100 that follows Bootstrap: nothing has been burned.
    pub fn after_load(load: &'a ExactOperationWitness) -> Result<Self> {
        let ExactOperationWitness::Load { step, .. } = load else {
            return Err(Error::Malformed("fixture Send origin requires Load"));
        };
        Ok(Self {
            core: &step.after_core,
            rest: &step.after_rest,
            statement: &step.statement,
            credential: &step.credential,
            burned: Fp::ZERO,
        })
    }
    /// A completed Receive under `head`, the public profile of the head that
    /// Receive produced. A Receive that burned an invalid Payment credited its
    /// Core balance and added the same amount to the head's burned total.
    pub fn after_receive(receive: &'a ExactOperationWitness, head: &[Fp; 64]) -> Result<Self> {
        let ExactOperationWitness::Receive { step, .. } = receive else {
            return Err(Error::Malformed("fixture Send origin requires Receive"));
        };
        Self::after_received(step, head)
    }
    pub(crate) fn after_received(
        step: &'a crate::receive::Witness,
        head: &[Fp; 64],
    ) -> Result<Self> {
        use crate::bootstrap_context::{BURNED_TOTAL, LINEAGE_IO, PENDING_ROOT};
        if head[crate::full::HEAD_IO] != step.statement[15]
            || head[LINEAGE_IO + PENDING_ROOT] != step.after_core[18]
        {
            return Err(Error::AuthorityMismatch("fixture Send predecessor head"));
        }
        Ok(Self {
            core: &step.after_core,
            rest: &step.after_rest,
            statement: &step.statement,
            credential: &step.credential,
            burned: head[LINEAGE_IO + BURNED_TOTAL],
        })
    }
}

/// Exact Request body paying 10 from the Load-shaped payer state to `receiver`
/// (its wallet, account and credential digest). Confers no authority.
pub fn request_to(
    load: &ExactOperationWitness,
    receiver: &crate::credential::Credential,
) -> Result<Vec<u8>> {
    Ok(request_from(&SendOrigin::after_load(load)?, receiver))
}

/// Exact Request body paying 10 from `origin` to `receiver`. The receiver's
/// payment key signs it; the Send relation does not verify that signature.
pub fn request_from(origin: &SendOrigin<'_>, receiver: &crate::credential::Credential) -> Vec<u8> {
    let mut body = [0u8; 418];
    body[0] = 1;
    body[2..130].copy_from_slice(&origin.credential.body[2..130]);
    body[130..194].copy_from_slice(&receiver.body[66..130]);
    body[194..210].copy_from_slice(&origin.core[12].to_repr()[..16]);
    body[210..242].copy_from_slice(&crate::credential::digest(receiver).to_repr());
    body[242..258].copy_from_slice(&10u128.to_le_bytes());
    body[258..290].copy_from_slice(&origin.rest[2].to_repr());
    body[306..314].copy_from_slice(&origin.core[27].to_repr()[..8]);
    body[314..346].copy_from_slice(&origin.rest[1].to_repr());
    body[346..354].copy_from_slice(&110u64.to_le_bytes());
    body[354] = 1;
    body[386..418].fill(0x66);
    body.to_vec()
}

/// Construct private Send10 witness material. It confers no authority: the
/// actual native preparation and hardware-selected proof validate every field.
pub fn send10(
    load: &ExactOperationWitness,
    request_signature: [u8; 64],
) -> Result<ExactOperationWitness> {
    let ExactOperationWitness::Load { step, .. } = load else {
        return Err(Error::Malformed("fixture Send requires Load"));
    };
    send10_to(load, &step.credential, request_signature)
}

/// Send10 witness material paying `receiver`; `request_signature` signs the
/// body from [`request_to`] with the same receiver.
pub fn send10_to(
    load: &ExactOperationWitness,
    receiver: &crate::credential::Credential,
    request_signature: [u8; 64],
) -> Result<ExactOperationWitness> {
    send10_from(&SendOrigin::after_load(load)?, receiver, request_signature)
}

/// Send10 witness material spending from `origin` and paying `receiver`;
/// `request_signature` signs the body from [`request_from`] with the same
/// origin and receiver. The statement and successor Core carry the origin's
/// adjusted burned total, and the send chain and statement name the Request's
/// receiver wallet, exactly as the Send relation derives them.
pub fn send10_from(
    origin: &SendOrigin<'_>,
    receiver: &crate::credential::Credential,
    request_signature: [u8; 64],
) -> Result<ExactOperationWitness> {
    let request = CompletedRequest {
        body: request_from(origin, receiver).try_into().unwrap(),
        signature: request_signature,
    };
    let terms = crate::request::commitments(&request).map_err(Error::Malformed)?;
    let receiver_wallet = crate::request::word::RECEIVER_WALLET.map(|word| terms.words[word]);
    let before = *origin.core;
    let rest = *origin.rest;
    let mut fields = *origin.statement;
    fields[9] = before[11] + Fp::ONE;
    fields[10] = before[13];
    fields[11] = before[22];
    fields[12] = origin.burned;
    fields[13] = before[18];
    fields[14] = crate::monetary_state::commitment(&before, &rest);
    fields[16] = Fp::from(3);
    fields[17] = terms.credit;
    fields[18..20].copy_from_slice(&receiver_wallet);
    fields[20] = before[12];
    fields[21] = Fp::from(10);
    fields[22] = Fp::ZERO;
    fields[23] = terms.digest;
    fields[24] = Fp::from(110);
    fields[25] = Fp::from(110);
    let pending_value =
        iroha_pasta::poseidon::hash_with_domain(crate::send_maps::PENDING_DOMAIN, &fields[17..24]);
    let (pending, pending_root) = crate::indexed_map::empty_insert(terms.credit, pending_value);
    if pending.old_root != before[18] {
        return Err(Error::Malformed(
            "fixture Send requires an empty pending map",
        ));
    }
    let fee_value = iroha_pasta::poseidon::hash_with_domain(
        crate::send_maps::FEE_DOMAIN,
        &[terms.credit, Fp::ZERO, Fp::ZERO],
    );
    let (fee, _) = crate::indexed_map::empty_insert(terms.credit, fee_value);
    let mut after = before;
    after[9] -= Fp::from(10);
    after[10] = origin.burned;
    after[11] += Fp::ONE;
    after[12] += Fp::ONE;
    after[15] = iroha_pasta::poseidon::hash_with_domain(
        crate::send_arithmetic::CHAIN_DOMAIN,
        &[
            before[15],
            terms.credit,
            receiver_wallet[0],
            receiver_wallet[1],
            before[12],
            Fp::from(10),
            Fp::ZERO,
            terms.digest,
        ],
    );
    after[18] = pending_root;
    after[28] = Fp::from(110);
    after[29] += Fp::ONE;
    fields[15] = crate::monetary_state::commitment(&after, &rest);
    Ok(ExactOperationWitness::Send(Box::new(
        crate::send::Witness {
            before_core: before,
            before_rest: rest,
            after_core: after,
            after_rest: rest,
            credential: origin.credential.clone(),
            request,
            fee_schedule: crate::fee_schedule::SignedFeeSchedule {
                body: [0; 191],
                signature: [0; 64],
            },
            quota: crate::send_controls::QuotaWitness::default(),
            lower: 110,
            upper: 110,
            statement: fields,
            receipt: unsigned_receipt(origin.credential, &fields, Fp::ZERO),
            maps: crate::send_maps::Witness { pending, fee },
        },
    )))
}

/// Private Receive witness for the exact self-transfer Payment. The ordinary
/// Runtime validates the complete Payment and the resulting recursive proof.
pub fn receive_self(
    bootstrap: &ExactOperationWitness,
    send: &ExactOperationWitness,
    payment: &VerifiedIncomingPayment,
) -> Result<ExactOperationWitness> {
    let ExactOperationWitness::Bootstrap(boot) = bootstrap else {
        return Err(Error::Malformed("fixture Bootstrap kind"));
    };
    let ExactOperationWitness::Send(send) = send else {
        return Err(Error::Malformed("fixture Send kind"));
    };
    let mut receiver = boot.input.clone();
    receiver.core = send.after_core;
    receiver.rest = send.after_rest;
    receiver.statement = send.statement;
    receiver.receipt = payment.receipt().clone();
    let key = crate::full::WalletKey {
        d: BigUint::from(1u8),
        q: super::point(&send.credential.body[130..195])?,
    };
    let mut actual_send = (**send).clone();
    actual_send.receipt = payment.receipt().clone();
    let mut witness = crate::receive_fixture::from_send(
        &receiver,
        &key,
        &actual_send,
        payment.exact_pair(),
        *payment.profile(),
    )
    .map_err(Error::Malformed)?;
    witness.receipt.signature.fill(0);
    Ok(ExactOperationWitness::Receive {
        step: Box::new(witness),
        incoming_payment: payment.exact_bytes().into(),
    })
}

/// Private Receive witness for a second wallet's first Receive after its own
/// Bootstrap. It is built from that wallet's Bootstrap state, the public
/// profile of its verified Bootstrap head and the verified incoming Payment
/// alone: the receiver never holds the payer's private Send witness. The
/// receipt stays unsigned until the receiver's payment key signs the selected
/// body.
pub fn receive_from(
    receiver: &ExactOperationWitness,
    receiver_head: &[Fp; 64],
    payment: &VerifiedIncomingPayment,
) -> Result<ExactOperationWitness> {
    let ExactOperationWitness::Bootstrap(boot) = receiver else {
        return Err(Error::Malformed("fixture Bootstrap kind"));
    };
    let key = crate::full::WalletKey {
        d: BigUint::from(1u8),
        q: super::point(&boot.input.credential.body[130..195])?,
    };
    // `receive_fixture::from_send` reads only the Request, payer credential,
    // statement and receipt, which are the Payment's own originals. The state,
    // map and control fields of this carrier are inert.
    let (pending, _) = crate::indexed_map::empty_insert(Fp::ONE, Fp::ZERO);
    let (fee, _) = crate::indexed_map::empty_insert(Fp::ONE, Fp::ZERO);
    let incoming = crate::send::Witness {
        before_core: [Fp::ZERO; 30],
        before_rest: [Fp::ZERO; 6],
        after_core: [Fp::ZERO; 30],
        after_rest: [Fp::ZERO; 6],
        credential: payment.payer().clone(),
        request: payment.request().clone(),
        fee_schedule: crate::fee_schedule::SignedFeeSchedule {
            body: [0; 191],
            signature: [0; 64],
        },
        quota: crate::send_controls::QuotaWitness::default(),
        lower: 0,
        upper: 0,
        statement: *payment.statement(),
        receipt: payment.receipt().clone(),
        maps: crate::send_maps::Witness { pending, fee },
    };
    let mut witness = crate::receive_fixture::from_send(
        &boot.input,
        &key,
        &incoming,
        payment.exact_pair(),
        *receiver_head,
    )
    .map_err(Error::Malformed)?;
    witness.receipt.signature.fill(0);
    Ok(ExactOperationWitness::Receive {
        step: Box::new(witness),
        incoming_payment: payment.exact_bytes().into(),
    })
}

/// Software stand-in for a fixture wallet's hardware boundary. The engine has
/// already validated and frozen `prepared`; this signs its exact selected
/// receipt message with a public fixture scalar, and Native verifies the result
/// under the prepared wallet's payment key.
fn sign_selected(
    prepared: &PreparedOperation,
    d: &BigUint,
    label: &str,
) -> Result<HardwareAdvanceReceipt> {
    let signature = bootstrap::sign(super::field(&prepared.signing_message())?, d, label);
    HardwareAdvanceReceipt::verify_selected(prepared, prepared.receipt_body(), signature)
}

/// Enrollment marker of the software counterparty wallet's Bootstrap.
const COUNTERPARTY_MARKER: [u8; 32] = [0x42; 32];

/// Second wallet (B) of the isolated two-wallet flow:
/// A Bootstrap, A Load100, B Bootstrap, B-signed Request, A Send10 to B,
/// B Receive. A device holds one hardware payment key, so B's key is a public
/// deterministic fixture scalar. B guards nothing and its signatures stand in
/// only for a second device's hardware boundary. B's Bootstrap and Receive are
/// still prepared, proved and verified by the real engine under the installed
/// programs, and B's Request signature is checked inside its Receive relation.
pub struct Counterparty {
    key: crate::full::WalletKey,
    bootstrap: ExactOperationWitness,
}
impl Counterparty {
    /// Wallet B, enrolled by the fixture issuer under the payer's scheme and
    /// asset. A Payment between wallets of different assets is burned.
    pub fn new(
        parameters: &AuthenticatedParameters,
        payer: &crate::credential::Credential,
    ) -> Result<Self> {
        if payer.body[2..34] != parameters.inner.scheme {
            return Err(Error::AuthorityMismatch("isolated fixture scheme"));
        }
        let key = crate::full::WalletKey::labelled("isolated fixture counterparty wallet B");
        let asset: [u8; 32] = payer.body[34..66].try_into().unwrap();
        let bootstrap = issue(parameters, &key, &[(34, asset)])?;
        Ok(Self { key, bootstrap })
    }
    /// B's unsigned Bootstrap witness; also the state its first Receive spends from.
    pub fn bootstrap(&self) -> &ExactOperationWitness {
        &self.bootstrap
    }
    pub fn credential(&self) -> &crate::credential::Credential {
        self.bootstrap.credential()
    }
    /// B's authenticated enrollment origin, the predecessor of its Bootstrap.
    pub fn origin(
        &self,
        engine: &NativeEngine,
        parameters: &AuthenticatedParameters,
    ) -> Result<VerifiedWalletHead> {
        let ExactOperationWitness::Bootstrap(input) = &self.bootstrap else {
            return Err(Error::Malformed("fixture Bootstrap kind"));
        };
        engine.enrollment_origin(input, COUNTERPARTY_MARKER, parameters)
    }
    /// B makes the Request: its payment key signs the exact body that
    /// [`request_to`] or [`request_from`] built for B's credential.
    pub fn sign_request(&self, body: &[u8]) -> Result<[u8; 64]> {
        let request = CompletedRequest {
            body: body
                .try_into()
                .map_err(|_| Error::Malformed("fixture Request body"))?,
            signature: [0; 64],
        };
        let own = self.credential();
        if request.body[130..194] != own.body[66..130]
            || request.body[210..242] != crate::credential::digest(own).to_repr()
        {
            return Err(Error::AuthorityMismatch("fixture Request receiver"));
        }
        Ok(bootstrap::sign(
            crate::request::commitments(&request)
                .map_err(Error::Malformed)?
                .message,
            &self.key.d,
            "isolated fixture counterparty Request",
        ))
    }
    /// B's stand-in hardware signature over an operation the engine prepared
    /// for B. The caller finalizes with the returned receipt.
    pub fn sign(&self, prepared: &PreparedOperation) -> Result<HardwareAdvanceReceipt> {
        sign_selected(
            prepared,
            &self.key.d,
            "isolated fixture counterparty receipt",
        )
    }
}
