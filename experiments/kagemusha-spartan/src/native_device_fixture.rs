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
    let (context, mut input) = crate::bootstrap::fixture(&hardware, &parameters.inner.generators.p);
    if context.relation != parameters.id {
        return Err(Error::AuthorityMismatch("fixture relation"));
    }
    let c = &mut input.credential;
    for (offset, bytes) in [
        (34, challenge.asset_digest),
        (66, challenge.wallet_id(key)),
        (98, challenge.account_digest),
        (404, challenge.app_policy),
        (448, challenge.enrollment_id(key)),
    ] {
        c.body[offset..offset + 32].copy_from_slice(&bytes);
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
}

/// Exact Request body for an isolated self-transfer. The caller obtains its
/// signature from the enrolled hardware through the canonical Request domain.
pub fn request_self(load: &ExactOperationWitness) -> Result<Vec<u8>> {
    let ExactOperationWitness::Load { step, .. } = load else {
        return Err(Error::Malformed("fixture Request requires Load"));
    };
    let mut body = [0u8; 418];
    body[0] = 1;
    body[2..130].copy_from_slice(&step.credential.body[2..130]);
    body[130..194].copy_from_slice(&step.credential.body[66..130]);
    body[194..210].copy_from_slice(&step.after_core[12].to_repr()[..16]);
    body[210..242].copy_from_slice(&crate::credential::digest(&step.credential).to_repr());
    body[242..258].copy_from_slice(&10u128.to_le_bytes());
    body[258..290].copy_from_slice(&step.after_rest[2].to_repr());
    body[306..314].copy_from_slice(&step.after_core[27].to_repr()[..8]);
    body[314..346].copy_from_slice(&step.after_rest[1].to_repr());
    body[346..354].copy_from_slice(&110u64.to_le_bytes());
    body[354] = 1;
    body[386..418].fill(0x66);
    Ok(body.to_vec())
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
    let request = CompletedRequest {
        body: request_self(load)?.try_into().unwrap(),
        signature: request_signature,
    };
    let terms = crate::request::commitments(&request).map_err(Error::Malformed)?;
    let before = step.after_core;
    let mut fields = step.statement;
    fields[9] = before[11] + Fp::ONE;
    fields[10] = before[13];
    fields[11] = before[22];
    fields[12] = Fp::ZERO;
    fields[13] = before[18];
    fields[14] = crate::monetary_state::commitment(&before, &step.after_rest);
    fields[16] = Fp::from(3);
    fields[17] = terms.credit;
    fields[18..20].copy_from_slice(&before[5..7]);
    fields[20] = before[12];
    fields[21] = Fp::from(10);
    fields[22] = Fp::ZERO;
    fields[23] = terms.digest;
    fields[24] = Fp::from(110);
    fields[25] = Fp::from(110);
    let pending_value =
        iroha_pasta::poseidon::hash_with_domain(crate::send_maps::PENDING_DOMAIN, &fields[17..24]);
    let (pending, pending_root) = crate::indexed_map::empty_insert(terms.credit, pending_value);
    let fee_value = iroha_pasta::poseidon::hash_with_domain(
        crate::send_maps::FEE_DOMAIN,
        &[terms.credit, Fp::ZERO, Fp::ZERO],
    );
    let (fee, _) = crate::indexed_map::empty_insert(terms.credit, fee_value);
    let mut after = before;
    after[9] -= Fp::from(10);
    after[11] += Fp::ONE;
    after[12] += Fp::ONE;
    after[15] = iroha_pasta::poseidon::hash_with_domain(
        crate::send_arithmetic::CHAIN_DOMAIN,
        &[
            before[15],
            terms.credit,
            before[5],
            before[6],
            before[12],
            Fp::from(10),
            Fp::ZERO,
            terms.digest,
        ],
    );
    after[18] = pending_root;
    after[28] = Fp::from(110);
    after[29] += Fp::ONE;
    fields[15] = crate::monetary_state::commitment(&after, &step.after_rest);
    Ok(ExactOperationWitness::Send(Box::new(
        crate::send::Witness {
            before_core: before,
            before_rest: step.after_rest,
            after_core: after,
            after_rest: step.after_rest,
            credential: step.credential.clone(),
            request,
            fee_schedule: crate::fee_schedule::SignedFeeSchedule {
                body: [0; 191],
                signature: [0; 64],
            },
            quota: crate::send_controls::QuotaWitness::default(),
            lower: 110,
            upper: 110,
            statement: fields,
            receipt: unsigned_receipt(&step.credential, &fields, Fp::ZERO),
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
