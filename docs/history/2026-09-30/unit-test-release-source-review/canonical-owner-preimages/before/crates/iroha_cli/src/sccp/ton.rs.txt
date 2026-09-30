//! TON destination actions of `iroha sccp` (`specs/sccp.md` §5.3, §7.1, §7.4).
//!
//! The minter's state is read with get methods over ADNL liteservers, every bundle and rotation
//! chain is verified locally against it, and the resulting minter messages are sent as
//! internal messages from the user's deployed `v5r1` wallet, signed with an owner-only Ed25519
//! key file.

use eyre::{Result, eyre};
use iroha_sccp::{
    api::{SccpMessageProofBundleV1, SccpRotationChainV1},
    v1::{proof::DestinationV1, roster::RosterStateV1},
};
use iroha_sccp_rpc::{
    FailoverPolicy,
    ton::{AccountId, BlockIdExt, LiteClient, LiteClientConfig, LiteServerSet},
};
use iroha_sccp_wallet::pure::{
    bundle::{BundlePurposeV1, DestinationContextV1, verify_message_bundle},
    rotation::verify_rotation_chain,
    ton::{
        SEND_MODE_PAY_FEES_SEPARATELY_IGNORE_ERRORS, TonAccountV1, TonWalletKeyV1, TvmIntV1,
        WALLET_V5R1_MAINNET_ID, finalize_body, get_method_id, internal_message, parse_int_stack,
        rotate_body, stack_of_ints, wallet_v5_transfer,
    },
};

/// Value sent with a rotation (excess returns to the wallet), in nanotons.
const ROTATE_VALUE: u128 = 100_000_000;
/// Lifetime of a wallet external message (seconds).
const EXTERNAL_LIFETIME_S: u32 = 120;

/// Connect to liteservers: a comma-separated list of `<ip>:<port>:<base64 key>` entries, or
/// the compiled public list when empty.
pub(super) fn connect(liteservers: &str) -> Result<LiteClient> {
    let servers = if liteservers.trim().is_empty() {
        LiteServerSet::compiled_defaults()
    } else {
        let entries: Vec<&str> = liteservers.split(',').map(str::trim).collect();
        LiteServerSet::parse(&entries).map_err(|error| eyre!("liteservers: {error}"))?
    };
    Ok(LiteClient::new(
        servers,
        LiteClientConfig::default(),
        FailoverPolicy::default(),
    ))
}

/// Parse a raw workchain-0 address `0:<64 hex>`.
pub(super) fn parse_account(text: &str) -> Result<TonAccountV1> {
    let hex_part = text
        .strip_prefix("0:")
        .ok_or_else(|| eyre!("a TON account is a raw `0:<64 hex>` address"))?;
    let bytes = hex::decode(hex_part).map_err(|_| eyre!("TON account id is not hex"))?;
    <[u8; 32]>::try_from(bytes).map_err(|_| eyre!("TON account id must be 32 bytes"))
}

fn last_block(lite: &LiteClient) -> Result<BlockIdExt> {
    Ok(lite
        .get_masterchain_info()
        .map_err(|error| eyre!("getMasterchainInfo: {error}"))?
        .last)
}

/// Run get method `method` of `account` with integer `args` at the last block.
fn run_method(
    lite: &LiteClient,
    account: &TonAccountV1,
    method: &str,
    args: &[i64],
) -> Result<Vec<Option<TvmIntV1>>> {
    let result = lite
        .run_smc_method(
            4,
            last_block(lite)?,
            AccountId {
                workchain: 0,
                address: *account,
            },
            get_method_id(method),
            stack_of_ints(args).map_err(|error| eyre!("{method} arguments: {error}"))?,
        )
        .map_err(|error| eyre!("runSmcMethod {method}: {error}"))?;
    if result.exit_code != 0 && result.exit_code != 1 {
        return Err(eyre!("{method} exited with code {}", result.exit_code));
    }
    let stack = result
        .result
        .ok_or_else(|| eyre!("{method} returned no stack"))?;
    parse_int_stack(&stack).map_err(|error| eyre!("{method} result: {error}"))
}

fn int_at(values: &[Option<TvmIntV1>], index: usize, what: &str) -> Result<TvmIntV1> {
    values
        .get(index)
        .copied()
        .flatten()
        .ok_or_else(|| eyre!("the minter state has no integer `{what}`"))
}

/// The user's deployed `v5r1` wallet and its key.
pub(super) struct TonWallet {
    address: TonAccountV1,
    key: TonWalletKeyV1,
}

impl TonWallet {
    /// Load `key_file` for the wallet at `address` and check the wallet's public key.
    pub(super) fn load(
        lite: &LiteClient,
        address: &str,
        key_file: &std::path::Path,
    ) -> Result<Self> {
        let address = parse_account(address)?;
        let key = TonWalletKeyV1::load(key_file).map_err(|error| eyre!("TON key file: {error}"))?;
        let public = run_method(lite, &address, "get_public_key", &[])?;
        let public = int_at(&public, 0, "public key")?
            .to_u256()
            .ok_or_else(|| eyre!("the wallet public key is not a 256-bit value"))?;
        if public != key.public_key() {
            return Err(eyre!(
                "the key file does not control wallet 0:{}",
                hex::encode(address)
            ));
        }
        Ok(Self { address, key })
    }

    /// The wallet account.
    pub(super) const fn address(&self) -> &TonAccountV1 {
        &self.address
    }

    /// Send `messages` (internal messages with send modes) and return the external message
    /// `BoC` hash.
    pub(super) fn send(
        &self,
        lite: &LiteClient,
        messages: &[(iroha_sccp::v1::ton_cell::Cell, u8)],
    ) -> Result<[u8; 32]> {
        let seqno = run_method(lite, &self.address, "seqno", &[])?;
        let seqno = int_at(&seqno, 0, "seqno")?
            .to_u64()
            .and_then(|seqno| u32::try_from(seqno).ok())
            .ok_or_else(|| eyre!("the wallet seqno is not a u32"))?;
        let now = lite
            .get_time()
            .map_err(|error| eyre!("getTime: {error}"))?
            .now;
        let boc = wallet_v5_transfer(
            &self.address,
            WALLET_V5R1_MAINNET_ID,
            seqno,
            now.saturating_add(EXTERNAL_LIFETIME_S),
            messages,
            &self.key,
        )
        .map_err(|error| eyre!("wallet message: {error}"))?;
        let status = lite
            .send_message(boc.clone())
            .map_err(|error| eyre!("sendMessage: {error}"))?;
        if status.status != 1 {
            return Err(eyre!(
                "the liteserver refused the message ({})",
                status.status
            ));
        }
        Ok(iroha_sccp::ton_boc_single_root_hash_v1(&boc).unwrap_or_default())
    }
}

/// One TON deployment (the minter) reached through liteservers.
pub(super) struct TonDestination {
    lite: LiteClient,
    minter: TonAccountV1,
    destination: DestinationV1,
}

impl TonDestination {
    /// Connect to `liteservers` (see [`connect`]).
    pub(super) fn connect(
        liteservers: &str,
        minter: TonAccountV1,
        destination: DestinationV1,
    ) -> Result<Self> {
        Ok(Self {
            lite: connect(liteservers)?,
            minter,
            destination,
        })
    }

    /// The liteserver client.
    pub(super) const fn lite(&self) -> &LiteClient {
        &self.lite
    }

    /// The minter's roster state from `get_sccp_state` (§5.3.6).
    pub(super) fn roster_state(&self) -> Result<RosterStateV1> {
        let values = run_method(&self.lite, &self.minter, "get_sccp_state", &[])?;
        let u256 = |index, what| {
            int_at(&values, index, what)?
                .to_u256()
                .ok_or_else(|| eyre!("`{what}` is not a 256-bit value"))
        };
        let u64_at = |index, what| {
            int_at(&values, index, what)?
                .to_u64()
                .ok_or_else(|| eyre!("`{what}` is not a u64"))
        };
        Ok(RosterStateV1 {
            digest: u256(5, "digest")?,
            generation: u64_at(6, "generation")?,
            valid_until_ms: u64_at(7, "validUntilMs")?,
            prev_digest: u256(8, "prevDigest")?,
            prev_valid_until_ms: u64_at(9, "prevValidUntilMs")?,
        })
    }

    /// Destination time in milliseconds (the liteserver clock).
    pub(super) fn now_ms(&self) -> Result<u64> {
        Ok(u64::from(
            self.lite
                .get_time()
                .map_err(|error| eyre!("getTime: {error}"))?
                .now,
        ) * 1_000)
    }

    /// Verify `bundle` for this deployment and send `sccp_finalize` from `wallet` with the
    /// minter's `finalize_required_value`.
    pub(super) fn finalize(
        &self,
        bundle: &SccpMessageProofBundleV1,
        taira_network_id: [u8; 32],
        wallet: &TonWallet,
    ) -> Result<[u8; 32]> {
        let context = DestinationContextV1 {
            taira_network_id,
            destination: self.destination,
            roster_state: self.roster_state()?,
            now_ms: self.now_ms()?,
        };
        let verified = verify_message_bundle(bundle, &context, BundlePurposeV1::Finalize).map_err(
            |error| eyre!("the proof bundle does not verify for this deployment: {error}"),
        )?;
        let nonce = i64::try_from(verified.nonce()).map_err(|_| eyre!("nonce overflows"))?;
        let value = run_method(
            &self.lite,
            &self.minter,
            "finalize_required_value",
            &[nonce],
        )?;
        let value = int_at(&value, 0, "required value")?
            .to_u64()
            .ok_or_else(|| eyre!("the required value is not a u64"))?;
        let body = finalize_body(&verified, verified.nonce(), wallet.address())
            .map_err(|error| eyre!("finalize message: {error}"))?;
        let message = internal_message(&self.minter, u128::from(value), true, body, None)
            .map_err(|error| eyre!("internal message: {error}"))?;
        wallet.send(
            &self.lite,
            &[(message, SEND_MODE_PAY_FEES_SEPARATELY_IGNORE_ERRORS)],
        )
    }

    /// Verify `chain` from the minter's roster to `target_generation` and send one
    /// `sccp_rotate` per rotation from `wallet`. Returns the sent message hashes.
    pub(super) fn rotate(
        &self,
        chain: &SccpRotationChainV1,
        target_generation: u64,
        taira_network_id: [u8; 32],
        wallet: &TonWallet,
    ) -> Result<Vec<[u8; 32]>> {
        let plan = verify_rotation_chain(
            chain,
            &self.roster_state()?,
            target_generation,
            &taira_network_id,
            self.now_ms()?,
        )
        .map_err(|error| eyre!("the rotation chain does not verify: {error}"))?;
        plan.rotations
            .iter()
            .map(|rotation| {
                let body = rotate_body(rotation, rotation.attestation.height, wallet.address())
                    .map_err(|error| eyre!("rotate message: {error}"))?;
                let message = internal_message(&self.minter, ROTATE_VALUE, true, body, None)
                    .map_err(|error| eyre!("internal message: {error}"))?;
                wallet.send(
                    &self.lite,
                    &[(message, SEND_MODE_PAY_FEES_SEPARATELY_IGNORE_ERRORS)],
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_accounts_parse() {
        assert_eq!(
            parse_account(&format!("0:{}", "ab".repeat(32))).expect("parses"),
            [0xab; 32]
        );
        assert!(parse_account("-1:00").is_err());
        assert!(parse_account("0:zz").is_err());
        assert!(connect("not-a-server").is_err());
    }
}
