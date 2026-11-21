//! Signed open Request and its exact receiver account companion under IPM1 profile 0x8001.
use super::*;
use crate::{fee_schedule, payment_wire::PeerOriginals, request};
use iroha_data_model::{account::AccountId, kagemusha::kagemusha_wallet_account_digest_v1};

const REQUEST: usize = request::BODY_BYTES + request::SIGNATURE_BYTES;
const CREDENTIAL: usize = credential::BODY_BYTES + 64;
const CERTIFICATE: usize = 172;
const SCHEDULE: usize = fee_schedule::BODY_BYTES + 64;
const WITHOUT_FEE: usize = REQUEST + CREDENTIAL + CERTIFICATE;
const WITH_FEE: usize = WITHOUT_FEE + CERTIFICATE + SCHEDULE;

/// Exact originals in wire order. Decoding binds the receiver key and account to the signed
/// terms. The installed authority must also authenticate the credential and any fee policy.
#[derive(Debug)]
pub struct OpenRequestPeer<'a> {
    pub request: &'a [u8],
    pub receiver_credential: &'a [u8],
    pub certificates: &'a [u8],
    pub fee_schedule: &'a [u8],
    pub destination_account: &'a [u8],
}

impl OpenRequestPeer<'_> {
    fn check_dependencies(&self, b: &[u8; request::BODY_BYTES]) -> Result<Credential> {
        let credential = read_credential(self.receiver_credential)?;
        let has_fee = !self.fee_schedule.is_empty();
        let amount = u128::from_le_bytes(b[242..258].try_into().unwrap());
        let fee = u128::from_le_bytes(b[290..306].try_into().unwrap());
        if b[..2] != 1u16.to_le_bytes()
            || b[66..130] != [0; 64]
            || b[194..210] != [0; 16]
            || amount == 0
            || amount.checked_add(fee).is_none()
            || b[130..162] == [0; 32]
            || b[162..194] == [0; 32]
            || (b[306..314] == [0; 8]) != (b[314..346] == [0; 32])
            || b[386..418] == [0; 32]
            || credential.body[..66] != b[..66]
            || credential.body[66..130] != b[130..194]
            || credential::digest(&credential).to_repr() != b[210..242]
            || self.certificates.len() != (1 + usize::from(has_fee)) * CERTIFICATE
            || (b[258..290] != [0; 32]) != has_fee
            || (!has_fee && b[290..306] != [0; 16])
            || !(1..=4096).contains(&self.destination_account.len())
        {
            return Err(Error::Malformed("open Request terms or dependencies"));
        }
        let account: AccountId = norito::core::with_decode_limits_scope(
            norito::DecodeLimits::new(4096, 4096, 16384, 32768, 16),
            || norito::decode_canonical(self.destination_account),
        )
        .map_err(|_| Error::Malformed("open Request canonical AccountId"))?;
        if norito::encode_canonical(&account)
            .map_err(|_| Error::Malformed("open Request AccountId"))?
            != self.destination_account
            || kagemusha_wallet_account_digest_v1(&account)
                .map_err(|_| Error::Malformed("open Request account digest"))?
                != b[162..194]
        {
            return Err(Error::AuthorityMismatch("open Request destination account"));
        }
        let certs = self
            .certificates
            .chunks_exact(CERTIFICATE)
            .map(read_certificate)
            .collect::<Result<Vec<_>>>()?;
        let digests: Vec<_> = certs.iter().map(certificate::certificate_digest).collect();
        if digests
            .windows(2)
            .any(|ds| ds[0].to_repr() >= ds[1].to_repr())
            || request::certificate_set_digest(&digests).to_repr() != b[354..386]
        {
            return Err(Error::AuthorityMismatch("open Request certificates"));
        }
        let matches_signer = |role: certificate::Role, id: &[u8]| {
            certs.iter().zip(&digests).any(|(c, d)| {
                c.body[..34] == b[..34] && c.body[34] == role as u8 && d.to_repr() == id
            })
        };
        if !matches_signer(certificate::Role::Enrollment, &credential.body[500..532]) {
            return Err(Error::AuthorityMismatch("open Request credential issuer"));
        }
        if has_fee {
            if self.fee_schedule.len() != SCHEDULE {
                return Err(Error::Malformed("open Request schedule"));
            }
            let schedule = fee_schedule::SignedFeeSchedule {
                body: self.fee_schedule[..fee_schedule::BODY_BYTES]
                    .try_into()
                    .unwrap(),
                signature: self.fee_schedule[fee_schedule::BODY_BYTES..]
                    .try_into()
                    .unwrap(),
            };
            if super::super::open_request::fee_amount(self.fee_schedule, amount)? != fee
                || schedule.body[..66] != b[..66]
                || fee_schedule::digest(&schedule).to_repr() != b[258..290]
                || !matches_signer(
                    certificate::Role::RegulatoryPolicy,
                    &schedule.body[159..191],
                )
            {
                return Err(Error::AuthorityMismatch("open Request schedule"));
            }
        }
        Ok(credential)
    }

    /// Validate unsigned receiver terms and the exact companion originals before hardware signing.
    /// This grants no authority to use a Request for Send.
    pub fn validate_for_issuance(&self) -> Result<()> {
        let body = self
            .request
            .try_into()
            .map_err(|_| Error::Malformed("open Request body length"))?;
        self.check_dependencies(body).map(|_| ())
    }

    fn check(&self) -> Result<()> {
        let req = CompletedRequest::from_tape(self.request).map_err(Error::Malformed)?;
        let credential = self.check_dependencies(&req.body)?;
        let terms = request::commitments(&req).map_err(Error::Malformed)?;
        certificate::verify_native(
            &point(&credential.body[130..195])?,
            terms.message,
            &req.signature,
        )
        .map_err(|_| Error::AuthorityMismatch("open Request receiver signature"))
    }
}

/// Form one uncompressed schema-3 Request. No caller-selected encoding or schema is accepted.
pub fn encode_open_request_peer(
    request: &[u8],
    receiver_credential: &[u8],
    sorted_certificate_originals: &[u8],
    fee_schedule: &[u8],
    account_original: &[u8],
) -> Result<Vec<u8>> {
    let originals = OpenRequestPeer {
        request,
        receiver_credential,
        certificates: sorted_certificate_originals,
        fee_schedule,
        destination_account: account_original,
    };
    originals.check()?;
    let envelope = [
        request,
        receiver_credential,
        sorted_certificate_originals,
        fee_schedule,
    ]
    .concat();
    PeerOriginals::OpenRequest
        .encode(&envelope, account_original)
        .map_err(Error::Rejected)
}

/// Decode bounded wire originals, verify the receiver signature and the exact account companion.
pub fn decode_open_request_peer(wire: &[u8]) -> Result<OpenRequestPeer<'_>> {
    let (envelope, destination_account) = PeerOriginals::OpenRequest
        .decode(wire)
        .map_err(Error::Rejected)?;
    let certificates_end = match envelope.len() {
        WITHOUT_FEE => WITHOUT_FEE,
        WITH_FEE => WITH_FEE - SCHEDULE,
        _ => return Err(Error::Malformed("open Request originals length")),
    };
    let originals = OpenRequestPeer {
        request: &envelope[..REQUEST],
        receiver_credential: &envelope[REQUEST..REQUEST + CREDENTIAL],
        certificates: &envelope[REQUEST + CREDENTIAL..certificates_end],
        fee_schedule: &envelope[certificates_end..],
        destination_account,
    };
    originals.check()?;
    Ok(originals)
}

impl PreparedOperation {
    /// Reconstruct a raw Send's original peer Request using its exact signed tape, receiver
    /// credential, issuer certificates and fee schedule. The supplied canonical AccountId
    /// is checked against the destination digest signed by both receiver and issuer.
    ///
    /// # Errors
    /// Rejects any other operation, incomplete originals, or a different peer message.
    pub fn verify_recovered_send_request(&self, supplied: &[u8]) -> Result<()> {
        let ExactOperationWitness::Send(send) = self.witness.as_ref() else {
            return Err(Error::AuthorityMismatch("Request recovery requires a Send"));
        };
        let (_, account) = PeerOriginals::OpenRequest
            .decode(supplied)
            .map_err(Error::Rejected)?;
        let mut body = send.request.body;
        // Specialization fills only the payer wallet, account, and Send sequence.
        body[66..130].fill(0);
        body[194..210].fill(0);
        let tape = [body.as_slice(), &send.request.signature].concat();
        let originals = &send.request_originals;
        let certificates = originals
            .certificates
            .iter()
            .flat_map(certificate_bytes)
            .collect::<Vec<_>>();
        let schedule = if body[258..290] == [0; 32] {
            Vec::new()
        } else {
            [
                send.fee_schedule.body.as_slice(),
                &send.fee_schedule.signature,
            ]
            .concat()
        };
        let recovered = encode_open_request_peer(
            &tape,
            &credential_bytes(&originals.receiver),
            &certificates,
            &schedule,
            account,
        )?;
        if recovered != supplied {
            return Err(Error::AuthorityMismatch(
                "released Send exact original Request",
            ));
        }
        Ok(())
    }
}

impl NativeEngine {
    /// Recheck an unselected Send review against the protected observation and current time.
    /// Selected recovery uses its historical observations without requiring a new clock sample.
    pub fn revalidate_unselected_time(&self, prepared: &PreparedOperation) -> Result<()> {
        self.check_send_time(&prepared.witness, false)?;
        self.check_send_time(&prepared.witness, true)
    }

    /// Project the canonical payer account for a local regulatory refusal before Receive
    /// preparation. This projection grants no proof verdict or value.
    pub fn payment_payer_accounts_for_policy(
        &self,
        wire: &[u8],
        parameters: &AuthenticatedParameters,
    ) -> Result<[[u8; 32]; 2]> {
        let p = &parameters.inner;
        let pair = p.profile(full::SEND)?;
        payment_wire::payer_policy_accounts(
            &payment_wire::Profile {
                pair: &pair,
                scheme: p.scheme,
                relation: p.relation,
            },
            wire,
        )
        .map_err(Error::Rejected)
    }

    /// Verify a received open Request against the installed scheme root, application-bound
    /// receiver credential, issuer and optional policy signer before using it to prepare Send.
    pub fn authenticate_open_request_peer<'a>(
        &self,
        wire: &'a [u8],
        parameters: &AuthenticatedParameters,
    ) -> Result<OpenRequestPeer<'a>> {
        let originals = decode_open_request_peer(wire)?;
        let cert_for = |role: certificate::Role| {
            originals
                .certificates
                .chunks_exact(CERTIFICATE)
                .find(|c| c[34] == role as u8)
                .ok_or(Error::AuthorityMismatch("open Request signer role"))
        };
        self.authenticate_credential_originals(
            originals.receiver_credential,
            cert_for(certificate::Role::Enrollment)?,
            parameters,
        )?
        .enrollment_fields()?;
        if !originals.fee_schedule.is_empty() {
            self.authenticate_fee_schedule_originals(
                originals.fee_schedule,
                cert_for(certificate::Role::RegulatoryPolicy)?,
                originals.request[34..66].try_into().unwrap(),
                parameters,
            )?;
        }
        Ok(originals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issued_open_request_binds_signed_originals_and_canonical_account() {
        let account = AccountId::new(
            "ed01203D4017C3E843895A92B70AA74D1B7EBC9C982CCF2EC4968CC0CD55F12AF4660C"
                .parse()
                .unwrap(),
        );
        let account_original = norito::encode_canonical(&account).unwrap();
        let digest = kagemusha_wallet_account_digest_v1(&account).unwrap();
        let chain = crate::chain_fixture::new(&Gens::new(4096));
        let mut receiver = chain.send.request_originals.receiver.clone();
        receiver.body[98..130].copy_from_slice(&digest);
        let credential_bytes = credential_bytes(&receiver);
        let mut request = chain.send.request.clone();
        request.body[66..130].fill(0);
        request.body[194..210].fill(0);
        request.body[162..194].copy_from_slice(&digest);
        request.body[210..242].copy_from_slice(&credential::digest(&receiver).to_repr());
        request.signature = bootstrap::sign(
            request::commitments(&request).unwrap().message,
            &chain.receiver_key.d,
            "open Request",
        );
        let signed = [request.body.as_slice(), request.signature.as_slice()].concat();
        let cert = certificate_bytes(&chain.send.request_originals.certificates[0]);
        let wire =
            encode_open_request_peer(&signed, &credential_bytes, &cert, &[], &account_original)
                .unwrap();
        OpenRequestPeer {
            request: &request.body,
            receiver_credential: &credential_bytes,
            certificates: &cert,
            fee_schedule: &[],
            destination_account: &account_original,
        }
        .validate_for_issuance()
        .unwrap();
        let mut specialized = chain.send.clone();
        specialized.request = request.clone();
        specialized.request.body[66..130].copy_from_slice(&chain.send.request.body[66..130]);
        specialized.request.body[194..210].copy_from_slice(&u128::MAX.to_le_bytes());
        let stable = crate::native::wallet_send_replay_key(&wire).unwrap();
        assert_eq!(
            ExactOperationWitness::Send(Box::new(specialized))
                .transfer_replay_key()
                .unwrap(),
            Some(stable)
        );
        let mut overflow = request.body;
        overflow[242..258].copy_from_slice(&u128::MAX.to_le_bytes());
        overflow[290..306].copy_from_slice(&1u128.to_le_bytes());
        assert!(
            OpenRequestPeer {
                request: &overflow,
                receiver_credential: &credential_bytes,
                certificates: &cert,
                fee_schedule: &[],
                destination_account: &account_original
            }
            .validate_for_issuance()
            .is_err()
        );
        let opened = decode_open_request_peer(&wire).unwrap();
        assert_eq!(opened.request, &signed);
        assert_eq!(opened.destination_account, &account_original);
        assert_eq!(&wire[4..12], &[1, 0, 128, 1, 2, 0, 0, 3]);
        assert_eq!(wire.len(), 84 + 16 + WITHOUT_FEE + account_original.len());
        let wrap = |bytes: &[u8], account: &[u8]| {
            PeerOriginals::OpenRequest.encode(bytes, account).unwrap()
        };
        let (golden, _) = PeerOriginals::OpenRequest.decode(&wire).unwrap();
        for pos in [
            0,
            66,
            98,
            162,
            194,
            210,
            242,
            258,
            386,
            418,
            REQUEST + 98,
            REQUEST + 532,
            REQUEST + CREDENTIAL + 34,
        ] {
            let mut altered = golden.to_vec();
            altered[pos] ^= 1;
            assert!(
                decode_open_request_peer(&wrap(&altered, &account_original)).is_err(),
                "accepted mutation {pos}"
            );
        }
        assert!(decode_open_request_peer(&wrap(golden, &[1; 40])).is_err());
        assert!(
            decode_open_request_peer(&wrap(&golden[..golden.len() - 1], &account_original))
                .is_err()
        );
        let mut wrong = wire.clone();
        wrong[8] = 3;
        assert!(decode_open_request_peer(&wrong).is_err());
        assert!(PeerOriginals::Payment.decode(&wire).is_err());
    }
}
