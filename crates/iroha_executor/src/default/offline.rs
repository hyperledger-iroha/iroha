#[cfg(test)]
#[allow(unused_imports)]
use iroha_data_model::{
    offline::{
        AppleAppAttestProof, OfflineAllowanceCommitment, OfflineBalanceProof, OfflinePlatformProof,
        OfflineSpendReceipt, OfflineToOnlineTransfer, OfflineWalletCertificate,
        OfflineWalletPolicy,
    },
    prelude::Metadata,
};
use iroha_smart_contract::data_model::{
    isi::{RegisterOfflineAllowance, SubmitOfflineToOnlineTransfer},
    offline::{AGGREGATE_PROOF_VERSION_V1, OFFLINE_REJECTION_REASON_PREFIX, compute_receipts_root},
    prelude::Numeric,
};

use super::*;
use crate::data_model::ValidationFail;

/// Validate offline-to-online bundles before delegating execution to the host.
pub fn visit_submit_offline_to_online_transfer<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    isi: &SubmitOfflineToOnlineTransfer,
) {
    let transfer = isi.transfer();

    if transfer.receipts().is_empty() {
        deny!(
            executor,
            ValidationFail::NotPermitted(format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}empty_bundle:offline bundle must include at least one receipt"
            ))
        );
    }

    let mut invoice_ids = std::collections::BTreeSet::new();
    for receipt in transfer.receipts() {
        if !invoice_ids.insert(receipt.invoice_id()) {
            deny!(
                executor,
                ValidationFail::NotPermitted(format!(
                    "{OFFLINE_REJECTION_REASON_PREFIX}invoice_duplicate:offline bundle contains duplicate invoice_id values"
                ))
            );
        }
    }

    let mut sum = Numeric::zero();
    for receipt in transfer.receipts() {
        if receipt.to() != transfer.receiver() {
            deny!(
                executor,
                ValidationFail::NotPermitted(format!(
                    "{OFFLINE_REJECTION_REASON_PREFIX}receipt_receiver_mismatch:receipt receiver mismatch"
                ))
            );
        }
        if let Some(val) = sum.checked_add(receipt.amount().clone()) {
            sum = val;
        } else {
            deny!(
                executor,
                ValidationFail::NotPermitted(format!(
                    "{OFFLINE_REJECTION_REASON_PREFIX}receipt_amount_overflow:receipt totals overflow numeric bounds"
                ))
            );
        }
    }

    if &sum != transfer.balance_proof().claimed_delta() {
        deny!(
            executor,
            ValidationFail::NotPermitted(format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}delta_mismatch:claimed delta does not match sum of receipt amounts"
            ))
        );
    }

    if let Some(envelope) = transfer.aggregate_proof() {
        if envelope.version != AGGREGATE_PROOF_VERSION_V1 {
            deny!(
                executor,
                ValidationFail::NotPermitted(format!(
                    "{OFFLINE_REJECTION_REASON_PREFIX}aggregate_proof_version_unsupported:aggregate proof version {} is not supported",
                    envelope.version
                ))
            );
        }
        let receipts_root = match compute_receipts_root(transfer.receipts()) {
            Ok(root) => root,
            Err(err) => {
                deny!(
                    executor,
                    ValidationFail::NotPermitted(format!(
                        "{OFFLINE_REJECTION_REASON_PREFIX}aggregate_proof_hash_error:failed to hash receipts for aggregate proof: {err}"
                    ))
                );
            }
        };
        if envelope.receipts_root != receipts_root {
            deny!(
                executor,
                ValidationFail::NotPermitted(format!(
                    "{OFFLINE_REJECTION_REASON_PREFIX}aggregate_proof_root_mismatch:aggregate proof receipts_root mismatch"
                ))
            );
        }
    }

    execute!(executor, isi);
}

/// Basic validation for registering offline allowances.
pub fn visit_register_offline_allowance<V: Execute + Visit + ?Sized>(
    executor: &mut V,
    isi: &RegisterOfflineAllowance,
) {
    if *isi.certificate().controller() != executor.context().authority {
        deny!(
            executor,
            ValidationFail::NotPermitted(format!(
                "{OFFLINE_REJECTION_REASON_PREFIX}unauthorized_controller:only the certificate controller may register an offline allowance"
            ))
        );
    }
    execute!(executor, isi);
}

#[cfg(test)]
mod tests {
    use core::num::NonZeroU64;

    use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
    use iroha_data_model::{
        account::AccountId,
        asset::{AssetDefinitionId, AssetId},
        block::BlockHeader,
        domain::DomainId,
        name::Name,
        offline::{
            AppleAppAttestProof, OfflineAllowanceCommitment, OfflineBalanceProof,
            OfflinePlatformProof, OfflineSpendReceipt, OfflineToOnlineTransfer,
            OfflineWalletCertificate, OfflineWalletPolicy,
        },
        prelude::Metadata,
    };

    use super::*;
    use crate::{
        Execute, Iroha,
        data_model::executor::Result as ExecResult,
        prelude::{Context, Visit},
    };

    struct StubExecutor {
        host: Iroha,
        context: Context,
        verdict: ExecResult,
    }

    impl StubExecutor {
        fn new(authority: AccountId) -> Self {
            let header = BlockHeader::new(
                NonZeroU64::new(1).expect("height > 0"),
                None,
                None,
                None,
                0,
                0,
            );
            let context = Context {
                authority,
                curr_block: header,
            };
            Self {
                host: Iroha,
                context,
                verdict: Ok(()),
            }
        }
    }

    impl Execute for StubExecutor {
        fn host(&self) -> &Iroha {
            &self.host
        }

        fn context(&self) -> &Context {
            &self.context
        }

        fn context_mut(&mut self) -> &mut Context {
            &mut self.context
        }

        fn verdict(&self) -> &ExecResult {
            &self.verdict
        }

        fn deny(&mut self, reason: ValidationFail) {
            self.verdict = Err(reason);
        }
    }

    impl Visit for StubExecutor {}

    fn sample_account(seed: u8, domain: &str) -> AccountId {
        let keypair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        let domain_id: DomainId = domain.parse().expect("valid domain");
        AccountId::new(domain_id, keypair.public_key().clone())
    }

    fn sample_asset(owner: &AccountId) -> AssetId {
        let definition = AssetDefinitionId::new(
            owner.domain().clone(),
            "sample".parse::<Name>().expect("valid name"),
        );
        AssetId::new(definition, owner.clone())
    }

    fn sample_certificate(controller: &AccountId) -> OfflineWalletCertificate {
        OfflineWalletCertificate::new(
            controller.clone(),
            OfflineAllowanceCommitment::new(
                sample_asset(controller),
                Numeric::new(1_000, 0),
                vec![0x11; 32],
            ),
            controller.signatory().clone(),
            vec![0xAA],
            0,
            1,
            OfflineWalletPolicy::new(Numeric::new(1_000, 0), Numeric::new(500, 0), 1),
            Signature::from_bytes(&[0xBB]),
            Metadata::default(),
            None,
            None,
            None,
        )
    }

    fn sample_receipt(
        receiver: &AccountId,
        amount: Numeric,
        invoice_id: String,
    ) -> OfflineSpendReceipt {
        let asset = sample_asset(receiver);
        OfflineSpendReceipt::new(
            Hash::new(b"offline-tx"),
            receiver.clone(),
            receiver.clone(),
            asset,
            amount,
            1,
            invoice_id,
            OfflinePlatformProof::AppleAppAttest(AppleAppAttestProof::new(
                "aa".to_owned(),
                1,
                vec![0xAA],
                Hash::new(b"challenge"),
            )),
            None,
            sample_certificate(receiver),
            Signature::from_bytes(&[0xCC]),
        )
    }

    fn build_submit_transfer(
        receiver: &AccountId,
        receipt_amounts: &[Numeric],
        claimed_delta: Numeric,
    ) -> SubmitOfflineToOnlineTransfer {
        build_submit_transfer_with_invoice_ids(receiver, receipt_amounts, claimed_delta, &[])
    }

    fn build_submit_transfer_with_invoice_ids(
        receiver: &AccountId,
        receipt_amounts: &[Numeric],
        claimed_delta: Numeric,
        invoice_ids: &[String],
    ) -> SubmitOfflineToOnlineTransfer {
        let receipts = receipt_amounts
            .iter()
            .cloned()
            .enumerate()
            .map(|(idx, amount)| {
                let invoice_id = invoice_ids
                    .get(idx)
                    .cloned()
                    .unwrap_or_else(|| format!("inv-{idx:03}"));
                sample_receipt(receiver, amount, invoice_id)
            })
            .collect();

        let balance_proof = OfflineBalanceProof::new(
            OfflineAllowanceCommitment::new(
                sample_asset(receiver),
                Numeric::new(1_000, 0),
                vec![0x10; 32],
            ),
            vec![0x20; 32],
            claimed_delta,
            None,
        );

        let transfer = OfflineToOnlineTransfer::new(
            Hash::new(b"bundle"),
            receiver.clone(),
            receiver.clone(),
            receipts,
            balance_proof,
            None,
            None,
            None,
        );

        SubmitOfflineToOnlineTransfer::new(transfer)
    }

    #[test]
    fn denies_mismatched_claimed_delta() {
        let receiver = sample_account(0x01, "sbp");
        let isi = build_submit_transfer(
            &receiver,
            &[Numeric::new(200, 0), Numeric::new(50, 0)],
            Numeric::new(249, 0),
        );
        let mut executor = StubExecutor::new(receiver);

        visit_submit_offline_to_online_transfer(&mut executor, &isi);

        assert!(matches!(
            executor.verdict(),
            Err(ValidationFail::NotPermitted(msg)) if msg.contains("claimed delta does not match")
        ));
    }

    #[test]
    fn denies_duplicate_invoice_ids() {
        let receiver = sample_account(0x03, "sbp");
        let invoice_ids = vec!["dup-invoice".to_owned(), "dup-invoice".to_owned()];
        let isi = build_submit_transfer_with_invoice_ids(
            &receiver,
            &[Numeric::new(10, 0), Numeric::new(20, 0)],
            Numeric::new(30, 0),
            &invoice_ids,
        );
        let mut executor = StubExecutor::new(receiver);

        visit_submit_offline_to_online_transfer(&mut executor, &isi);

        assert!(matches!(
            executor.verdict(),
            Err(ValidationFail::NotPermitted(msg)) if msg.contains("invoice_duplicate")
        ));
    }

    #[test]
    fn accepts_matching_claimed_delta() {
        let receiver = sample_account(0x02, "sbp");
        let isi = build_submit_transfer(
            &receiver,
            &[Numeric::new(120, 0), Numeric::new(30, 0)],
            Numeric::new(150, 0),
        );
        let mut executor = StubExecutor::new(receiver);

        visit_submit_offline_to_online_transfer(&mut executor, &isi);

        assert!(executor.verdict().is_ok());
    }
}
