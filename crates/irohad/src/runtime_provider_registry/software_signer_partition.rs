impl IrohaRuntimeProviderBindingsV1 {
    /// Partition the complete catalog into the closed external-software-signer
    /// subset and every other deployment-owned provider without dropping entries.
    pub(crate) fn partition_external_software_signers_v1(&self) -> (Self, Self) {
        let mut signers = Vec::new();
        let mut base = Vec::new();
        for binding in &self.bindings {
            if matches!(
                binding.slot,
                IrohaRuntimeProviderSlotV1::GovernanceDagSigner
                    | IrohaRuntimeProviderSlotV1::StreamTokenSigner
                    | IrohaRuntimeProviderSlotV1::ProofOutcomeTransactionSigner
                    | IrohaRuntimeProviderSlotV1::RepairTransactionSigner
                    | IrohaRuntimeProviderSlotV1::ReserveTransactionSigner
                    | IrohaRuntimeProviderSlotV1::OrderbookTransactionSigner
                    | IrohaRuntimeProviderSlotV1::EvidenceViewerReceiptSigner
                    | IrohaRuntimeProviderSlotV1::PopCredentialProviderRegistry
                    | IrohaRuntimeProviderSlotV1::PotrGatewaySigner
                    | IrohaRuntimeProviderSlotV1::PotrProviderSigner
                    | IrohaRuntimeProviderSlotV1::BillingStatementSigner
            ) {
                signers.push(binding.clone());
            } else {
                base.push(binding.clone());
            }
        }
        let catalog = |bindings| Self {
            chain_id: self.chain_id.clone(),
            bindings,
        };
        (catalog(signers), catalog(base))
    }
}
