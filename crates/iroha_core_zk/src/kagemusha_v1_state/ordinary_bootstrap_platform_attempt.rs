//! Exact ordinary bootstrap OS invocation and original retention before capture/proving.

use super::*;
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalChallengeV1, KagemushaAppOperationApprovalEvidenceV1,
    KagemushaAppOperationApprovalV1, KagemushaHardwarePlatformClassV1,
    kagemusha_ordinary_apple_original_parts_v1,
};
use rand_core_06::{OsRng, RngCore as _};
use sha2::{Digest as _, Sha256};

const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "ordinary-bootstrap-platform.norito.wal",
    magic: b"IKGOBP1\0",
    hash_domain: b"iroha:kagemusha:v1:ordinary-bootstrap-platform-attempt\0",
    maximum_payload_bytes: 16 * 1024,
};
#[derive(Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_v1_state::OrdinaryBootstrapPlatformRecordV1")]
enum Record {
    Prepared {
        ticket: u64,
        challenge: Box<KagemushaAppOperationApprovalChallengeV1>,
        scope: DigestV1,
    },
    Invoked,
    Original(Vec<u8>),
    Consumed(Vec<u8>),
    Cancelled,
}
pub(super) struct BootstrapPlatformAttempt {
    wal: PrivateJournal,
    ticket: u64,
    challenge: KagemushaAppOperationApprovalChallengeV1,
    scope: DigestV1,
    class: KagemushaHardwarePlatformClassV1,
    app_release: DigestV1,
    stage: u8,
    raw: Option<Vec<u8>>,
    receipt: Option<Vec<u8>>,
}
impl BootstrapPlatformAttempt {
    pub(super) fn create(
        path: &Path,
        challenge: KagemushaAppOperationApprovalChallengeV1,
        scope: DigestV1,
        class: KagemushaHardwarePlatformClassV1,
        app_release: DigestV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let mut entropy = [0; 8];
        OsRng.try_fill_bytes(&mut entropy).map_err(material)?;
        let ticket = u64::from_le_bytes(entropy);
        if ticket == 0 || scope == [0; 32] {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let mut this = Self {
            wal: PrivateJournal::create_new(path, FORMAT).map_err(material)?,
            ticket,
            challenge,
            scope,
            class,
            app_release,
            stage: 0,
            raw: None,
            receipt: None,
        };
        this.append(Record::Prepared {
            ticket,
            challenge: Box::new(challenge),
            scope,
        })?;
        Ok(this)
    }
    pub(super) fn open_existing(
        path: &Path,
        challenge: KagemushaAppOperationApprovalChallengeV1,
        scope: DigestV1,
        class: KagemushaHardwarePlatformClassV1,
        app_release: DigestV1,
    ) -> Result<Self, KagemushaStateErrorV1> {
        let mut wal = PrivateJournal::open_existing(path, FORMAT).map_err(material)?;
        let (sequence, raw) = wal
            .replay_next()
            .map_err(material)?
            .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?;
        let Record::Prepared {
            ticket,
            challenge: held,
            scope: held_scope,
        } = decode(&raw)?
        else {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        };
        if sequence != 0 || ticket == 0 || *held != challenge || held_scope != scope {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let mut this = Self {
            wal,
            ticket,
            challenge,
            scope,
            class,
            app_release,
            stage: 0,
            raw: None,
            receipt: None,
        };
        let mut rows = 1;
        while let Some((sequence, raw)) = this.wal.replay_next().map_err(material)? {
            if sequence as usize != rows || rows >= 4 {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            rows += 1;
            match decode(&raw)? {
                Record::Invoked if this.stage == 0 => this.stage = 1,
                Record::Original(raw) if this.stage == 1 => {
                    this.check_raw(&raw)?;
                    this.raw = Some(raw);
                    this.stage = 2;
                }
                Record::Consumed(receipt) if this.stage == 2 => {
                    if receipt.len() != 184 {
                        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                    }
                    this.receipt = Some(receipt);
                    this.stage = 3;
                }
                Record::Cancelled if this.stage == 0 => this.stage = 4,
                _ => return Err(KagemushaStateErrorV1::SnapshotIntegrity),
            }
        }
        this.wal.check_owned().map_err(material)?;
        Ok(this)
    }
    pub(super) fn ticket(&self) -> u64 {
        self.ticket
    }
    pub(super) fn scope(&self) -> DigestV1 {
        self.scope
    }
    pub(super) fn challenge(&self) -> &KagemushaAppOperationApprovalChallengeV1 {
        &self.challenge
    }
    pub(super) fn recheck(&self) -> Result<(), KagemushaStateErrorV1> {
        self.wal.check_owned().map_err(material)?;
        if self.stage == 4 {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        Ok(())
    }
    pub(super) fn fence(&mut self) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.recheck()?;
        match self.stage {
            0 => {
                self.append(Record::Invoked)?;
                self.stage = 1;
                Ok(vec![vec![1], vec![], vec![]])
            }
            1 => Err(KagemushaStateErrorV1::InvalidCandidateStage),
            2 => Ok(vec![
                vec![2],
                self.raw
                    .clone()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
                vec![],
            ]),
            3 => Ok(vec![
                vec![3],
                self.raw
                    .clone()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
                self.receipt
                    .clone()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
            ]),
            _ => Err(KagemushaStateErrorV1::InvalidCandidateStage),
        }
    }
    pub(super) fn retain_raw(&mut self, raw: &[u8]) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.recheck()?;
        self.check_raw(raw)?;
        if let Some(held) = &self.raw {
            if held != raw {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        } else {
            if self.stage != 1 {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            }
            self.append(Record::Original(raw.to_vec()))?;
            self.raw = Some(raw.to_vec());
            self.stage = 2;
        }
        self.recheck()?;
        Ok(Sha256::digest(raw).into())
    }
    pub(super) fn approval_original(&self) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck()?;
        let raw = self
            .raw
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let evidence = match self.class {
            KagemushaHardwarePlatformClassV1::AndroidKeyMint => {
                KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore {
                    signature_der: raw.clone(),
                }
            }
            KagemushaHardwarePlatformClassV1::AppleAppAttest => {
                KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest {
                    raw_assertion: raw.clone(),
                }
            }
            _ => return Err(KagemushaStateErrorV1::InvalidHardwareProfile),
        };
        norito::encode_canonical(&KagemushaAppOperationApprovalV1 {
            challenge: self.challenge,
            evidence,
        })
        .map_err(material)
    }
    pub(super) fn complete(
        &mut self,
        counter: Option<u32>,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck()?;
        let expected = self.make_receipt(counter)?;
        if let Some(held) = &self.receipt {
            if *held != expected {
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        } else {
            if self.stage != 2 {
                return Err(KagemushaStateErrorV1::InvalidCandidateStage);
            }
            self.append(Record::Consumed(expected.clone()))?;
            self.receipt = Some(expected.clone());
            self.stage = 3;
        }
        Ok(expected)
    }
    pub(super) fn receipt_for_counter(
        &self,
        counter: Option<u32>,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck()?;
        self.make_receipt(counter)
    }
    pub(super) fn retained_consumed_receipt_for_original(
        &self,
        original: &[u8],
        counter: Option<u32>,
    ) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        self.recheck()?;
        let recovered = self.recover()?;
        if self.approval_original()? != original
            || recovered[0] != [2]
            || recovered[2] != self.receipt_for_counter(counter)?
        {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.recheck()?;
        Ok(recovered[2].clone())
    }
    pub(super) fn recover(&self) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.recheck()?;
        match self.stage {
            0 => Ok(vec![vec![0], vec![], vec![]]),
            1 => Err(KagemushaStateErrorV1::InvalidCandidateStage),
            2 => Ok(vec![
                vec![1],
                self.raw
                    .clone()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
                vec![],
            ]),
            3 => Ok(vec![
                vec![2],
                self.raw
                    .clone()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
                self.receipt
                    .clone()
                    .ok_or(KagemushaStateErrorV1::SnapshotIntegrity)?,
            ]),
            _ => Err(KagemushaStateErrorV1::InvalidCandidateStage),
        }
    }
    pub(super) fn cancel(&mut self) -> Result<(), KagemushaStateErrorV1> {
        self.recheck()?;
        if self.stage != 0 {
            return Err(KagemushaStateErrorV1::InvalidCandidateStage);
        }
        self.append(Record::Cancelled)?;
        self.stage = 4;
        Ok(())
    }
    fn check_raw(&self, raw: &[u8]) -> Result<(), KagemushaStateErrorV1> {
        match self.class {
            KagemushaHardwarePlatformClassV1::AndroidKeyMint if (8..=72).contains(&raw.len()) => {
                let signature = p256::ecdsa::Signature::from_der(raw).map_err(material)?;
                if signature.to_der().as_bytes() != raw {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
            }
            KagemushaHardwarePlatformClassV1::AppleAppAttest => {
                kagemusha_ordinary_apple_original_parts_v1(raw, self.app_release)
                    .map_err(material)?;
            }
            _ => return Err(KagemushaStateErrorV1::InvalidHardwareProfile),
        }
        Ok(())
    }
    fn make_receipt(&self, counter: Option<u32>) -> Result<Vec<u8>, KagemushaStateErrorV1> {
        let raw = self
            .raw
            .as_ref()
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let mut bytes = b"KGMAPP1\0".to_vec();
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&self.ticket.to_le_bytes());
        for digest in [
            self.challenge.operation_id,
            self.scope,
            Sha256::digest(self.challenge.canonical_signing_bytes().map_err(material)?).into(),
            Sha256::digest(raw).into(),
            self.challenge.enrollment_digest,
        ] {
            bytes.extend_from_slice(&digest);
        }
        bytes.push(u8::from(counter.is_some()));
        bytes.extend_from_slice(&counter.unwrap_or(0).to_le_bytes());
        if bytes.len() != 184 {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        Ok(bytes)
    }
    fn append(&mut self, record: Record) -> Result<(), KagemushaStateErrorV1> {
        let bytes = norito::encode_canonical(&record).map_err(material)?;
        if bytes.len() > FORMAT.maximum_payload_bytes as usize {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.wal.append(&bytes).map_err(material)
    }
}
fn decode(raw: &[u8]) -> Result<Record, KagemushaStateErrorV1> {
    if raw.len() > FORMAT.maximum_payload_bytes as usize {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    let record: Record = norito::decode_canonical(raw).map_err(material)?;
    if norito::encode_canonical(&record).map_err(material)? != raw {
        return Err(KagemushaStateErrorV1::SnapshotIntegrity);
    }
    Ok(record)
}

/// Exercise receipt persistence with the maintained model fixture's actual native W and signature.
/// This test hook constructs no financial or production bootstrap authority.
#[cfg(test)]
pub(super) fn assert_receipt_binding_and_replay(
    approval: &KagemushaAppOperationApprovalV1,
    app_release: DigestV1,
    counter: Option<u32>,
) {
    let (class, raw) = match &approval.evidence {
        KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der } => (
            KagemushaHardwarePlatformClassV1::AndroidKeyMint,
            signature_der.as_slice(),
        ),
        KagemushaAppOperationApprovalEvidenceV1::AppleAppAttest { raw_assertion } => (
            KagemushaHardwarePlatformClassV1::AppleAppAttest,
            raw_assertion.as_slice(),
        ),
    };
    let challenge = approval.challenge;
    let scope = [99; 32];
    assert_ne!(scope, challenge.enrollment_digest);
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().canonicalize().unwrap().join("platform-attempt");
    let mut attempt =
        BootstrapPlatformAttempt::create(&path, challenge, scope, class, app_release).unwrap();
    let ticket = attempt.ticket();
    assert_eq!(attempt.fence().unwrap(), vec![vec![1], vec![], vec![]]);
    assert!(
        attempt.fence().is_err(),
        "unknown platform outcomes cannot re-sign"
    );
    assert_eq!(
        attempt.retain_raw(raw).unwrap(),
        <[u8; 32]>::from(Sha256::digest(raw))
    );
    assert_eq!(
        attempt.approval_original().unwrap(),
        norito::encode_canonical(approval).unwrap()
    );
    let receipt = attempt.complete(counter).unwrap();
    assert_eq!(receipt.len(), 184);
    assert_eq!(&receipt[11..19], &ticket.to_le_bytes());
    assert_eq!(&receipt[19..51], &challenge.operation_id);
    assert_eq!(&receipt[51..83], &scope);
    assert_eq!(
        &receipt[83..115],
        &Sha256::digest(challenge.canonical_signing_bytes().unwrap())[..]
    );
    assert_eq!(&receipt[115..147], &Sha256::digest(raw)[..]);
    assert_eq!(&receipt[147..179], &challenge.enrollment_digest);
    assert_ne!(&receipt[147..179], &scope);
    assert_eq!(receipt[179], u8::from(counter.is_some()));
    assert_eq!(&receipt[180..184], &counter.unwrap_or(0).to_le_bytes());
    let prefix = attempt.wal.recovery_prefix().unwrap();
    assert_eq!(attempt.complete(counter).unwrap(), receipt);
    assert_eq!(attempt.wal.recovery_prefix().unwrap(), prefix);
    assert_eq!(
        attempt
            .retained_consumed_receipt_for_original(
                &norito::encode_canonical(approval).unwrap(),
                counter,
            )
            .unwrap(),
        receipt
    );
    let mut substituted = norito::encode_canonical(approval).unwrap();
    substituted[0] ^= 1;
    assert!(
        attempt
            .retained_consumed_receipt_for_original(&substituted, counter)
            .is_err()
    );
    let other_counter = counter.map_or(Some(1), |value| Some(value ^ 1));
    assert!(
        attempt
            .retained_consumed_receipt_for_original(
                &norito::encode_canonical(approval).unwrap(),
                other_counter,
            )
            .is_err()
    );
    assert_eq!(attempt.wal.recovery_prefix().unwrap(), prefix);
    drop(attempt);
    let mut reopened =
        BootstrapPlatformAttempt::open_existing(&path, challenge, scope, class, app_release)
            .unwrap();
    assert_eq!(reopened.ticket(), ticket);
    assert_eq!(
        reopened.recover().unwrap(),
        vec![vec![2], raw.to_vec(), receipt.clone()]
    );
    assert_eq!(
        reopened.approval_original().unwrap(),
        norito::encode_canonical(approval).unwrap()
    );
    assert_eq!(reopened.receipt_for_counter(counter).unwrap(), receipt);
    assert_eq!(reopened.complete(counter).unwrap(), receipt);
    assert_eq!(reopened.wal.recovery_prefix().unwrap(), prefix);
    assert_eq!(
        reopened
            .retained_consumed_receipt_for_original(
                &norito::encode_canonical(approval).unwrap(),
                counter,
            )
            .unwrap(),
        receipt
    );
    assert!(
        reopened
            .retained_consumed_receipt_for_original(&substituted, counter)
            .is_err()
    );
    assert!(
        reopened
            .retained_consumed_receipt_for_original(
                &norito::encode_canonical(approval).unwrap(),
                other_counter,
            )
            .is_err()
    );
    assert_eq!(reopened.wal.recovery_prefix().unwrap(), prefix);
}

#[cfg(test)]
#[path = "ordinary_bootstrap_platform_attempt_tests.rs"]
mod tests;
