//! Non-shipping isolated-device fixture. Its fixed public issuer is never
//! production trust. Actual device custody still performs every hardware receipt.
use super::*;
use ff::{Field, PrimeField};
use iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLoadReceiptV1;
use iroha_data_model::{
    kagemusha::*,
    sumeragi_finality::{SumeragiCommitVerifierV1, test_fixtures::NativeFinalityFixture},
};

/// Public secret of the fixture scheme root; [`scheme`] publishes its point.
/// The root signs only certificates.
const ROOT_SECRET: [u8; 32] = [0x11; 32];
/// Public secret of the fixture's Artifact-role signer, which the root
/// certifies and which signs parameter manifests.
const ARTIFACT_SECRET: [u8; 32] = [0x15; 32];

/// Isolated future scheme on the same deterministic signed genesis used to
/// finalize fixture Loads. Its relation is an explicit fixture identity.
pub fn scheme() -> KagemushaWalletSchemeV1 {
    static SCHEME: std::sync::OnceLock<KagemushaWalletSchemeV1> = std::sync::OnceLock::new();
    SCHEME
        .get_or_init(|| {
            let fixture =
                NativeFinalityFixture::start_with_explicit_parameters("portable-native-fixture");
            let root =
                p256::mul(&p256::generator(), &BigUint::from_bytes_be(&ROOT_SECRET)).unwrap();
            let mut key = [0; 65];
            key[0] = 4;
            key[1..33].copy_from_slice(&crate::certificate::scalar_bytes(&root.x));
            key[33..65].copy_from_slice(&crate::certificate::scalar_bytes(&root.y));
            KagemushaWalletSchemeV1 {
                version: 1,
                network_id: *fixture.network_id().as_bytes(),
                scheme_root_key: KagemushaDevicePublicKeyV1::try_from(key.as_slice()).unwrap(),
                relation_id: [0x33; 32],
                provider_contract: kagemusha_wallet_provider_contract_v1(),
            }
        })
        .clone()
}

pub(crate) fn enrollment_certificate() -> crate::certificate::Certificate {
    static CERT: std::sync::OnceLock<crate::certificate::Certificate> = std::sync::OnceLock::new();
    CERT.get_or_init(|| {
        let issuer = p256::mul(&p256::generator(), &BigUint::from_bytes_be(&[0x26; 32])).unwrap();
        crate::certificate::issue_fixture(
            &scheme().scheme_id(),
            crate::certificate::Role::Enrollment as u8,
            &issuer,
            1,
            &BigUint::from_bytes_be(&ROOT_SECRET),
        )
    })
    .clone()
}

#[cfg(test)]
pub(crate) fn chain(generators: &crate::ipa::Gens<iroha_pasta::Eq>) -> crate::chain_fixture::Chain {
    crate::chain_fixture::for_scope(generators, enrollment_certificate(), scheme().relation_id)
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
    let mut parameters =
        AuthenticatedParameters::install(bundle, manifest, signature, &scheme(), finality)?;
    parameters.confirmation = ConfirmationScope::FixtureSend;
    Ok(parameters)
}

/// Re-issue an authenticated device bundle whose root-signed manifest is in
/// the retired v1 format, for its exact unchanged programs: `source` holds
/// `program-bundle.norito`, `parameter-manifest.norito` and
/// `parameter-signature.bin`; `destination` must be a new empty directory.
/// It receives a byte-identical program bundle and a current-format manifest
/// (same scheme, relation, programs, dimensions and finality scope, plus the
/// generator/table profile and the root-issued Artifact-role certificate)
/// signed by the fixture's Artifact-role key, and the result must install.
/// The source directory is never written.
pub fn reissue_bundle(source: &std::path::Path, destination: &std::path::Path) -> Result<()> {
    let io = |e: std::io::Error| Error::Rejected(e.to_string());
    if std::fs::read_dir(destination).map_err(io)?.next().is_some() {
        return Err(Error::Rejected("re-issue destination is not empty".into()));
    }
    let read = |name: &str| std::fs::read(source.join(name)).map_err(io);
    let bundle = read("program-bundle.norito")?;
    let retired = read("parameter-manifest.norito")?;
    let retired_signature: [u8; 64] = read("parameter-signature.bin")?
        .try_into()
        .map_err(|_| Error::Malformed("retired parameter signature"))?;
    let (manifest, signature) = super::engine::reissue_fixture_manifest(
        &bundle,
        &retired,
        retired_signature,
        &ROOT_SECRET,
        &ARTIFACT_SECRET,
    )?;
    install(&bundle, &manifest, signature)?;
    for (name, bytes) in [
        ("program-bundle.norito", bundle.as_slice()),
        ("parameter-manifest.norito", manifest.as_slice()),
        ("parameter-signature.bin", signature.as_slice()),
    ] {
        std::fs::write(destination.join(name), bytes).map_err(io)?;
    }
    Ok(())
}
fn enrollment_marker(
    challenge: &KagemushaWalletEnrollmentChallengeV1,
    key: &KagemushaDevicePublicKeyV1,
) -> Result<[u8; 32]> {
    KagemushaWalletMarkerV1::enrollment(challenge, *key)
        .and_then(|marker| marker.marker_digest())
        .map_err(|_| Error::AuthorityMismatch("fixture enrollment marker"))
}

fn device_key(q: &p256::Point) -> KagemushaDevicePublicKeyV1 {
    let mut sec1 = [0u8; 65];
    sec1[0] = 4;
    sec1[1..33].copy_from_slice(&crate::certificate::scalar_bytes(&q.x));
    sec1[33..].copy_from_slice(&crate::certificate::scalar_bytes(&q.y));
    KagemushaDevicePublicKeyV1::try_from(sec1.as_slice()).expect("fixture P-256 public point")
}

/// The shared wallet vectors' canonical asset scope.
#[cfg(test)]
pub(crate) fn vector_asset() -> KagemushaWalletAssetScopeV1 {
    let vectors: norito::json::Value = norito::json::from_str(include_str!(
        "../../../fixtures/kagemusha/wallet_v1_vectors.json"
    ))
    .unwrap();
    let hex = vectors["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["type"].as_str() == Some("KagemushaWalletAssetScopeV1"))
        .unwrap()["canonical_hex"]
        .as_str()
        .unwrap();
    let bytes: Vec<u8> = (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect();
    norito::decode_canonical_with_limits(&bytes, norito::canonical_decode_limits(bytes.len()))
        .unwrap()
}

/// Issue the same enrolled fixture wallet to the governed Activation and Unload tests.
#[cfg(test)]
pub(crate) fn registered_bootstrap(
    parameters: &AuthenticatedParameters,
    scope: &KagemushaWalletAssetScopeV1,
    account: &iroha_data_model::account::AccountId,
) -> (crate::full::WalletKey, ExactOperationWitness, [u8; 32]) {
    let key = crate::full::WalletKey::labelled("installed Activation fixture wallet");
    let public = device_key(&key.q);
    let challenge = KagemushaWalletEnrollmentChallengeV1 {
        version: 1,
        scheme_id: scheme().scheme_id(),
        asset_digest: scope.asset_digest(),
        account_digest: kagemusha_wallet_account_digest_v1(account).unwrap(),
        app_policy: [0x61; 32],
        enrollment_policy: [0x62; 32],
        issuer_nonce: [0x63; 32],
    };
    let operation = bootstrap(parameters, &challenge, &public).unwrap();
    let marker = enrollment_marker(&challenge, &public).unwrap();
    (key, operation, marker)
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
        enrollment_marker(challenge, key)?,
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
    marker: [u8; 32],
    fields: &[(usize, [u8; 32])],
) -> Result<ExactOperationWitness> {
    let (context, mut input) = crate::bootstrap::fixture_with_certificate(
        key,
        &parameters.inner.generators.p,
        enrollment_certificate(),
        scheme().relation_id,
    );
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
    input.statement[19..21].copy_from_slice(&limbs(&marker));
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
    let ExactOperationWitness::Bootstrap(boot) = bootstrap else {
        return Err(Error::Malformed("fixture Load requires Bootstrap input"));
    };
    let (receipt, finality) = certified_loads(&[(&boot.input.credential, 1)])?
        .pop()
        .ok_or(Error::Malformed("fixture missing Load event"))?;
    load_from_receipt(
        &boot.input,
        &receipt,
        &finality,
        crate::indexed_map::empty_insert,
    )
}

/// Certify all requested ordinal events in one checkpoint successor. Every
/// ordinal has a distinct transaction/request and its own Merkle inclusion.
fn certified_loads(
    wallets: &[(&Credential, u8)],
) -> Result<Vec<(KagemushaWalletLoadReceiptV1, KagemushaWalletLoadFinalityV1)>> {
    use iroha_data_model::{
        events::{
            EventBox,
            data::{DataEvent, kagemusha::KagemushaLoadCommittedV1},
        },
        prelude::{HashOf, MerkleTree},
        sumeragi_finality::SumeragiCommitCertificateV1,
    };
    let mut consensus =
        NativeFinalityFixture::start_with_explicit_parameters("portable-native-fixture");
    let header = consensus.next_header();
    let height = header.height.get();
    let receipts: Vec<_> = wallets
        .iter()
        .enumerate()
        .flat_map(|(group, &(c, count))| {
            (0..count).map(move |ordinal| {
                let mut request_id = [0x45; 32];
                let mut transaction_hash = [0x56; 32];
                request_id[30] ^= u8::try_from(group).expect("isolated fixture wallet count");
                request_id[31] ^= ordinal;
                transaction_hash[30] ^= u8::try_from(group).expect("isolated fixture wallet count");
                transaction_hash[31] ^= ordinal;
                KagemushaWalletLoadReceiptV1 {
                    version: 1,
                    scheme_id: c.body[2..34].try_into().unwrap(),
                    asset_digest: c.body[34..66].try_into().unwrap(),
                    wallet_id: c.body[66..98].try_into().unwrap(),
                    request_id,
                    ordinal: u128::from(ordinal),
                    amount: 100,
                    online_charge: 0,
                    charge_quote: [0; 32],
                    transaction_hash,
                    block_height: height,
                    payer_account_digest: c.body[98..130].try_into().unwrap(),
                }
            })
        })
        .collect();
    let events: Vec<EventBox> = receipts
        .iter()
        .map(|receipt| {
            KagemushaLoadCommittedV1::from_receipt(receipt)
                .map(|event| EventBox::Data(DataEvent::KagemushaLoadCommitted(event).into()))
                .map_err(|e| Error::Rejected(e.to_string()))
        })
        .collect::<Result<_>>()?;
    let tree: MerkleTree<EventBox> = events.iter().map(HashOf::new).collect();
    let block = consensus.block_with_submitted_work(header);
    let proof = consensus.certify_with_events(block, &events);
    let certificate = SumeragiCommitCertificateV1::from_proof(&proof)
        .map_err(|e| Error::Rejected(e.to_string()))?;
    receipts
        .into_iter()
        .enumerate()
        .map(|(i, receipt)| {
            let finality = KagemushaWalletLoadFinalityV1 {
                version: 1,
                receipt_digest: receipt
                    .receipt_digest()
                    .map_err(|e| Error::Rejected(e.to_string()))?,
                certificate: certificate.clone(),
                event_proof: tree
                    .get_proof(i as u32)
                    .ok_or(Error::Malformed("fixture Load event"))?,
            };
            Ok((receipt, finality))
        })
        .collect()
}

fn load_from_receipt(
    input: &crate::bootstrap::Inputs,
    receipt: &KagemushaWalletLoadReceiptV1,
    finality: &KagemushaWalletLoadFinalityV1,
    insert: impl FnOnce(Fp, Fp) -> (crate::indexed_map::InsertWitness<32>, Fp),
) -> Result<ExactOperationWitness> {
    let c = &input.credential;
    let ordinary = receipt
        .transcript()
        .map_err(|e| Error::Rejected(e.to_string()))?;
    let digest = crate::load::ordinary_digest(&ordinary);
    let recovery_value = iroha_pasta::poseidon::hash_with_domain(
        crate::load::RECOVERY_DOMAIN,
        &[
            Fp::from_u128(receipt.ordinal),
            digest,
            Fp::from_u128(receipt.amount),
        ],
    );
    let (recovery, root) = insert(
        Fp::from_u128(1u128 << 127).double() + Fp::from_u128(receipt.ordinal),
        recovery_value,
    );
    let mut after = input.core;
    after[9] += Fp::from_u128(receipt.amount);
    after[11] += Fp::ONE;
    after[13] += Fp::ONE;
    after[19] = root;
    after[29] += Fp::ONE;
    let mut fields = input.statement;
    fields[8] = after[0];
    fields[9] = after[11];
    fields[10] = after[13];
    fields[11] = after[22];
    fields[12..14].fill(Fp::ZERO);
    fields[14] = crate::monetary_state::commitment(&input.core, &input.rest);
    fields[15] = crate::monetary_state::commitment(&after, &input.rest);
    fields[16] = Fp::from(2);
    fields[17] = digest;
    fields[18] = Fp::from_u128(receipt.ordinal);
    fields[19] = Fp::from_u128(receipt.amount);
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
#[path = "native_control_lifecycle_tests.rs"]
mod control_lifecycle;

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLoadReceiptV1;

    #[test]
    #[ignore = "compares every public fixed-row commitment of the signed device bundle"]
    fn signed_fixed_row_prefixes_match_dense_commitments() {
        use crate::ipa::{Aff, F, Gens};
        use crate::r1cs::Circuit;
        use crate::uniform::UniformCircuit;
        use group::Curve;
        use iroha_pasta::PastaCurve;
        use iroha_pasta::msm::{MemoryBudget, msm_public};

        fn check<C: PastaCurve>(program: &UniformCircuit<F<C>>, gens: &Gens<C>, kind: usize) {
            let shape = &program.shape;
            let count = program.fixed_rows(shape) - program.wallet_rows();
            let active = shape.active_cols();
            let expanded = program.fixed_values(shape);
            assert_eq!(expanded.len(), count * shape.cols());
            let lengths: Vec<_> = (0..count)
                .map(|k| program.fixed_commitment_prefix(shape, k).len())
                .collect();
            let dense = || -> Vec<Aff<C>> {
                expanded
                    .chunks_exact(shape.cols())
                    .map(|row| {
                        msm_public::<C>(&row[..active], &gens.g[..active], MemoryBudget::DEFAULT)
                            .unwrap()
                            .to_affine()
                    })
                    .collect()
            };
            let prefix = || crate::spartan::fixed_commitments(shape, gens, program);
            let time = |f: &dyn Fn() -> Vec<Aff<C>>| {
                let start = std::time::Instant::now();
                let result = f();
                (result, start.elapsed().as_micros())
            };
            for prefix_first in [false, true] {
                let (reference, actual) = if prefix_first {
                    let actual = time(&prefix);
                    (time(&dense), actual)
                } else {
                    (time(&dense), time(&prefix))
                };
                assert_eq!(reference.0, actual.0);
                eprintln!("signed fixed rows {} kind={kind} rows={count} cols={} active={active} prefix_lengths={lengths:?} prefix_first={prefix_first} dense_us={} prefix_us={}",
                    shape.label, shape.cols(), reference.1, actual.1);
            }
        }

        let path = std::path::PathBuf::from(
            std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap(),
        );
        let read = |name: &str| std::fs::read(path.join(name)).unwrap();
        let parameters = install(
            &read("program-bundle.norito"),
            &read("parameter-manifest.norito"),
            read("parameter-signature.bin").try_into().unwrap(),
        )
        .unwrap();
        for (k, program) in parameters.inner.programs.iter().enumerate() {
            check(&program.primary, &parameters.inner.generators.p, k);
            check(&program.secondary, &parameters.inner.generators.s, k);
        }
    }

    #[test]
    fn load_fixture_verifies_exact_receipt_height_and_event() {
        let chain = chain(&crate::ipa::Gens::<iroha_pasta::Eq>::new(4096));
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
        let chain = chain(&crate::ipa::Gens::<iroha_pasta::Eq>::new(4096));
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
    }

    #[test]
    #[ignore = "requires the three-mode fixed-point signed program bundle"]
    fn signed_time_anchor_review_from_bundle() {
        let path = std::path::PathBuf::from(
            std::env::var_os("KAGEMUSHA_SPARTAN_DEVICE_ARTIFACTS").unwrap(),
        );
        let read = |name: &str| std::fs::read(path.join(name)).unwrap();
        let parameters = install(
            &read("program-bundle.norito"),
            &read("parameter-manifest.norito"),
            read("parameter-signature.bin").try_into().unwrap(),
        )
        .unwrap();
        let engine = NativeEngine::default();
        let account = iroha_data_model::account::AccountId::new(
            "ed0120D75A980182B10AB7D54BFED3C964073A0EE172F3DAA62325AF021A68F707511A"
                .parse()
                .unwrap(),
        );
        let (key, boot, marker) = registered_bootstrap(&parameters, &vector_asset(), &account);
        let ExactOperationWitness::Bootstrap(b) = &boot else {
            unreachable!()
        };
        let origin = engine.enrollment_origin(b, marker, &parameters).unwrap();
        let prepared = engine.prepare(&boot, &origin, &parameters).unwrap();
        let signed = sign_selected(&prepared, &key.d, "signed floor test bootstrap").unwrap();
        let enrolled = engine.finalize(prepared, &signed, &parameters).unwrap();
        let load = load100(&boot).unwrap();
        let prepared = engine.prepare(&load, enrolled.head(), &parameters).unwrap();
        let signed = sign_selected(&prepared, &key.d, "signed floor test Load100").unwrap();
        let loaded = engine.finalize(prepared, &signed, &parameters).unwrap();
        let ExactOperationWitness::Load {
            step: load_step, ..
        } = &load
        else {
            unreachable!()
        };
        let step = crate::control::tests::recursive_refresh(
            &load_step.after_core,
            &load_step.after_rest,
            loaded.head().profile(),
            &key,
            &b.input,
        );
        let mut changed = step.clone();
        changed.authority.anchor.signature[3] ^= 1;
        assert!(
            engine
                .prepare(
                    &ExactOperationWitness::RefreshTimeAnchor(Box::new(changed)),
                    loaded.head(),
                    &parameters
                )
                .is_err()
        );
        let mut jumped = step.clone();
        jumped.after_core[28] += Fp::ONE;
        assert!(
            engine
                .prepare(
                    &ExactOperationWitness::RefreshTimeAnchor(Box::new(jumped)),
                    loaded.head(),
                    &parameters
                )
                .is_err()
        );
        let expected_anchor = step.after_rest[5].to_repr();
        let expected_floor = step.after_core[28].to_repr();
        let signed_time = crate::time_anchor::issuer_time(&step.authority.anchor);
        let operation = ExactOperationWitness::RefreshTimeAnchor(Box::new(step));
        let original = operation.encode().unwrap();
        assert_eq!(
            ExactOperationWitness::decode(&original)
                .unwrap()
                .encode()
                .unwrap(),
            original
        );
        let mut trailing = original.clone();
        trailing.push(0);
        assert!(ExactOperationWitness::decode(&trailing).is_err());
        let prepared = engine
            .prepare(&operation, loaded.head(), &parameters)
            .unwrap();
        let capsule = prepared.capsule().clone();
        let review = prepared.review_bytes().unwrap();
        assert_eq!(review.len(), 243);
        assert_eq!(&review[..3], &[2, 0, 7]);
        assert_eq!(&review[3..35], capsule.scheme());
        assert_eq!(&review[35..67], capsule.wallet());
        assert_eq!(&review[67..99], capsule.operation_id());
        assert_eq!(&review[99..131], prepared.capsule_digest());
        assert_eq!(&review[131..163], &expected_anchor);
        assert_eq!(&review[163..171], &signed_time.to_le_bytes());
        assert_eq!(&review[171..179], &expected_floor[..8]);
        assert_eq!(&review[179..211], capsule.predecessor());
        assert_eq!(&review[211..243], capsule.successor());
        assert_eq!(
            engine
                .resume(&capsule, &parameters)
                .unwrap()
                .review_bytes()
                .unwrap(),
            review
        );
        engine.before_selection(&capsule, &parameters).unwrap();
        let signed = sign_selected(&prepared, &key.d, "signed floor test refresh").unwrap();
        let completed = engine.finalize(prepared, &signed, &parameters).unwrap();
        assert_eq!(&completed.head().profile()[62..64], &[Fp::ZERO, Fp::ONE]);
        let verified = engine
            .verify_completion(&capsule, completed.completion(), &parameters)
            .unwrap();
        assert_eq!(
            verified.completion().encode().unwrap(),
            completed.completion().encode().unwrap()
        );
        assert!(completed.completion().output_part(1).unwrap().is_empty());
        assert!(
            engine
                .prepare(&operation, completed.head(), &parameters)
                .is_err()
        );
        let ExactOperationWitness::RefreshTimeAnchor(refresh) = &operation else {
            unreachable!()
        };
        engine::test_unload_claim(
            &parameters,
            completed.head(),
            refresh,
            load_step,
            &key,
            account,
        );
        eprintln!(
            "signed TimeAnchor v2 review {} bytes, signed time {}, resulting floor {}, exact completion {} bytes",
            review.len(),
            signed_time,
            u64::from_le_bytes(expected_floor[..8].try_into().unwrap()),
            completed.completion().encode().unwrap().len()
        );
    }

    /// Two wallets through the real engine under the authenticated program
    /// bundle of a complete compiler run: A Bootstrap, A Load100, B Bootstrap,
    /// B-signed Request, A Send10 to B, B Receive; then back: A-signed
    /// Request, B Send10 to A, A Receive after its own Send. The device runner
    /// runs this flow with A in platform custody. Here wallet A is issued
    /// through the `bootstrap` entry a device uses, with a software key
    /// standing in for its hardware boundary.
    #[test]
    #[ignore = "requires the authenticated full-backend-chain/native-device bundle"]
    fn two_wallet_round_trip_from_bundle() {
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
                // Wall-clock bounds let an external RSS sampler attribute
                // its samples to this operation.
                let unix_ms = || {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_millis())
                };
                let from = unix_ms();
                let begun = Instant::now();
                let prepared = engine.prepare(witness, head, &parameters)?;
                let prepare = begun.elapsed().as_secs_f64();
                let receipt = sign(&prepared)?;
                let begun = Instant::now();
                let completed = engine.finalize(prepared, &receipt, &parameters)?;
                eprintln!(
                    "two-wallet timing {label}: prepare={prepare:.3}s finalize={:.3}s unix_ms={from}..{}",
                    begun.elapsed().as_secs_f64(),
                    unix_ms()
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
            let a_origin = engine.enrollment_origin(
                a_input,
                enrollment_marker(&challenge, &device_key(&a_key.q))?,
                &parameters,
            )?;
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
            let receive = receive_from(
                b.bootstrap(),
                b.bootstrap(),
                b_enrolled.head().profile(),
                &payment,
            )?;
            let received = advance("B Receive", &b_sign, b_enrolled.head(), &receive)?;

            // Back: A makes the Request, B pays it from what it received, and
            // A receives after its own Send.
            let b_origin = SendOrigin::after_receive(&receive, received.head().profile())?;
            let back_request = CompletedRequest {
                body: request_from(&b_origin, a_bootstrap.credential())
                    .try_into()
                    .map_err(|_| Error::Malformed("fixture Request body"))?,
                signature: [0; 64],
            };
            let a_request_signature = bootstrap::sign(
                crate::request::commitments(&back_request)
                    .map_err(Error::Malformed)?
                    .message,
                &a_key.d,
                "two-wallet fixture wallet A Request",
            );
            let back = send10_from(&b_origin, a_bootstrap.credential(), a_request_signature)?;
            let b_sent = advance("B Send10 to A", &b_sign, received.head(), &back)?;
            let back_bytes = b_sent.completion().output_part(1)?;
            eprintln!(
                "two-wallet complete return Payment: {} B, blake3 {}",
                back_bytes.len(),
                blake3::hash(&back_bytes).to_hex()
            );
            if let Some(output) = std::env::var_os("KAGEMUSHA_SPARTAN_TEST_OUTPUT") {
                std::fs::write(
                    std::path::PathBuf::from(output).join("two-wallet-return-payment.ipm1"),
                    &back_bytes,
                )
                .map_err(|e| Error::Rejected(e.to_string()))?;
            }
            let begun = Instant::now();
            let back_payment = engine.verify_payment(&back_bytes, &parameters)?;
            eprintln!(
                "two-wallet timing A verifies Payment: {:.3}s",
                begun.elapsed().as_secs_f64()
            );
            let a_receive =
                receive_from(&a_bootstrap, &send, sent.head().profile(), &back_payment)?;
            let a_received = advance("A Receive", &a_sign, sent.head(), &a_receive)?;
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

            let ExactOperationWitness::Send(back_step) = &back else {
                unreachable!()
            };
            let ExactOperationWitness::Receive {
                step: a_received_step,
                ..
            } = &a_receive
            else {
                unreachable!()
            };
            assert_eq!(&back_payment.request().body[66..98], b_wallet, "B pays");
            assert_eq!(
                &back_payment.request().body[130..162],
                a_wallet,
                "A is paid"
            );
            assert_eq!(back_step.after_core[9], Fp::ZERO, "B spent its 10");
            assert_eq!(
                a_received_step.after_core[9],
                Fp::from(100),
                "A holds 90 + 10"
            );
            assert_eq!(
                b_sent.head().profile()[burned],
                Fp::ZERO,
                "B burned nothing"
            );
            assert_eq!(
                a_received.head().profile()[burned],
                Fp::ZERO,
                "A credited the return Payment without Burn"
            );
            require_credited(sent.head().profile(), a_received.head().profile())?;
            require_credited(b_enrolled.head().profile(), received.head().profile())?;
            assert_eq!(
                a_received.head().profile()[crate::full::HEAD_IO],
                a_received_step.statement[15],
                "A's head is its Receive successor"
            );
            assert!(
                a_received.completion().output_part(1)?.is_empty(),
                "Receive emits no Payment"
            );
            for (label, payment) in [("A to B", &bytes), ("B to A", &back_bytes)] {
                assert!(
                    payment.len() <= crate::payment_wire::QR_BUDGET,
                    "complete Payment {label} {} exceeds {} bytes",
                    payment.len(),
                    crate::payment_wire::QR_BUDGET
                );
            }
            Ok(())
        }
        run().unwrap();
    }
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
    body[354..386].copy_from_slice(
        &crate::request::certificate_set_digest(&[crate::certificate::certificate_digest(
            &enrollment_certificate(),
        )])
        .to_repr(),
    );
    body[386..418].fill(0x66);
    body.to_vec()
}

/// Send10 witness material paying `receiver`; `request_signature` signs the
/// body from [`request_to`] with the same receiver. It confers no authority:
/// the actual native preparation and hardware-selected proof validate every
/// field.
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
    send10_with_request(origin, receiver, request)
}

/// Complete the independently issued open Request with the payer's actual wallet,
/// account and next send ordinal. Only the isolated no-fee ten-unit scenario is admitted.
pub fn send10_from_open(
    origin: &SendOrigin<'_>,
    receiver: &crate::credential::Credential,
    wire: &[u8],
) -> Result<ExactOperationWitness> {
    let open = super::decode_open_request_peer(wire)?;
    if open.receiver_credential[..532] != receiver.body
        || open.receiver_credential[532..] != receiver.signature
        || !open.fee_schedule.is_empty()
        || open.request[242..258] != 10u128.to_le_bytes()
    {
        return Err(Error::AuthorityMismatch("fixture return Request terms"));
    }
    let mut request = CompletedRequest::from_tape(open.request).map_err(Error::Malformed)?;
    request.body[66..130].copy_from_slice(&origin.credential.body[66..130]);
    request.body[194..210].copy_from_slice(&origin.core[12].to_repr()[..16]);
    send10_with_request(origin, receiver, request)
}

fn send10_with_request(
    origin: &SendOrigin<'_>,
    receiver: &crate::credential::Credential,
    request: CompletedRequest,
) -> Result<ExactOperationWitness> {
    let terms = crate::request::commitments(&request).map_err(Error::Malformed)?;
    let receiver_wallet = crate::request::word::RECEIVER_WALLET.map(|word| terms.words[word]);
    let before = *origin.core;
    let rest = *origin.rest;
    let floor = u64::from_le_bytes(before[28].to_repr()[..8].try_into().unwrap());
    let accepted = u64::from_le_bytes(request.body[346..354].try_into().unwrap());
    let time = floor.max(accepted);
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
    fields[24] = Fp::from(time);
    fields[25] = Fp::from(time);
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
    after[28] = Fp::from(time);
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
            request_originals: crate::request::Originals {
                receiver: receiver.clone(),
                certificates: vec![enrollment_certificate()],
            },
            fee_schedule: crate::fee_schedule::SignedFeeSchedule {
                body: [0; 191],
                signature: [0; 64],
            },
            quota: crate::send_controls::QuotaWitness::default(),
            lower: time,
            upper: time,
            statement: fields,
            receipt: unsigned_receipt(origin.credential, &fields, Fp::ZERO),
            maps: crate::send_maps::Witness { pending, fee },
        },
    )))
}

/// Private Receive witness for a wallet's first Receive. `receiver` is that
/// wallet's Bootstrap witness and `latest` its latest completed operation: the
/// Bootstrap itself, a Load or a Send. `receiver_head` is the public profile
/// of the head `latest` produced. The witness is built from that state and the
/// verified incoming Payment alone: the receiver never holds the payer's
/// private Send witness. The receipt stays unsigned until the receiver's
/// payment key signs the selected body.
pub fn receive_from(
    receiver: &ExactOperationWitness,
    latest: &ExactOperationWitness,
    receiver_head: &[Fp; 64],
    payment: &VerifiedIncomingPayment,
) -> Result<ExactOperationWitness> {
    let ExactOperationWitness::Bootstrap(boot) = receiver else {
        return Err(Error::Malformed("fixture Bootstrap kind"));
    };
    let own = latest.credential();
    if own.body != boot.input.credential.body || own.signature != boot.input.credential.signature {
        return Err(Error::AuthorityMismatch("fixture Receive state wallet"));
    }
    // The receiver's current Core and Rest; `from_send` rewrites every
    // statement field after the fixed context it keeps.
    let mut state = boot.input.clone();
    match latest {
        ExactOperationWitness::Bootstrap(_) => {}
        ExactOperationWitness::Load { step, .. } => {
            (state.core, state.rest, state.statement) =
                (step.after_core, step.after_rest, step.statement);
        }
        ExactOperationWitness::RefreshTimeAnchor(step)
        | ExactOperationWitness::Unload { step, .. } => {
            (state.core, state.rest, state.statement) =
                (step.after_core, step.after_rest, step.statement);
        }
        ExactOperationWitness::Send(step) => {
            (state.core, state.rest, state.statement) =
                (step.after_core, step.after_rest, step.statement);
        }
        ExactOperationWitness::Receive { .. } => {
            return Err(Error::Malformed(
                "fixture Receive builds only a first Receive",
            ));
        }
    }
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
        request_originals: crate::request::Originals {
            receiver: boot.input.credential.clone(),
            certificates: vec![boot.input.certificate.clone()],
        },
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
        &state,
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

/// Check that a completed Receive credited its incoming Payment instead of
/// burning it. `before` is the public profile of the Receive's predecessor
/// head and `after` that of the head the Receive produced. Fresh Native admission
/// checks the incoming Payment before selection. A previously selected Receive
/// can carry the relation's Burn outcome. This fixture checks that the adjusted
/// burned total is unchanged and the credit root moved.
pub fn require_credited(before: &[Fp; 64], after: &[Fp; 64]) -> Result<()> {
    use crate::bootstrap_context::{BURNED_TOTAL, CREDIT_ROOT, LINEAGE_IO};
    if after[LINEAGE_IO + BURNED_TOTAL] != before[LINEAGE_IO + BURNED_TOTAL] {
        return Err(Error::Rejected(
            "fixture Receive burned the incoming Payment".into(),
        ));
    }
    if after[LINEAGE_IO + CREDIT_ROOT] == before[LINEAGE_IO + CREDIT_ROOT] {
        return Err(Error::Rejected("fixture Receive recorded no credit".into()));
    }
    Ok(())
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

fn software_bootstrap(
    parameters: &AuthenticatedParameters,
    key: &crate::full::WalletKey,
    asset_digest: [u8; 32],
    account_digest: [u8; 32],
    salt: u8,
) -> Result<(ExactOperationWitness, [u8; 32])> {
    let public = device_key(&key.q);
    let challenge = KagemushaWalletEnrollmentChallengeV1 {
        version: 1,
        scheme_id: scheme().scheme_id(),
        asset_digest,
        account_digest,
        app_policy: [salt; 32],
        enrollment_policy: [salt + 1; 32],
        issuer_nonce: [salt + 2; 32],
    };
    Ok((
        bootstrap(parameters, &challenge, &public)?,
        enrollment_marker(&challenge, &public)?,
    ))
}

/// Second wallet (B) of the isolated two-wallet flow:
/// A Bootstrap, A Load100, B Bootstrap, B-signed Request, A Send10 to B,
/// B Receive, A-signed Request, B Send10 to A, A Receive. A device holds one
/// hardware payment key, so B's key is a public deterministic fixture scalar.
/// B guards nothing and its signatures stand in only for a second device's
/// hardware boundary. B's Bootstrap, Receive and Send are still prepared,
/// proved and verified by the real engine under the installed programs, and
/// each receiver's Request signature is checked inside its Receive relation.
pub struct Counterparty {
    marker: [u8; 32],
    key: crate::full::WalletKey,
    bootstrap: ExactOperationWitness,
}
impl Counterparty {
    fn software_account() -> iroha_data_model::account::AccountId {
        // RFC 8032 public test vector, separate from the phone fixture's account and payment key.
        iroha_data_model::account::AccountId::new(
            "ed01203D4017C3E843895A92B70AA74D1B7EBC9C982CCF2EC4968CC0CD55F12AF4660C"
                .parse()
                .expect("fixture account public key"),
        )
    }
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
        let account = kagemusha_wallet_account_digest_v1(&Self::software_account())
            .map_err(|_| Error::Malformed("fixture counterparty account"))?;
        let (bootstrap, marker) = software_bootstrap(parameters, &key, asset, account, 0x42)?;
        Ok(Self {
            key,
            bootstrap,
            marker,
        })
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
        engine.enrollment_origin(input, self.marker, parameters)
    }
    /// The receiver's wire for an ordinary wallet-owned Send. The original open
    /// signature binds the same terms that appear in the completed fixture witness.
    pub fn open_request_wire(&self, completed: &[u8], signature: [u8; 64]) -> Result<Vec<u8>> {
        let mut body: [u8; 418] = completed
            .try_into()
            .map_err(|_| Error::Malformed("fixture open Request body"))?;
        body[66..130].fill(0);
        body[194..210].fill(0);
        let cert = enrollment_certificate();
        let credential = self.credential();
        let account = norito::encode_canonical(&Self::software_account())
            .map_err(|_| Error::Malformed("fixture counterparty account"))?;
        encode_open_request_peer(
            &[body.as_slice(), &signature].concat(),
            &[credential.body.as_slice(), &credential.signature].concat(),
            &[cert.body.as_slice(), &cert.signature].concat(),
            &[],
            &account,
        )
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
