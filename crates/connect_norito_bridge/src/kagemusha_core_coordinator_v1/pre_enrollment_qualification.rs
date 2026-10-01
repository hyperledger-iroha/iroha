//! Native transient qualification before an issuer ceremony or monetary Core exists.
//!
//! The independently authenticated release and original native journal ticket pin the catalog
//! and wallet. A fresh operation-1 device signature is retained in this process for phase 2;
//! decoded device fields, cached responses and application projections cannot create admission.

use super::{
    KagemushaCoreCoordinatorBackendErrorV1 as Error, KagemushaCoreCoordinatorMethodV1 as Method,
    kagemusha_core_coordinator_decode_request_v1, kagemusha_core_coordinator_encode_response_v1,
    startup_qualification::{NativeReadObservationV1, NativeStartupQualificationOwnerV1},
};

type Result<T> = std::result::Result<T, Error>;

/// Opaque native-signed observation retained before the issuer challenge is accepted.
/// It grants no retail session, hardware checkpoint, trusted time or monetary authority.
pub struct KagemushaVerifiedPreEnrollmentQualificationV1 {
    observation: NativeReadObservationV1,
}

impl KagemushaVerifiedPreEnrollmentQualificationV1 {
    pub(super) fn require_original(&self, canonical_qualification: &[u8]) -> Result<()> {
        if self.observation.operation != 1
            || self.observation.canonical_reply != canonical_qualification
        {
            return Err(Error::Rejected);
        }
        Ok(())
    }
}

/// One transient, independently pinned operation-1 owner for the original live journal ticket.
/// Construction is private to the consuming kernel delegate; applications cannot supply pins.
pub struct KagemushaPreEnrollmentQualificationOwnerV1 {
    observer: NativeStartupQualificationOwnerV1,
    nonce: Option<[u8; 32]>,
    verified: Option<KagemushaVerifiedPreEnrollmentQualificationV1>,
}

impl KagemushaPreEnrollmentQualificationOwnerV1 {
    pub(super) fn new(observer: NativeStartupQualificationOwnerV1) -> Self {
        Self {
            observer,
            nonce: None,
            verified: None,
        }
    }

    pub(super) fn verified(&self) -> Option<&KagemushaVerifiedPreEnrollmentQualificationV1> {
        self.verified.as_ref()
    }

    pub(super) fn invoke(&mut self, method: Method, frame: &[u8]) -> Result<Vec<u8>> {
        let fields =
            kagemusha_core_coordinator_decode_request_v1(frame).map_err(|_| Error::Rejected)?;
        let empty =
            || kagemusha_core_coordinator_encode_response_v1(&[]).map_err(|_| Error::Rejected);
        match method {
            Method::BeginObservation => {
                if fields.len() != 2 || fields[0] != 1_u32.to_le_bytes() || self.nonce.is_some() {
                    return Err(Error::Rejected);
                }
                let nonce = self
                    .observer
                    .begin(1, &fields[1])
                    .map_err(|_| Error::Rejected)?;
                self.nonce = Some(nonce);
                kagemusha_core_coordinator_encode_response_v1(&[nonce.to_vec()])
                    .map_err(|_| Error::Rejected)
            }
            Method::AcceptQualification => {
                self.observer
                    .stage_qualification(&fields)
                    .map_err(|_| Error::Rejected)?;
                empty()
            }
            Method::AcceptAuthenticatedReply => {
                if fields.len() != 10 || fields[0] != 1_u32.to_le_bytes() {
                    return Err(Error::Rejected);
                }
                let nonce: [u8; 32] = fields[1]
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Rejected)?;
                let (_, observation) = self
                    .observer
                    .accept(1, nonce, &fields[2], &fields[3], &fields[4], &fields[5..])
                    .map_err(|_| Error::Rejected)?;
                if self
                    .verified
                    .as_ref()
                    .is_some_and(|previous| previous.observation != observation)
                {
                    return Err(Error::Rejected);
                }
                self.verified = Some(KagemushaVerifiedPreEnrollmentQualificationV1 { observation });
                empty()
            }
            _ => Err(Error::Rejected),
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::kagemusha_core_coordinator_v1::{
        KagemushaEnrollmentJournalSelectionV1, initial_enrollment::tests as fixture,
        kagemusha_core_coordinator_decode_response_v1,
        kagemusha_core_coordinator_encode_request_v1,
    };
    use iroha_data_model::kagemusha::{
        KagemushaDeviceReadCredentialCommandV1, kagemusha_decode_device_success_response_v1,
    };

    fn owner(
        selected: &KagemushaEnrollmentJournalSelectionV1,
    ) -> KagemushaPreEnrollmentQualificationOwnerV1 {
        let c = fixture::journal_context(selected);
        let enrollment = iroha_core_zk::kagemusha_v1_state::KagemushaRecoveryEnrollmentBindingV1 {
            enrollment_id: c.owner.enrollment_id().unwrap(),
            core_authorization_key_reference: crate::kagemusha_device_bridge_v1::sender_payload::hardware_authorization_key_reference_v1(&c.native_authorization_public_key),
            owner: c.owner,
        };
        KagemushaPreEnrollmentQualificationOwnerV1::new(
            NativeStartupQualificationOwnerV1::from_pre_enrollment_context(
                &c.release,
                enrollment,
                &c.native_authorization_public_key,
                selected.hardware_profile_id,
            )
            .unwrap(),
        )
    }

    pub(in crate::kagemusha_core_coordinator_v1) fn verified_for_selection(
        selected: &KagemushaEnrollmentJournalSelectionV1,
    ) -> KagemushaVerifiedPreEnrollmentQualificationV1 {
        let mut owner = owner(selected);
        let command = KagemushaDeviceReadCredentialCommandV1::canonical_bytes().unwrap();
        let begin = kagemusha_core_coordinator_encode_request_v1(&[
            1_u32.to_le_bytes().to_vec(),
            command.clone(),
        ])
        .unwrap();
        let result = owner.invoke(Method::BeginObservation, &begin).unwrap();
        assert_eq!(
            owner.invoke(Method::BeginObservation, &begin),
            Err(Error::Rejected)
        );
        let nonce: [u8; 32] = kagemusha_core_coordinator_decode_response_v1(&result).unwrap()[0]
            .as_slice()
            .try_into()
            .unwrap();
        let qualification = fixture::journal_qualification_fields(selected);
        owner
            .invoke(
                Method::AcceptQualification,
                &kagemusha_core_coordinator_encode_request_v1(&qualification).unwrap(),
            )
            .unwrap();
        let original = fixture::journal_qualification_response(selected, nonce);
        let reply = kagemusha_decode_device_success_response_v1(&original, 1, nonce).unwrap();
        let mut fields = vec![
            1_u32.to_le_bytes().to_vec(),
            nonce.to_vec(),
            command,
            reply.payload.to_vec(),
            reply.authenticator.to_vec(),
        ];
        fields.extend(qualification[..5].iter().cloned());
        let frame = kagemusha_core_coordinator_encode_request_v1(&fields).unwrap();
        owner
            .invoke(Method::AcceptAuthenticatedReply, &frame)
            .unwrap();
        owner
            .invoke(Method::AcceptAuthenticatedReply, &frame)
            .unwrap();
        fields[1][0] ^= 1;
        assert_eq!(
            owner.invoke(
                Method::AcceptAuthenticatedReply,
                &kagemusha_core_coordinator_encode_request_v1(&fields).unwrap()
            ),
            Err(Error::Rejected)
        );
        owner.verified.take().unwrap()
    }

    #[test]
    fn fresh_native_pre_enrollment_observation_requires_catalog_signature_nonce_and_exact_original()
    {
        let selected = KagemushaEnrollmentJournalSelectionV1 {
            account_i105: fixture::journal_account(),
            ticket: 7,
            client_nonce: [21; 32],
            release_id: fixture::journal_pins().release_id,
            hardware_profile_id: fixture::journal_pins().hardware_profile_id,
            lane_id: [32; 32],
        };
        let verified = verified_for_selection(&selected);
        verified
            .require_original(&fixture::journal_challenge_fields(&selected)[4])
            .unwrap();
        let mut replaced = verified.observation.canonical_reply.clone();
        replaced[0] ^= 1;
        assert_eq!(verified.require_original(&replaced), Err(Error::Rejected));
    }
}
