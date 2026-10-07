//! Original semantic tapes, without upgrading incoming soft verdicts into native admission.

use super::*;

fn digest(domain: &[u8; 8], original: &[u8]) -> [u8; 32] {
    kagemusha_wallet_poseidon_bytes_v1(u64::from_le_bytes(*domain), original)
}

// Preserve the carried operation/version/capsule/payment and signature even when
// they are invalid. Canonical statement field conversion is a shape boundary;
// receipt semantics, signature and membership belong to the soft source owners.
fn receipt(
    signer: &KagemushaWalletReceiptSignerV1,
    statement: &KagemushaWalletStatementV1,
    proof: [u8; 32],
    original: &KagemushaWalletReceiptV1,
) -> Result<(Vec<u8>, [u8; 32]), Error> {
    let body = KagemushaWalletReceiptBodyV1 {
        version: original.version,
        scheme_id: signer.scheme_id,
        wallet_id: signer.wallet_id,
        provider_contract: signer.provider_contract,
        sequence: statement.sequence,
        operation_id: original.operation_id,
        predecessor: statement.predecessor,
        successor: statement.successor,
        statement_digest: authority(statement.statement_digest())?,
        proof_digest: proof,
        capsule_digest: original.capsule_digest,
        payment_digest: original.payment_digest,
    };
    let object = kagemusha_wallet_signed_object_digest_v1(
        KagemushaWalletObjectDigestDomainV1::Receipt,
        &body.signing_message(),
        &original.signature,
    );
    Ok((signed_tape(body.transcript(), &original.signature), object))
}

fn credited_tape(tag: u8, credit: [u8; 32], payment: [u8; 32], evidence: [u8; 32]) -> Vec<u8> {
    let mut tape = vec![1, 0, tag];
    for word in [credit, payment, evidence] {
        tape.extend_from_slice(&word);
    }
    tape
}

fn opening_tape(original: &KagemushaWalletCreditOpeningV1) -> Result<Vec<u8>, Error> {
    if original.siblings.len() != 32 * 32 {
        return Err(Error::Authority);
    }
    let mut tape = Vec::with_capacity(1125);
    tape.extend_from_slice(&original.credit_id);
    tape.extend_from_slice(&original.payment_digest);
    tape.push(u8::from(original.burned));
    tape.extend_from_slice(&original.next_key);
    tape.extend_from_slice(&original.slot.to_le_bytes());
    tape.extend_from_slice(&original.siblings);
    Ok(tape)
}

pub(super) fn original(
    bytes: &[u8],
    request: &KagemushaWalletRequestV1,
    payment_digest: &[u8; 32],
    expected_digest: &[u8; 32],
    proposal: ArchiveIncomingWitnessV1,
) -> Result<native::Evidence, Error> {
    let credited: KagemushaWalletCreditedV1 = decode(bytes)?;
    if credited.version != 1 || credited.scheme_id != credited.evidence.statement().scheme_id {
        return Err(Error::Authority);
    }
    let evidence = match (credited.evidence, proposal) {
        (
            KagemushaWalletCreditedEvidenceV1::Receive { package },
            ArchiveIncomingWitnessV1::Receive(mode),
        ) => {
            if package.version != 1
                || package.lineage != KagemushaWalletLineageSlotV1::None
                || package.statement.effect.kind() != KagemushaWalletOperationKindV1::Receive
            {
                return Err(Error::Authority);
            }
            let mut proof_tape = u32::try_from(package.step_proof.bytes.len())
                .map_err(|_| Error::Authority)?
                .to_le_bytes()
                .to_vec();
            proof_tape.extend_from_slice(&package.step_proof.bytes);
            let proof = digest(b"kgwstep1", &proof_tape);
            let signer = authority(KagemushaWalletReceiptSignerV1::from_credential(
                &request.receiver_credential,
            ))?;
            let (receipt, receipt_digest) =
                receipt(&signer, &package.statement, proof, &package.receipt)?;
            let package_digest = authority(kagemusha_wallet_package_digest_v1(
                &authority(package.statement.statement_digest())?,
                &proof,
                &receipt_digest,
            ))?;
            native::Evidence::Receive {
                statement: fields(authority(package.statement.field_items())?)?,
                receipt,
                sigma: package.step_proof.bytes,
                credited: credited_tape(
                    1,
                    request.body.credit_id(),
                    *payment_digest,
                    package_digest,
                ),
                mode: *mode,
            }
        }
        (
            KagemushaWalletCreditedEvidenceV1::Status { status },
            ArchiveIncomingWitnessV1::Status(witness),
        ) => {
            let signer = KagemushaWalletReceiptSignerV1 {
                scheme_id: status.lineage.public.scheme_id,
                wallet_id: status.lineage.public.wallet_id,
                provider_contract: kagemusha_wallet_provider_contract_v1(),
                payment_key: status.lineage.public.payment_key,
            };
            let (receipt, receipt_digest) = receipt(
                &signer,
                &status.statement,
                status.proof_digest,
                &status.receipt,
            )?;
            let opening = opening_tape(&status.opening)?;
            let mut tape = status.version.to_le_bytes().to_vec();
            for word in [
                authority(status.statement.statement_digest())?,
                status.proof_digest,
                receipt_digest,
                status.lineage.lineage_digest(),
                digest(b"kgwcopn1", &opening),
            ] {
                tape.extend_from_slice(&word);
            }
            native::Evidence::Status {
                statement: fields(authority(status.statement.field_items())?)?,
                receipt,
                omega: status.lineage.bytes(),
                credited: credited_tape(
                    2,
                    request.body.credit_id(),
                    *payment_digest,
                    digest(b"kgwcsts1", &tape),
                ),
                status: tape,
                credit_opening: opening,
                witness,
            }
        }
        _ => return Err(Error::Authority),
    };
    let tape = match &evidence {
        native::Evidence::Receive { credited, .. } | native::Evidence::Status { credited, .. } => {
            credited
        }
    };
    if digest(b"kgwcrdd1", tape) != *expected_digest {
        return Err(Error::Authority);
    }
    Ok(evidence)
}
