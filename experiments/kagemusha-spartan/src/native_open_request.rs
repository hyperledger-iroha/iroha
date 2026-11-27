//! Derive receiver terms only from the selected opening and its verified released proof.
use super::*;
use crate::{bootstrap_context, certificate, credential, request};
use iroha_data_model::kagemusha::{
    KagemushaWalletFeeRoundingV1 as Rounding, KagemushaWalletFeeScheduleBodyV1,
    KagemushaWalletRequestBodyV1,
};

pub(super) fn fee_amount(schedule: &[u8], amount: u128) -> Result<u128> {
    if schedule.len() != crate::fee_schedule::BODY_BYTES + 64 {
        return Err(Error::Malformed("Request fee schedule length"));
    }
    let body = KagemushaWalletFeeScheduleBodyV1 {
        version: u16::from_le_bytes(schedule[..2].try_into().unwrap()),
        scheme_id: schedule[2..34].try_into().unwrap(),
        asset_digest: schedule[34..66].try_into().unwrap(),
        schedule_id: u64::from_le_bytes(schedule[66..74].try_into().unwrap()),
        beneficiary_account_digest: schedule[74..106].try_into().unwrap(),
        basis_points: u32::from_le_bytes(schedule[106..110].try_into().unwrap()),
        fixed: u128::from_le_bytes(schedule[110..126].try_into().unwrap()),
        minimum: u128::from_le_bytes(schedule[126..142].try_into().unwrap()),
        maximum: u128::from_le_bytes(schedule[142..158].try_into().unwrap()),
        rounding: match schedule[158] {
            1 => Rounding::Down,
            2 => Rounding::Up,
            _ => return Err(Error::Malformed("Request fee rounding")),
        },
        signer_certificate: schedule[159..191].try_into().unwrap(),
    };
    body.fee(amount)
        .map_err(|_| Error::Malformed("Request fee amount"))
}

impl NativeEngine {
    /// Return the canonical 418-byte open Request terms and ordered signer originals.
    /// The provider must prove which Released capsule and proof are currently selected.
    pub fn open_request_terms(
        &self,
        prepared: &PreparedOperation,
        head: &VerifiedWalletHead,
        issuer: &[u8],
        fee_schedule: &[u8],
        fee_issuer: &[u8],
        amount: u128,
        nonce: [u8; 32],
        parameters: &AuthenticatedParameters,
    ) -> Result<(Vec<u8>, Vec<u8>)> {
        head.require_parameters(parameters)?;
        let (core, rest) = prepared.successor_opening();
        let c = prepared.credential();
        let commitment = crate::monetary_state::commitment(core, rest);
        if amount == 0
            || nonce == [0; 32]
            || prepared.manifest_digest != *parameters.manifest_digest()
            || commitment != head.profile()[crate::full::HEAD_IO]
            || commitment.to_repr() != *prepared.capsule.successor()
            || core[0] != Fp::from(1)
            || head.profile()[bootstrap_context::LINEAGE_IO + bootstrap_context::LIFECYCLE]
                != core[0]
            || c.body != head.credential().body
            || c.signature != head.credential().signature
        {
            return Err(Error::AuthorityMismatch(
                "inactive or foreign released Request source",
            ));
        }
        let issuer_id = certificate::certificate_digest(&engine::read_certificate(issuer)?);
        if issuer_id.to_repr() != c.body[500..532] {
            return Err(Error::AuthorityMismatch("Request enrollment issuer"));
        }
        let mut signers = vec![(issuer_id, issuer)];
        let asset: [u8; 32] = c.body[34..66].try_into().unwrap();
        let fee = if rest[2] == Fp::from(0) {
            if !fee_schedule.is_empty() || !fee_issuer.is_empty() {
                return Err(Error::AuthorityMismatch("unheld Request fee schedule"));
            }
            0
        } else {
            let digest = self.authenticate_fee_schedule_originals(
                fee_schedule,
                fee_issuer,
                &asset,
                parameters,
            )?;
            if digest != rest[2] {
                return Err(Error::AuthorityMismatch("held Request fee schedule"));
            }
            let policy_id = certificate::certificate_digest(&engine::read_certificate(fee_issuer)?);
            if policy_id == issuer_id {
                return Err(Error::AuthorityMismatch("duplicate Request signer"));
            }
            signers.push((policy_id, fee_issuer));
            fee_amount(fee_schedule, amount)?
        };
        signers.sort_by_key(|(id, _)| id.to_repr());
        let u64_word = |value: Fp| {
            let bytes = value.to_repr();
            if bytes[8..] != [0; 24] {
                return Err(Error::Malformed("Request u64 opening"));
            }
            Ok(u64::from_le_bytes(bytes[..8].try_into().unwrap()))
        };
        let terms = KagemushaWalletRequestBodyV1 {
            version: 1,
            scheme_id: c.body[2..34].try_into().unwrap(),
            asset_digest: asset,
            payer_wallet_id: [0; 32],
            payer_account_digest: [0; 32],
            send_ordinal: 0,
            receiver_wallet_id: c.body[66..98].try_into().unwrap(),
            receiver_account_digest: c.body[98..130].try_into().unwrap(),
            receiver_credential_digest: credential::digest(c).to_repr(),
            amount,
            fee_schedule: rest[2].to_repr(),
            fee,
            policy_epoch: u64_word(core[27])?,
            scheme_policy: rest[1].to_repr(),
            receiver_accepted_time_ms: u64_word(core[28])?,
            certificates: request::certificate_set_digest(
                &signers.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            )
            .to_repr(),
            nonce,
        };
        terms
            .validate_open()
            .map_err(|_| Error::Malformed("open Request terms"))?;
        let original = signers
            .into_iter()
            .flat_map(|(_, b)| b.iter().copied())
            .collect();
        Ok((terms.transcript(), original))
    }
}
