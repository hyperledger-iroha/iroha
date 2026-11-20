//! Exact keyed State positions needed by the native-owner payout verifier.

use super::Kagemusha;
use crate::Result;
use iroha_data_model::{
    kagemusha::KagemushaWalletPayoutKeyV1,
    sumeragi_finality::{KeyedStateRowsV1, keyed_state::StateKeyV1},
};
use norito::codec::Encode as _;

impl Kagemusha<'_> {
    /// Fetch the exact payout and runtime-selection rows at the wallet's selected tip.
    ///
    /// The account needs `CanReadStateRow` for `world.kagemusha_wallet_ledger` with
    /// `key_hex = hex(Fee(credit).ledger_key(scheme).encode())` and for the
    /// `world.runtime_selection` cell with `key_hex = ""`. These bytes are unverified
    /// evidence for the native owner's acknowledgement, alongside the original payout
    /// and canonical `(RootSelectionV1, Vec<DefinitionDescriptorV1>)` schema bundle.
    /// The native owner authenticates the selection and payout at the same certified root.
    ///
    /// # Errors
    /// See [`super::AccountClient::state_rows`]. Missing rows remain proven absence;
    /// only the native owner can decide whether the payout is acknowledged.
    pub async fn fee_payout_rows(
        &self,
        height: u64,
        scheme: [u8; 32],
        credit: [u8; 32],
    ) -> Result<KeyedStateRowsV1> {
        self.account
            .state_rows(height, &payout_rows(scheme, credit))
            .await
    }
}

fn payout_rows(scheme: [u8; 32], credit: [u8; 32]) -> [StateKeyV1; 2] {
    [
        StateKeyV1 {
            table: "world.kagemusha_wallet_ledger".into(),
            key: KagemushaWalletPayoutKeyV1::Fee(credit)
                .ledger_key(scheme)
                .encode(),
        },
        StateKeyV1 {
            table: "world.runtime_selection".into(),
            key: Vec::new(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payout_rows_are_exact_sorted_keys_including_the_certified_selection() {
        let rows = payout_rows([1; 32], [2; 32]);
        assert!(rows[0] < rows[1]);
        let key: iroha_data_model::kagemusha::KagemushaWalletLedgerKeyV1 =
            norito::codec::Decode::decode(&mut rows[0].key.as_slice()).unwrap();
        assert_eq!(key.components(), (5, [1; 32], [2; 32], [0; 32]));
        assert_eq!(rows[1].table, "world.runtime_selection");
        assert!(rows[1].key.is_empty());
    }
}
