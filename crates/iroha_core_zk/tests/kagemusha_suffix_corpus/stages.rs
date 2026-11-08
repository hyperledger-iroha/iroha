//! Canonical stage originals produced by the genuine continuation test for ledger tests.
use super::*;
use iroha_data_model::account::AccountId;
use iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::*;

// Use the shared model owner and verify exact parity with the source-frozen gate05 framing.
// The independent hash below is an engineering assertion, never a production decoder.
fn recovery_digest(openings: &[Vec<u8>]) -> [u8; 32] {
    let shared = kagemusha_wallet_suffix_recovery_witness_digest_v1(openings).unwrap();
    assert_eq!(
        openings.iter().map(Vec::len).collect::<Vec<_>>(),
        [1124, 1028]
    );
    let frame = norito::encode_canonical(&openings.to_vec()).unwrap();
    let mut digest = Sha256::new();
    digest.update(KAGEMUSHA_WALLET_SUFFIX_DIGEST_PREFIX_V1);
    digest.update(b"recovery-witness");
    digest.update([0]);
    digest.update((frame.len() as u64).to_le_bytes());
    digest.update(frame);
    let prior_gate_digest: [u8; 32] = digest.finalize().into();
    assert_eq!(
        shared, prior_gate_digest,
        "shared recovery-witness owner changed gate05 framing"
    );
    shared
}

pub(super) struct Stages {
    header: KagemushaWalletSuffixHeaderV1,
    anchor: KagemushaWalletSuffixAnchorV1,
    chunks: Vec<Vec<u8>>,
}

fn account(corpus: &Corpus) -> AccountId {
    ["account.norito", "account-b.norito", "account-c.norito"]
        .into_iter()
        .map(|name| {
            decode::<AccountId>(&bounded(&corpus.root.join("setup").join(name), 1024)).unwrap()
        })
        .find(|account| {
            kagemusha_wallet_account_digest_v1(account).unwrap()
                == corpus.credential.body.account_digest
        })
        .expect("the genuine fixture account matches the merchant credential")
}

fn step_frame(step: &StepOriginals) -> Vec<u8> {
    let credit = match &step.credit.1 {
        Some(empty) => KagemushaWalletSuffixCreditWitnessV1::Inserted {
            low: step.credit.0.clone(),
            empty: empty.clone(),
        },
        None => KagemushaWalletSuffixCreditWitnessV1::Present {
            membership: step.credit.0.clone(),
        },
    };
    let originals = [
        (KagemushaWalletSuffixOriginalRoleV1::Request, &step.request),
        (KagemushaWalletSuffixOriginalRoleV1::Payment, &step.payment),
        (
            KagemushaWalletSuffixOriginalRoleV1::PayerCredential,
            &step.payer_credential,
        ),
        (
            KagemushaWalletSuffixOriginalRoleV1::PayerCertificates,
            &step.payer_certificates,
        ),
    ]
    .into_iter()
    .map(|(role, bytes)| KagemushaWalletSuffixOriginalV1 {
        role,
        bytes: bytes.clone(),
    })
    .collect();
    KagemushaWalletSuffixStepV1 {
        version: 1,
        kind: KagemushaWalletSuffixStepKindV1::Receive { credit },
        before: step.before,
        after: step.after,
        statement: step.statement,
        sigma: step.sigma.clone(),
        receipt: step.receipt,
        map_openings: step.consumed.to_vec(),
        originals,
    }
    .to_canonical_bytes()
    .unwrap()
}

impl Stages {
    pub(super) fn prepare(
        corpus: &Corpus,
        steps: &[StepOriginals],
        points: &[KagemushaWalletLineagePublicV1],
        derivation: &SuffixUnloadDerivationV1,
        sigma: &KagemushaWalletStepProofV1,
    ) -> Self {
        assert_eq!(steps.len(), points.len());
        let certificates = corpus.certificates();
        let issuer = *certificates
            .certificate(
                &corpus.credential.body.issuer_certificate,
                KagemushaWalletSignerRoleV1::Enrollment,
            )
            .unwrap();
        let anchor = KagemushaWalletSuffixAnchorV1 {
            version: 1,
            credential: corpus.credential,
            certificates: KagemushaWalletCertificateSetV1::new(vec![issuer]).unwrap(),
            lineage: corpus.anchor.lineage.clone(),
            state: *corpus.state(0),
        };
        let anchor_bytes = anchor.to_canonical_bytes().unwrap();
        let mut offset = 0;
        let mut chunks = Vec::new();
        for (index, partition) in steps
            .chunks(KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1)
            .enumerate()
        {
            let frames: Vec<_> = partition.iter().map(step_frame).collect();
            let start_index = index * KAGEMUSHA_WALLET_SUFFIX_CHUNK_STEPS_MAX_V1;
            let chunk = KagemushaWalletSuffixChunkV1 {
                version: 1,
                chunk_index: index as u128,
                stream_offset: offset,
                start: if start_index == 0 {
                    corpus.anchor.lineage.public
                } else {
                    points[start_index - 1]
                },
                data: KagemushaWalletSuffixChunkV1::stream(&frames).unwrap(),
            };
            offset += chunk.data.len() as u128;
            chunks.push(chunk.to_canonical_bytes().unwrap());
        }
        let pack = bounded(
            &corpus.root.join("catalog/engineering-verifier-pack.norito"),
            VERIFIER_PACK_MAX_BYTES_V1,
        );
        let header = KagemushaWalletSuffixHeaderV1 {
            version: 1,
            method: KAGEMUSHA_WALLET_SUFFIX_METHOD_NATIVE_V1,
            network_id: corpus.installed.verifier().scheme().network_id,
            scheme_id: corpus.credential.body.scheme_id,
            wallet_id: corpus.credential.body.wallet_id,
            verifier_definition: kagemusha_wallet_suffix_verifier_definition_v1(&pack),
            artifact_manifest: corpus.installed.verifier().manifest_digest(),
            source_capsule: corpus.frozen[&(steps.len() as u128)]
                .capsule
                .custody_digest(),
            request_id: Sha256::digest(b"engineering-native-suffix-stage-gate-v1").into(),
            anchor_digest: kagemusha_wallet_suffix_digest_v1(
                KagemushaWalletSuffixDigestRoleV1::Anchor,
                &anchor_bytes,
            ),
            anchor_sequence: 0,
            step_count: steps.len() as u128,
            chunk_count: chunks.len() as u128,
            stream_bytes: offset,
            chunk_digests: chunks
                .iter()
                .map(|bytes| {
                    kagemusha_wallet_suffix_digest_v1(
                        KagemushaWalletSuffixDigestRoleV1::Chunk,
                        bytes,
                    )
                })
                .collect(),
            tip: derivation.predecessor_adjusted.head,
            tip_sequence: derivation.predecessor.core.sequence,
            tip_adjusted: derivation.predecessor_adjusted,
            statement: derivation.statement,
            sigma: sigma.clone(),
            recovery_witness_digest: recovery_digest(&derivation.openings),
            account: account(corpus),
            charge: KagemushaWalletUnloadChargeV1::None,
        };
        header.validate().unwrap();
        anchor.require_header(&header).unwrap();
        Self {
            header,
            anchor,
            chunks,
        }
    }

    pub(super) fn authority(&self) -> [u8; 32] {
        kagemusha_wallet_suffix_authority_digest_v1(&self.header.to_canonical_bytes().unwrap())
            .unwrap()
    }

    pub(super) fn publish(
        &self,
        corpus: &Corpus,
        verified: &VerifiedSuffixUnloadV1,
        openings: &[Vec<u8>],
    ) {
        assert_eq!(verified.authority_digest(), self.authority());
        let unload = KagemushaWalletSuffixUnloadV1 {
            version: 1,
            credential: *verified.credential(),
            certificates: self.anchor.certificates.clone(),
            after: *verified.successor().0,
            receipt: *verified.receipt(),
            map_openings: openings.to_vec(),
        };
        unload
            .verify_terms(
                corpus.installed.verifier().scheme(),
                &self.header,
                &self.authority(),
            )
            .unwrap();
        let Some(directory) = std::env::var_os("KAGEMUSHA_SUFFIX_STAGE_OUTPUT") else {
            return;
        };
        let directory = PathBuf::from(directory);
        fs::create_dir_all(&directory).unwrap();
        let write = |name: &str, bytes: Vec<u8>| fs::write(directory.join(name), bytes).unwrap();
        write("header.norito", self.header.to_canonical_bytes().unwrap());
        write("anchor.norito", self.anchor.to_canonical_bytes().unwrap());
        write("unload.norito", unload.to_canonical_bytes().unwrap());
        for (index, bytes) in self.chunks.iter().enumerate() {
            write(&format!("chunk-{index:03}.norito"), bytes.clone());
        }
        write(
            "verifier-pack.norito",
            bounded(
                &corpus.root.join("catalog/engineering-verifier-pack.norito"),
                VERIFIER_PACK_MAX_BYTES_V1,
            ),
        );
        write(
            "installation.norito",
            bounded(
                &corpus.root.join("catalog/engineering-installation.norito"),
                1024,
            ),
        );
        let asset_bytes = bounded(&corpus.root.join("setup/asset.norito"), 4096);
        let asset: KagemushaWalletAssetScopeV1 = decode(&asset_bytes).unwrap();
        assert_eq!(asset.asset_digest(), corpus.credential.body.asset_digest);
        write("asset.norito", asset_bytes);
        write(
            "scheme.norito",
            corpus
                .installed
                .verifier()
                .scheme()
                .to_canonical_bytes()
                .unwrap(),
        );
        fs::write(directory.join("VERIFIED-NATIVE"), "All shared verifier checks and exact canonical stage terms passed. Full subsequent ordinary Omega continuation is reported separately.\n").unwrap();
        eprintln!(
            "SUFFIX_GATE canonical_stage_frames={} directory={}",
            self.chunks.len() + 2,
            directory.display()
        );
    }
}
