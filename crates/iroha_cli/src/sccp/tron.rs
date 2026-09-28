//! TRON destination actions of `iroha sccp` (`specs/sccp.md` §5.2.5, §7.1, §7.4).
//!
//! The TRON deployment runs the shared Solidity contract, so bundles, rotation chains and
//! calldata are the EVM ones; only transport and transactions differ. Every bundle and rotation
//! chain read from Taira is verified locally against the deployment's own `rosterState()`
//! before a `TriggerSmartContract` is built and signed with the owner-only key file (TRON keys
//! are secp256k1 keys, so the EVM key file format serves both).

use eyre::{Result, eyre};
use iroha_sccp::{
    api::{SccpMessageProofBundleV1, SccpRotationChainV1},
    v1::{
        evm_abi::{ViewCallV1, decode_roster_state_return},
        proof::DestinationV1,
        roster::RosterStateV1,
    },
};
use iroha_sccp_rpc::{
    EndpointSet, FailoverPolicy, HttpConfig, HttpTransport, TronClient, tron::TronBlock,
};
use iroha_sccp_wallet::pure::{
    bundle::{BundlePurposeV1, DestinationContextV1, verify_message_bundle},
    evm::{EvmSigningKey, finalize_calldata, rotate_rosters_calldata},
    rotation::verify_rotation_chain,
    tron::{
        DEFAULT_CALL_FEE_LIMIT_SUN, DEFAULT_CREATE_FEE_LIMIT_SUN, TronCallV1, TronCreateV1,
        TronRawTransactionV1, TronReferenceBlockV1, created_contract_address, tron_address,
    },
};

use super::wall_clock_ms;

/// Connect to a TRON HTTP API endpoint.
pub(super) fn connect_api(api_url: &str) -> Result<TronClient> {
    let endpoints =
        EndpointSet::parse(&[api_url], &[]).map_err(|error| eyre!("TRON endpoint: {error}"))?;
    let transport = HttpTransport::new(endpoints, HttpConfig::default(), FailoverPolicy::default())
        .map_err(|error| eyre!("TRON transport: {error}"))?;
    Ok(TronClient::new(transport))
}

fn reference(block: &TronBlock) -> TronReferenceBlockV1 {
    TronReferenceBlockV1 {
        number: block.header.number,
        block_id: block.block_id,
        timestamp_ms: block.header.timestamp,
    }
}

fn head(client: &TronClient) -> Result<TronBlock> {
    client
        .now_block()
        .map_err(|error| eyre!("getnowblock: {error}"))
}

/// Sign and broadcast `raw`, returning its transaction id.
fn broadcast(
    client: &TronClient,
    raw: &TronRawTransactionV1,
    key: &EvmSigningKey,
) -> Result<[u8; 32]> {
    let signed = raw.sign(key).map_err(|error| eyre!("signing: {error:?}"))?;
    let outcome = client
        .broadcast_hex(&signed)
        .map_err(|error| eyre!("broadcasthex: {error}"))?;
    if !outcome.result {
        return Err(eyre!(
            "the TRON node refused the transaction: {} {}",
            outcome.code.unwrap_or_default(),
            outcome.message.unwrap_or_default()
        ));
    }
    Ok(raw.id())
}

/// One TRON deployment reached through a java-tron HTTP API endpoint.
pub(super) struct TronDestination {
    client: TronClient,
    contract: [u8; 21],
    destination: DestinationV1,
}

impl TronDestination {
    /// Connect to `api_url`.
    pub(super) fn connect(
        api_url: &str,
        contract: [u8; 21],
        destination: DestinationV1,
    ) -> Result<Self> {
        Ok(Self {
            client: connect_api(api_url)?,
            contract,
            destination,
        })
    }

    /// Read the deployment's `rosterState()` at the head.
    pub(super) fn roster_state(&self) -> Result<RosterStateV1> {
        let call = self
            .client
            .trigger_constant_contract(
                &self.contract,
                &self.contract,
                &ViewCallV1::RosterState.calldata(),
            )
            .map_err(|error| eyre!("rosterState(): {error}"))?;
        if call.reverted() {
            return Err(eyre!("rosterState() reverted"));
        }
        let data = call
            .constant_result
            .first()
            .ok_or_else(|| eyre!("rosterState() returned nothing"))?;
        decode_roster_state_return(data).map_err(|error| eyre!("rosterState() return: {error:?}"))
    }

    fn context(&self, taira_network_id: [u8; 32]) -> Result<DestinationContextV1> {
        Ok(DestinationContextV1 {
            taira_network_id,
            destination: self.destination,
            roster_state: self.roster_state()?,
            now_ms: head(&self.client)?.header.timestamp,
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
            head(&self.client)?.header.timestamp,
        )
        .map_err(|error| eyre!("the rotation chain does not verify: {error}"))?;
        Ok(rotate_rosters_calldata(&plan))
    }

    /// Sign `data` as a `TriggerSmartContract` of the deployment with `key` and broadcast it,
    /// returning the transaction id.
    pub(super) fn send(&self, key: &EvmSigningKey, data: Vec<u8>) -> Result<[u8; 32]> {
        let owner = tron_address(&key.address());
        let simulated = self
            .client
            .trigger_constant_contract(&owner, &self.contract, &data)
            .map_err(|error| eyre!("triggerconstantcontract: {error}"))?;
        if simulated.reverted() {
            return Err(eyre!("the call would revert on the TRON deployment"));
        }
        let raw = TronRawTransactionV1::trigger(
            &TronCallV1 {
                owner,
                contract: self.contract,
                data,
                fee_limit_sun: DEFAULT_CALL_FEE_LIMIT_SUN,
            },
            &reference(&head(&self.client)?),
            wall_clock_ms(),
        );
        broadcast(&self.client, &raw, key)
    }
}

/// Deploy `bytecode` (creation code with constructor arguments) from `key` and return the
/// transaction id and the created contract address.
pub(super) fn deploy(
    client: &TronClient,
    key: &EvmSigningKey,
    bytecode: Vec<u8>,
) -> Result<([u8; 32], [u8; 21])> {
    let owner = tron_address(&key.address());
    let raw = TronRawTransactionV1::create(
        &TronCreateV1 {
            owner,
            bytecode,
            name: "SccpTairaXor".into(),
            fee_limit_sun: DEFAULT_CREATE_FEE_LIMIT_SUN,
            origin_energy_limit: 10_000_000,
            consume_user_resource_percent: 100,
        },
        &reference(&head(client)?),
        wall_clock_ms(),
    );
    let address = created_contract_address(&raw.id(), &owner);
    Ok((broadcast(client, &raw, key)?, address))
}

/// The deployed runtime code hash of `contract` (`getcontractinfo` `runtimecode`), as
/// `RegisterRoute` records it.
pub(super) fn runtime_code_hash(client: &TronClient, contract: &[u8; 21]) -> Result<[u8; 32]> {
    let info = client
        .contract_info(contract)
        .map_err(|error| eyre!("getcontractinfo: {error}"))?
        .ok_or_else(|| eyre!("no contract at the address yet"))?;
    if info.runtime_code.is_empty() {
        return Err(eyre!("no code at the contract address yet"));
    }
    Ok(iroha_sccp::v1::hashes::keccak256(&[&info.runtime_code]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_blocks_become_transaction_references() {
        let block = TronBlock {
            block_id: [7; 32],
            header: iroha_sccp_rpc::tron::TronBlockHeader {
                number: 42,
                timestamp: 1_700_000_000_000,
                tx_trie_root: [0; 32],
                parent_hash: [0; 32],
                witness_address: [0x41; 21],
                witness_id: 0,
                version: 0,
                account_state_root: Vec::new(),
                witness_signature: Vec::new(),
            },
            transactions: Vec::new(),
        };
        let reference = reference(&block);
        assert_eq!(reference.number, 42);
        assert_eq!(reference.block_id, [7; 32]);
        assert_eq!(reference.timestamp_ms, 1_700_000_000_000);
        assert!(wall_clock_ms() > 1_700_000_000_000);
        assert!(connect_api("not a url").is_err());
    }
}
