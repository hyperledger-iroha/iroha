//! Minimal Nexus App Facade transfer recipe with fake wallet/Torii dependencies.

use std::{num::NonZeroU32, time::Duration};

use iroha::{
    crypto::{Algorithm, KeyPair, Signature},
    data_model::{
        asset::AssetDefinitionId,
        prelude::{AccountId, AssetId, ChainId, Metadata, Numeric},
        transaction::SignedTransaction,
    },
    nexus_app::{
        NexusAppClient, NexusAppConfig, NexusAppError, NexusApprovedAccount, NexusConnectOptions,
        NexusConnectSession, NexusConnectTransport, NexusFinalizeOptions, NexusSignableTransaction,
        NexusToriiSubmitter, NexusTransferInput, NexusTransferReceipt, NexusWalletSignature,
    },
};

#[derive(Clone)]
struct DemoConnectTransport {
    key_pair: KeyPair,
}

impl NexusConnectTransport for DemoConnectTransport {
    fn start_connect(
        &self,
        _config: &NexusAppConfig,
        options: NexusConnectOptions,
    ) -> Result<NexusConnectSession, NexusAppError> {
        let sid = options.sid.unwrap_or_else(|| "sid-demo-1".to_owned());
        Ok(NexusConnectSession {
            sid: sid.clone(),
            wallet_launch_uri: format!("iroha://connect?sid={sid}&role=wallet"),
            app_launch_uri: None,
            token_app: Some("demo-app-token".to_owned()),
            token_wallet: Some("demo-wallet-token".to_owned()),
            token_management: Some("demo-management-token".to_owned()),
            token_relay: Some("demo-relay-token".to_owned()),
            approved_account: None,
            signing_public_key: None,
        })
    }

    fn await_approval(
        &self,
        _session: &mut NexusConnectSession,
    ) -> Result<NexusApprovedAccount, NexusAppError> {
        Ok(NexusApprovedAccount {
            account_id: AccountId::new(self.key_pair.public_key().clone()),
            signing_public_key: self.key_pair.public_key().clone(),
        })
    }

    fn request_signature(
        &self,
        _session: &NexusConnectSession,
        signable: &NexusSignableTransaction,
    ) -> Result<NexusWalletSignature, NexusAppError> {
        let payload_hash = hex::decode(&signable.payload_hash_hex)
            .map_err(|err| NexusAppError::InvalidSignature(err.to_string()))?;
        let signature = Signature::new(self.key_pair.private_key(), &payload_hash);
        Ok(NexusWalletSignature {
            algorithm: signable.signature_algorithm,
            signature: signature.payload().to_vec(),
        })
    }
}

#[derive(Clone)]
struct DemoToriiSubmitter;

impl NexusToriiSubmitter for DemoToriiSubmitter {
    fn submit_and_wait(
        &self,
        transaction: &SignedTransaction,
        _options: NexusFinalizeOptions,
    ) -> Result<NexusTransferReceipt, NexusAppError> {
        let hash_hex = hex::encode(transaction.hash().as_ref());
        Ok(NexusTransferReceipt {
            signed_transaction: transaction.clone(),
            signed_transaction_hash_hex: hash_hex,
            status: None,
        })
    }
}

fn transfer_input(authority: AccountId) -> NexusTransferInput {
    let asset_definition = AssetDefinitionId::from_uuid_bytes_unchecked([
        0x7e, 0xad, 0x8e, 0xf0, 0x22, 0x22, 0x42, 0x22, 0x82, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22,
        0x22,
    ]);
    NexusTransferInput {
        source_asset_id: AssetId::new(asset_definition, authority.clone()),
        quantity: Numeric::new(1234_u32, 2),
        destination_account_id: authority.clone(),
        authority: Some(authority),
        metadata: Metadata::default(),
        creation_time_ms: Some(1_700_000_000_000),
        ttl: Some(Duration::from_secs(30)),
        nonce: Some(NonZeroU32::new(7).expect("non-zero nonce")),
    }
}

fn main() -> Result<(), NexusAppError> {
    let key_pair = KeyPair::random_with_algorithm(Algorithm::Ed25519);
    let account_id = AccountId::new(key_pair.public_key().clone());
    let client = NexusAppClient::new(
        NexusAppConfig {
            authority: Some(account_id.clone()),
            ..NexusAppConfig::new(ChainId::from("test-chain"))
        },
        DemoConnectTransport { key_pair },
        DemoToriiSubmitter,
    );

    let mut session = client.start_connect(NexusConnectOptions {
        sid: Some("sid-demo-1".to_owned()),
        node: None,
    })?;
    let approved = client.await_approval(&mut session)?;
    let receipt = client.transfer_with_wallet(
        &session,
        transfer_input(approved.account_id),
        NexusFinalizeOptions::default(),
    )?;

    println!("wallet URI: {}", session.wallet_launch_uri);
    println!(
        "signed transaction hash: {}",
        receipt.signed_transaction_hash_hex
    );
    Ok(())
}
