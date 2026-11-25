//! Authenticate the receiver's signed Request before an irreversible Send.
//! The proof opens the same terms, held schedule and fee calculation. Its Send
//! relation leaves receiver signature verification to the receiving wallet.
use super::*;

impl NativeEngine {
    pub(super) fn authenticate_send_request(
        &self,
        witness: &ExactOperationWitness,
        parameters: &AuthenticatedParameters,
    ) -> Result<()> {
        if let ExactOperationWitness::Send(step) = witness {
            let p = &parameters.inner;
            authenticate(step, &p.scheme, &p.root, &p.generators.p)?;
        }
        Ok(())
    }
}

fn authenticate(
    step: &crate::send::Witness,
    scheme: &[u8; 32],
    root: &p256::Point,
    generators: &Gens<Eq>,
) -> Result<()> {
    let req = &step.request;
    let body = &req.body;
    let payer = &step.credential.body;
    let receiver = &step.request_originals.receiver;
    let asset: &[u8; 32] = payer[34..66].try_into().unwrap();
    if body[..2] != 1u16.to_le_bytes()
        || &body[2..34] != scheme
        || body[2..130] != payer[2..130]
        || receiver.body[2..66] != payer[2..66]
        || body[130..194] != receiver.body[66..130]
        || body[66..98] == body[130..162]
        || payer[130..195] == receiver.body[130..195]
        || body[210..242] != credential::digest(receiver).to_repr()
    {
        return Err(Error::AuthorityMismatch("Send Request identities"));
    }
    let terms = crate::request::commitments(req).map_err(Error::Malformed)?;
    let has_schedule = body[258..290] != [0; 32];
    let certs = &step.request_originals.certificates;
    if certs.len() != 1 + usize::from(has_schedule) {
        return Err(Error::AuthorityMismatch("Send Request signer set"));
    }
    let digests: Vec<_> = certs.iter().map(certificate::certificate_digest).collect();
    if digests
        .windows(2)
        .any(|pair| pair[0].to_repr() >= pair[1].to_repr())
        || crate::request::certificate_set_digest(&digests).to_repr() != body[354..386]
    {
        return Err(Error::AuthorityMismatch("Send Request signer set"));
    }
    let signer = |digest: &[u8]| {
        digests
            .iter()
            .position(|value| value.to_repr() == digest)
            .map(|i| &certs[i])
            .ok_or(Error::AuthorityMismatch("Send Request signer"))
    };
    let issuer = signer(&receiver.body[500..532])?;
    authenticate_credential_under(scheme, root, generators, receiver, issuer)?;
    receiver.enrollment_fields()?;
    if has_schedule {
        let schedule = &step.fee_schedule;
        let policy = signer(&schedule.body[159..191])?;
        let digest = crate::fee_schedule::authenticate(schedule, policy, scheme, asset, root)
            .map_err(Error::AuthorityMismatch)?;
        if digest.to_repr() != body[258..290] {
            return Err(Error::AuthorityMismatch("Send Request fee schedule"));
        }
        // The complete Send preparation checks the amount, rounded fee and
        // predecessor's held schedule against these same authenticated bytes.
    } else if body[290..306] != [0; 16]
        || step.fee_schedule.body != [0; crate::fee_schedule::BODY_BYTES]
        || step.fee_schedule.signature != [0; 64]
    {
        return Err(Error::AuthorityMismatch("Send Request fee"));
    }
    let payment_key = point(&receiver.body[130..195])?;
    certificate::verify_native(&payment_key, terms.message, &req.signature)
        .map_err(|_| Error::AuthorityMismatch("Send Request receiver signature"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{chain_fixture, fee_schedule, request};

    #[test]
    fn request_authenticates_exact_receiver_and_sorted_signer_set() {
        let generators = Gens::<Eq>::new(1 << 12);
        let chain = chain_fixture::new(&generators);
        let scope = &chain.sender_context.root;
        let check = |step: &crate::send::Witness| {
            authenticate(step, &scope.scheme, &scope.root, &generators)
        };
        let mut send = chain.send.clone();
        check(&send).unwrap();
        let sign = |step: &mut crate::send::Witness| {
            step.request.signature = bootstrap::sign(
                request::commitments(&step.request).unwrap().message,
                &chain.receiver_key.d,
                "test receiver Request",
            );
        };
        let mut other = send.clone();
        other.request_originals.receiver = send.credential.clone();
        assert!(check(&other).is_err());
        let mut forged = send.clone();
        forged.request.signature[10] ^= 1;
        assert!(matches!(
            check(&forged),
            Err(Error::AuthorityMismatch("Send Request receiver signature"))
        ));
        let mut hidden = send.clone();
        hidden.fee_schedule.signature[0] = 1;
        assert!(check(&hidden).is_err());
        let mut extra = send.clone();
        extra
            .request_originals
            .certificates
            .push(chain.receiver_bootstrap.certificate.clone());
        assert!(check(&extra).is_err());
        send.request.body[354] ^= 1;
        sign(&mut send);
        assert!(check(&send).is_err());

        let policy_key = BigUint::from(8321u64);
        let policy = certificate::issue_fixture(
            &scope.scheme,
            certificate::Role::RegulatoryPolicy as u8,
            &p256::mul(&p256::generator(), &policy_key).unwrap(),
            13,
            &BigUint::from_bytes_be(&[0x11; 32]),
        );
        send.request_originals.certificates.push(policy.clone());
        send.request_originals
            .certificates
            .sort_by_key(|c| certificate::certificate_digest(c).to_repr());
        let digests: Vec<_> = send
            .request_originals
            .certificates
            .iter()
            .map(certificate::certificate_digest)
            .collect();
        send.request.body[354..386]
            .copy_from_slice(&request::certificate_set_digest(&digests).to_repr());
        let s = &mut send.fee_schedule;
        s.body[0] = 1;
        s.body[2..66].copy_from_slice(&send.credential.body[2..66]);
        s.body[66] = 1;
        s.body[74..106].fill(5);
        s.body[110] = 1;
        s.body[142..158].fill(255);
        s.body[158] = 1;
        s.body[159..191].copy_from_slice(&certificate::certificate_digest(&policy).to_repr());
        s.signature = bootstrap::sign(
            fee_schedule::message(s),
            &policy_key,
            "test Request fee schedule",
        );
        send.request.body[258..290].copy_from_slice(&fee_schedule::digest(s).to_repr());
        send.request.body[290] = 1;
        sign(&mut send);
        check(&send).unwrap();
        let mut swapped = send.clone();
        swapped.request_originals.certificates.reverse();
        assert!(check(&swapped).is_err());
        send.fee_schedule.signature[3] ^= 1;
        assert!(check(&send).is_err());
    }
}
