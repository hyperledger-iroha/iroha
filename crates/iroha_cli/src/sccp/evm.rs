//! EVM destination actions of `iroha sccp` for Ethereum and BSC (`specs/sccp.md` §7.1, §7.4).
//!
//! Every bundle and rotation chain read from Taira is verified locally against the
//! destination's own `rosterState()` before a transaction is built; the key comes only from an
//! owner-only key file.

use std::path::Path;

use eyre::{Result, WrapErr as _, eyre};
use iroha_sccp::{
    api::{SccpMessageProofBundleV1, SccpRotationChainV1},
    v1::{
        evm_abi::{ViewCallV1, decode_roster_state_return},
        proof::DestinationV1,
        roster::RosterStateV1,
    },
};
use iroha_sccp_rpc::{
    EndpointSet, EvmClient, FailoverPolicy, HttpConfig, HttpTransport,
    evm::{BlockId, BlockTag, EvmCallRequest},
};
use iroha_sccp_wallet::pure::{
    bundle::{BundlePurposeV1, DestinationContextV1, verify_message_bundle},
    evm::{
        Eip1559TransactionV1, EvmSigningKey, eip1559_chain_id, finalize_calldata, rlp_bytes,
        rlp_list, rlp_uint, rotate_rosters_calldata,
    },
    rotation::verify_rotation_chain,
};

use iroha::data_model::bridge::SccpNetworkV1;

/// One EVM deployment reached through a JSON-RPC endpoint.
pub(super) struct EvmDestination {
    client: EvmClient,
    network: SccpNetworkV1,
    contract: [u8; 20],
    destination: DestinationV1,
}

impl EvmDestination {
    /// Connect to `rpc_url` and check that it serves `network`.
    pub(super) fn connect(
        rpc_url: &str,
        network: SccpNetworkV1,
        contract: [u8; 20],
        destination: DestinationV1,
    ) -> Result<Self> {
        Ok(Self {
            client: connect_chain(rpc_url, network)?,
            network,
            contract,
            destination,
        })
    }

    /// Read the deployment's `rosterState()` at the latest block.
    pub(super) fn roster_state(&self) -> Result<RosterStateV1> {
        let data = self
            .client
            .call(
                &EvmCallRequest::new(self.contract, ViewCallV1::RosterState.calldata()),
                BlockId::Tag(BlockTag::Latest),
            )
            .map_err(|error| eyre!("rosterState(): {error}"))?;
        decode_roster_state_return(&data).map_err(|error| eyre!("rosterState() return: {error:?}"))
    }

    /// Destination time of the latest block in milliseconds.
    pub(super) fn now_ms(&self) -> Result<u64> {
        let block = self
            .client
            .block_by_number(BlockTag::Latest, false)
            .map_err(|error| eyre!("latest block: {error}"))?
            .ok_or_else(|| eyre!("the RPC endpoint has no latest block"))?;
        block
            .header
            .timestamp
            .checked_mul(1_000)
            .ok_or_else(|| eyre!("block timestamp overflows"))
    }

    fn context(&self, taira_network_id: [u8; 32]) -> Result<DestinationContextV1> {
        Ok(DestinationContextV1 {
            taira_network_id,
            destination: self.destination,
            roster_state: self.roster_state()?,
            now_ms: self.now_ms()?,
        })
    }

    /// Verify `bundle` for this deployment and return the finalize calldata.
    pub(super) fn finalize_call(
        &self,
        bundle: &SccpMessageProofBundleV1,
        taira_network_id: [u8; 32],
    ) -> Result<Vec<u8>> {
        let context = self.context(taira_network_id)?;
        let verified = verify_message_bundle(bundle, &context, BundlePurposeV1::Finalize).map_err(
            |error| eyre!("the proof bundle does not verify for this deployment: {error}"),
        )?;
        finalize_calldata(&verified).map_err(|error| eyre!("finalize calldata: {error:?}"))
    }

    /// Verify `chain` from the deployment's roster to `target_generation` and return one
    /// `rotateRosters` calldata per batch (empty when already current).
    pub(super) fn rotation_calls(
        &self,
        chain: &SccpRotationChainV1,
        target_generation: u64,
        taira_network_id: [u8; 32],
    ) -> Result<Vec<Vec<u8>>> {
        let state = self.roster_state()?;
        let plan = verify_rotation_chain(
            chain,
            &state,
            target_generation,
            &taira_network_id,
            self.now_ms()?,
        )
        .map_err(|error| eyre!("the rotation chain does not verify: {error}"))?;
        Ok(rotate_rosters_calldata(&plan))
    }

    /// Sign `data` as a call of the deployment with `key` and broadcast it, returning the
    /// transaction hash.
    pub(super) fn send(&self, key: &EvmSigningKey, data: Vec<u8>) -> Result<[u8; 32]> {
        send_transaction(&self.client, self.network, key, Some(self.contract), data)
            .map(|(hash, _)| hash)
    }
}

/// Connect to `rpc_url` and check that it serves `network`.
pub(super) fn connect_chain(rpc_url: &str, network: SccpNetworkV1) -> Result<EvmClient> {
    let endpoints =
        EndpointSet::parse(&[rpc_url], &[]).map_err(|error| eyre!("RPC endpoint: {error}"))?;
    let transport = HttpTransport::new(endpoints, HttpConfig::default(), FailoverPolicy::default())
        .map_err(|error| eyre!("RPC transport: {error}"))?;
    let client = EvmClient::new(transport);
    let expected = eip1559_chain_id(network).map_err(|error| eyre!("{error:?}"))?;
    let served = client
        .chain_id()
        .map_err(|error| eyre!("eth_chainId: {error}"))?;
    if served != expected {
        return Err(eyre!(
            "the RPC endpoint serves chain {served}, not {} ({expected})",
            network.profile_key()
        ));
    }
    Ok(client)
}

/// Sign and broadcast a call of `to` (a contract creation when `None`) with `key`, returning
/// the transaction hash and the sender nonce it used.
pub(super) fn send_transaction(
    client: &EvmClient,
    network: SccpNetworkV1,
    key: &EvmSigningKey,
    to: Option<[u8; 20]>,
    data: Vec<u8>,
) -> Result<([u8; 32], u64)> {
    let sender = key.address();
    let nonce = client
        .transaction_count(&sender, BlockId::Tag(BlockTag::Pending))
        .map_err(|error| eyre!("eth_getTransactionCount: {error}"))?;
    let gas = match to {
        Some(to) => {
            let mut request = EvmCallRequest::new(to, data.clone());
            request.from = Some(sender);
            client
                .estimate_gas(&request)
                .map_err(|error| eyre!("eth_estimateGas (the call would revert): {error}"))?
        }
        // Creation of the ~15 KiB SCCP contract; unused gas is refunded.
        None => 6_000_000,
    };
    let gas_limit = gas.saturating_add(gas / 5);
    let priority = client
        .max_priority_fee_per_gas()
        .map_err(|error| eyre!("eth_maxPriorityFeePerGas: {error}"))?
        .to_u128()
        .ok_or_else(|| eyre!("priority fee overflows"))?;
    let base_fee = client
        .block_by_number(BlockTag::Latest, false)
        .map_err(|error| eyre!("latest block: {error}"))?
        .and_then(|block| block.header.base_fee_per_gas)
        .and_then(iroha_sccp_rpc::evm::U256::to_u128)
        .ok_or_else(|| eyre!("the latest block has no base fee"))?;
    let transaction = Eip1559TransactionV1 {
        chain_id: eip1559_chain_id(network).map_err(|error| eyre!("{error:?}"))?,
        nonce,
        max_priority_fee_per_gas: priority,
        max_fee_per_gas: base_fee.saturating_mul(2).saturating_add(priority),
        gas_limit,
        to,
        value: 0,
        data,
    };
    transaction
        .validate()
        .map_err(|error| eyre!("transaction: {error:?}"))?;
    let signed = transaction
        .sign(key)
        .map_err(|error| eyre!("signing: {error:?}"))?;
    let hash = client
        .send_raw_transaction(&signed.raw())
        .map_err(|error| eyre!("eth_sendRawTransaction: {error}"))?;
    Ok((hash, nonce))
}

/// The address of the contract `sender` creates with `nonce`:
/// `keccak256(rlp([sender, nonce]))[12..]`.
#[must_use]
pub(super) fn created_address(sender: &[u8; 20], nonce: u64) -> [u8; 20] {
    let encoded = rlp_list(&[rlp_bytes(sender), rlp_uint(u128::from(nonce))]);
    let hash = iroha_sccp::v1::hashes::keccak256(&[&encoded]);
    let mut address = [0_u8; 20];
    address.copy_from_slice(&hash[12..]);
    address
}

/// The ABI-encoded constructor arguments of `SccpTairaXor` (§5.1.1).
#[must_use]
pub(super) fn constructor_args(
    taira_network_id: &[u8; 32],
    network: SccpNetworkV1,
    route_revision: u32,
    max_wrapped_supply: u128,
    roster: &iroha_sccp::v1::roster::RosterV1,
) -> Vec<u8> {
    use iroha_sccp::v1::{
        evm_abi::{AbiToken, encode_tokens, roster_token},
        hashes::{word_u64, word_u128},
    };
    encode_tokens(&[
        AbiToken::Word(*taira_network_id),
        AbiToken::Word(word_u64(u64::from(iroha_sccp::v1::network::tag(network)))),
        AbiToken::Word(word_u64(u64::from(route_revision))),
        AbiToken::Word(word_u128(max_wrapped_supply)),
        roster_token(roster),
    ])
}

/// The deployed runtime code hash of `contract`, as `RegisterRoute` records it.
pub(super) fn runtime_code_hash(client: &EvmClient, contract: &[u8; 20]) -> Result<[u8; 32]> {
    let code = client
        .code(contract, BlockId::Tag(BlockTag::Latest))
        .map_err(|error| eyre!("eth_getCode: {error}"))?;
    if code.is_empty() {
        return Err(eyre!("no code at the contract address yet"));
    }
    Ok(iroha_sccp::v1::hashes::keccak256(&[&code]))
}

/// Load the owner-only EVM key file at `path`.
pub(super) fn load_key(path: &Path) -> Result<EvmSigningKey> {
    EvmSigningKey::load(path)
        .map_err(|error| eyre!("EVM key file {}: {error:?}", path.display()))
        .wrap_err("external keys come only from owner-only key files")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn created_addresses_follow_the_rlp_rule() {
        // The first contract of 0x6ac7ea33f8831ea9dcc53393aaa88b25a785dbf0 (nonce 0) is the
        // well-known 0xcd234a471b72ba2f1ccf0a70fcaba648a5eecd8d.
        let sender: [u8; 20] = hex::decode("6ac7ea33f8831ea9dcc53393aaa88b25a785dbf0")
            .expect("hex")
            .try_into()
            .expect("20 bytes");
        assert_eq!(
            hex::encode(created_address(&sender, 0)),
            "cd234a471b72ba2f1ccf0a70fcaba648a5eecd8d"
        );
        assert_eq!(
            hex::encode(created_address(&sender, 1)),
            "343c43a37d37dff08ae8c4a11544c718abb4fcf8"
        );
    }

    #[test]
    fn constructor_arguments_start_with_the_static_words() {
        let roster = iroha_sccp::v1::roster::RosterV1 {
            generation: 2,
            valid_from_ms: 1,
            valid_until_ms: 2,
            members: vec![[1; 20], [2; 20], [3; 20], [4; 20]],
        };
        let encoded = constructor_args(&[7; 32], SccpNetworkV1::EthereumMainnet, 1, 1_000, &roster);
        assert_eq!(&encoded[..32], &[7; 32]);
        assert_eq!(encoded[63], 0x41);
        assert_eq!(encoded[95], 1);
        assert_eq!(
            u128::from_be_bytes(encoded[112..128].try_into().expect("16")),
            1_000
        );
        assert_eq!(encoded.len() % 32, 0);
    }
}
