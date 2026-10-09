//! Bounded certificate DATA reads for native wallet finality verification.
//! The phone's authenticated epoch owner, never this transport, grants finality.

use std::{num::NonZeroU64, time::Instant};

use iroha_allocation::{AllocationBudget, ChargedBuffer};
use iroha_core::{
    kagemusha_wallet_v1::CommittedLoadReceipts, state::StateView, sumeragi::finality,
};
use iroha_data_model::{
    account::AccountId,
    isi::kagemusha_wallet::KagemushaWalletLoadReceiptV1,
    kagemusha::{
        KAGEMUSHA_WALLET_LOAD_FINALITY_MAX_BYTES_V1, KagemushaWalletLoadFinalityV1,
        kagemusha_wallet_account_digest_v1,
    },
    sumeragi_finality::{
        ExecutionResultCommitment, MAX_COMMIT_CERTIFICATE_BYTES_V1, SumeragiCommitCertificateV1,
    },
};

type Result<T> = std::result::Result<T, &'static str>;

fn require_payer(payer: &AccountId, receipt: &KagemushaWalletLoadReceiptV1) -> Result<()> {
    receipt
        .validate()
        .map_err(|_| "invalid committed receipt")?;
    if receipt.payer_account_digest
        != kagemusha_wallet_account_digest_v1(payer).map_err(|_| "invalid payer identity")?
    {
        return Err("committed receipt belongs to another payer");
    }
    Ok(())
}

fn require_selected_data(
    certificate: &SumeragiCommitCertificateV1,
    height: u64,
    boundary: bool,
) -> Result<()> {
    if certificate
        .height()
        .map_err(|_| "certificate header differs")?
        != height
    {
        return Err("certificate height differs from selected original");
    }
    let result = ExecutionResultCommitment::decode(&certificate.result_preimage)
        .map_err(|_| "certificate result shape differs")?;
    if result.height != height || (boundary && result.schedule.boundary.is_none()) {
        return Err("certificate is not the selected boundary or receipt");
    }
    Ok(())
}

fn certificate_decode<T>(
    certificate: &SumeragiCommitCertificateV1,
    budget: &AllocationBudget,
    decode: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let extent = certificate
        .consensus_header
        .len()
        .checked_add(certificate.commit_qc.len())
        .and_then(|bytes| bytes.checked_add(certificate.result_preimage.len()))
        .filter(|bytes| *bytes <= MAX_COMMIT_CERTIFICATE_BYTES_V1)
        .ok_or("certificate decode extent differs")?;
    let limits = norito::canonical_decode_limits(extent);
    let _charge = budget
        .try_reserve_bytes(limits.max_total_allocated_bytes())
        .map_err(|_| "certificate decode allocation unavailable")?;
    norito::with_decode_limits_scope(limits, decode)
}

fn selected_certificate(
    view: &StateView<'_>,
    receipt_height: u64,
    boundary: Option<u64>,
    budget: &AllocationBudget,
    deadline: Instant,
) -> Result<finality::NativeCommitCertificateDataV1> {
    let height = boundary.unwrap_or(receipt_height);
    if height < 2
        || boundary.is_some_and(|height| height >= receipt_height)
        || Instant::now() >= deadline
    {
        return Err("certificate selection deadline or height differs");
    }
    let original = finality::read_commit_certificate(
        view,
        NonZeroU64::new(height).ok_or("zero certificate height")?,
        budget,
        deadline,
    )
    .map_err(|_| "committed native certificate unavailable")?;
    certificate_decode(original.certificate(), budget, || {
        require_selected_data(original.certificate(), height, boundary.is_some())
    })?;
    if Instant::now() >= deadline {
        return Err("native certificate read deadline expired");
    }
    Ok(original)
}

fn response_bytes(
    bytes: &[u8],
    maximum: usize,
    budget: &AllocationBudget,
    deadline: Instant,
) -> Result<ChargedBuffer<u8>> {
    if bytes.is_empty() || bytes.len() > maximum || Instant::now() >= deadline {
        return Err("native certificate response extent or deadline differs");
    }
    let mut output = ChargedBuffer::new(bytes.len(), budget)
        .map_err(|_| "certificate response allocation unavailable")?;
    output
        .append(bytes)
        .map_err(|_| "certificate response extent differs")?;
    if Instant::now() >= deadline {
        return Err("native certificate response deadline expired");
    }
    Ok(output)
}

/// Read exactly one named boundary as DATA; no source-supplied successor is followed.
pub(super) fn epoch(
    view: &StateView<'_>,
    payer: &AccountId,
    receipt: &KagemushaWalletLoadReceiptV1,
    boundary: u64,
    budget: &AllocationBudget,
    deadline: Instant,
) -> Result<ChargedBuffer<u8>> {
    require_payer(payer, receipt)?;
    let original =
        selected_certificate(view, receipt.block_height, Some(boundary), budget, deadline)?;
    let bytes = certificate_decode(original.certificate(), budget, || {
        original
            .certificate()
            .to_canonical_bytes()
            .map_err(|_| "native epoch certificate extent refused")
    })?;
    response_bytes(&bytes, MAX_COMMIT_CERTIFICATE_BYTES_V1, budget, deadline)
}

/// Return only the receipt certificate and counted event path as untrusted DATA.
/// The phone must authenticate its retained epoch and BLS certificate before Advance.
pub(super) fn evidence(
    view: &StateView<'_>,
    source: &CommittedLoadReceipts<'_, '_>,
    payer: &AccountId,
    receipt: &KagemushaWalletLoadReceiptV1,
    budget: &AllocationBudget,
    deadline: Instant,
) -> Result<ChargedBuffer<u8>> {
    require_payer(payer, receipt)?;
    let original = selected_certificate(view, receipt.block_height, None, budget, deadline)?;
    let (certificate, _certificate_charge) = original.into_parts();
    let event = source
        .event_path_for(
            payer,
            &receipt.scheme_id,
            &receipt.wallet_id,
            &receipt.request_id,
        )
        .map_err(|_| "committed Load event unavailable")?;
    let evidence = KagemushaWalletLoadFinalityV1 {
        version: 1,
        receipt_digest: receipt.receipt_digest().map_err(|_| "invalid receipt")?,
        certificate,
        event_proof: event
            .proof()
            .map_err(|_| "committed event path unavailable")?,
    };
    let bytes = evidence
        .to_canonical_bytes()
        .map_err(|_| "direct Load finality extent refused")?;
    // Acquire outgoing custody before releasing certificate and encoding coverage.
    response_bytes(
        &bytes,
        KAGEMUSHA_WALLET_LOAD_FINALITY_MAX_BYTES_V1,
        budget,
        deadline,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_core::{
        state::{StateReadOnly as _, World},
        sumeragi::test_chain::{CertifiedTestChain, TestChainConfig},
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        block::decode_framed_signed_block,
        sumeragi_finality::{SumeragiCommitVerifierV1, SumeragiFinalityVerifier},
    };
    use std::{num::NonZeroUsize, time::Duration};

    fn receipt_at(payer: &AccountId, height: u64) -> KagemushaWalletLoadReceiptV1 {
        KagemushaWalletLoadReceiptV1 {
            version: 1,
            scheme_id: [1; 32],
            asset_digest: [2; 32],
            wallet_id: [3; 32],
            request_id: [4; 32],
            ordinal: 1,
            amount: 7,
            online_charge: 0,
            charge_quote: [0; 32],
            transaction_hash: [5; 32],
            block_height: height,
            payer_account_digest: kagemusha_wallet_account_digest_v1(payer).unwrap(),
        }
    }

    fn verifier(view: &StateView<'_>) -> SumeragiCommitVerifierV1 {
        let genesis = finality::build_proof(view, 1).unwrap();
        let signed = decode_framed_signed_block(&genesis.block_wire).unwrap();
        let native =
            SumeragiFinalityVerifier::new(&signed, view.chain_id().as_str(), genesis.committee)
                .unwrap();
        SumeragiCommitVerifierV1::new(&native).unwrap()
    }

    #[test]
    fn receipt_source_reads_one_exact_frame_without_genesis_or_history_replay() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::default(), 1_000)).unwrap();
        chain.commit(Vec::new());
        chain.commit(Vec::new());
        let view = chain.state().view();
        let mut phone = verifier(&view);
        for height in [1, 2] {
            chain
                .kura()
                .corrupt_native_frame_for_test(NonZeroUsize::new(height).unwrap());
        }
        let budget = AllocationBudget::new(128 << 20);
        let deadline = Instant::now() + Duration::from_secs(30);
        let original = selected_certificate(&view, 3, None, &budget, deadline).unwrap();
        assert_eq!(phone.verify(original.certificate()).unwrap().height(), 3);
        assert!(
            certificate_decode(original.certificate(), &AllocationBudget::new(0), || Ok(()))
                .is_err()
        );
        let mut tampered = original.certificate().clone();
        *tampered.commit_qc.last_mut().unwrap() ^= 1;
        // Shape-matched transport DATA is not an authority capability.
        require_selected_data(&tampered, 3, false).unwrap();
        assert!(phone.verify(&tampered).is_err());
        drop(original);
        assert_eq!(budget.reserved_bytes(), 0);
        for (height, boundary, until) in [
            (1, None, deadline),
            (3, Some(3), deadline),
            (3, Some(4), deadline),
            (3, None, Instant::now()),
        ] {
            assert!(selected_certificate(&view, height, boundary, &budget, until).is_err());
        }
        assert!(selected_certificate(&view, 3, None, &AllocationBudget::new(0), deadline).is_err());
    }

    #[test]
    fn epoch_source_requires_exact_boundary_before_receipt_without_following_successors() {
        let mut chain = CertifiedTestChain::npos_boundary_fixture();
        chain.commit(Vec::new());
        let view = chain.state().view();
        let mut phone = verifier(&view);
        chain
            .kura()
            .corrupt_native_frame_for_test(NonZeroUsize::new(1).unwrap());
        let budget = AllocationBudget::new(128 << 20);
        let deadline = Instant::now() + Duration::from_secs(30);
        // H11 is not present: selecting H10 must not read its advertised successor.
        let original = selected_certificate(&view, 11, Some(10), &budget, deadline).unwrap();
        assert_eq!(phone.verify(original.certificate()).unwrap().height(), 10);
        let payer = AccountId::new(
            KeyPair::from_seed(vec![0x61; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let response = epoch(
            &view,
            &payer,
            &receipt_at(&payer, 11),
            10,
            &budget,
            deadline,
        )
        .unwrap();
        let decoded = SumeragiCommitCertificateV1::decode_canonical(response.as_slice()).unwrap();
        assert_eq!(&decoded, original.certificate());
        assert_eq!(phone.verify(&decoded).unwrap().height(), 10);
        assert!(require_selected_data(original.certificate(), 9, true).is_err());
        assert!(selected_certificate(&view, 11, Some(2), &budget, deadline).is_err());
        drop(response);
        drop(original);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn response_custody_is_retained_and_all_bounds_refuse() {
        let budget = AllocationBudget::new(16);
        let deadline = Instant::now() + Duration::from_secs(30);
        assert!(response_bytes(&[], 4, &budget, deadline).is_err());
        assert!(response_bytes(&[1, 2], 1, &budget, deadline).is_err());
        assert!(response_bytes(&[1], 4, &budget, Instant::now()).is_err());
        assert!(response_bytes(&[1], 4, &AllocationBudget::new(0), deadline).is_err());
        let response = response_bytes(&[1, 2], 4, &budget, deadline).unwrap();
        assert_eq!(response.as_slice(), &[1, 2]);
        assert_eq!(budget.reserved_bytes(), 2);
        drop(response);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn receipt_selection_requires_exact_payer_and_valid_non_genesis_load() {
        let payer = AccountId::new(
            KeyPair::from_seed(vec![0x61; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let other = AccountId::new(
            KeyPair::from_seed(vec![0x62; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        let receipt = receipt_at(&payer, 2);
        require_payer(&payer, &receipt).unwrap();
        assert!(require_payer(&other, &receipt).is_err());
        for changed in [
            KagemushaWalletLoadReceiptV1 {
                block_height: 1,
                ..receipt
            },
            KagemushaWalletLoadReceiptV1 {
                amount: 0,
                ..receipt
            },
        ] {
            assert!(require_payer(&payer, &changed).is_err());
        }
    }
}
