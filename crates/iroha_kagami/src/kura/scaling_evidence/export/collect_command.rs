//! Offline native collection with bounded exact reply and original source custody.

use super::{
    collector::CollectionLimits,
    filesystem::{CollectedOutputPair, NativeCollectionIdentity, ProofInputBinding, collect_bound},
};
use crate::{
    Outcome, RunArgs,
    kura::scaling_evidence::command::{ReaderArgs, parse_sha256},
};
use clap::Args as ClapArgs;
use color_eyre::eyre::{Result, ensure};
use iroha_crypto::Hash;
use iroha_data_model::NetworkId;
use iroha_model_base::chain::ChainId;
use std::{
    io::{BufWriter, Write},
    path::PathBuf,
};

const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_REPLY_BYTES: usize = 4096;

/// Collect native carrier/context and actual query vectors from the stopped original store.
#[derive(Clone, Debug, ClapArgs)]
pub(crate) struct Args {
    /// Independently selected lowercase SHA-256 invocation identity
    #[arg(long, value_parser = parse_sha256)]
    invocation_id: [u8; 32],
    /// Independent original chain identity
    #[arg(long)]
    chain_id: ChainId,
    /// Independent original genesis network identity
    #[arg(long)]
    network_id: NetworkId,
    /// Independent native epoch context identity from the original signed genesis
    #[arg(long, value_parser = parse_epoch_hash)]
    genesis_epoch_context_id: Hash,
    /// Absolute original canonical signed genesis path
    #[arg(long)]
    signed_genesis: PathBuf,
    /// Raw SHA-256 of the independently retained original genesis
    #[arg(long, value_parser = parse_sha256)]
    signed_genesis_sha256: [u8; 32],
    /// Maximum original genesis bytes
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=33554432))]
    signed_genesis_max_bytes: u64,
    /// Absolute original canonical genesis epoch context path
    #[arg(long)]
    context: PathBuf,
    /// Raw SHA-256 of the independently retained original epoch context
    #[arg(long, value_parser = parse_sha256)]
    context_sha256: [u8; 32],
    /// Maximum original epoch context and each archived complete context projection
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=8388608))]
    context_max_bytes: u64,
    /// Exact stopped canonical Kura store root
    #[arg(long)]
    block_store: PathBuf,
    /// Exact stopped canonical merge log path
    #[arg(long)]
    merge_log: PathBuf,
    /// New complete carrier/context vector destination
    #[arg(long)]
    carrier_out: PathBuf,
    /// New complete actual query vector destination
    #[arg(long)]
    queries_out: PathBuf,
    /// Maximum complete carrier/context vector bytes
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    carrier_max_bytes: u64,
    /// Maximum complete query vector bytes
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    queries_max_bytes: u64,
    /// Aggregate original inputs and output byte reservations
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_BYTES))]
    total_max_bytes: u64,
    /// Complete reply bytes including its final newline
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=MAX_REPLY_BYTES as u64))]
    reply_max_bytes: u64,
    /// Maximum complete Network query count across all native Decision carriers
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=1000000))]
    max_total_leaves: u64,
    /// Maximum complete Network input and typed output count per carrier
    #[arg(long, value_parser = clap::value_parser!(u64).range(1..=1000000))]
    max_leaves_per_carrier: u64,
    /// Explicit complete stopped-store admission; first height is one and last is at least two
    #[command(flatten)]
    reader: ReaderArgs,
}
impl<W: Write> RunArgs<W> for Args {
    fn run(self, writer: &mut BufWriter<W>) -> Outcome {
        let limits = CollectionLimits {
            carrier_bytes: self.carrier_max_bytes,
            query_bytes: self.queries_max_bytes,
            total_bytes: self.total_max_bytes,
            context_bytes: usize::try_from(self.context_max_bytes)?,
            queries: usize::try_from(self.max_total_leaves)?,
            leaves_per_carrier: usize::try_from(self.max_leaves_per_carrier)?,
        };
        let outputs = CollectedOutputPair::admit(&self.carrier_out, &self.queries_out, limits)?;
        let collected = collect_bound(
            ProofInputBinding {
                path: self.signed_genesis,
                sha256: self.signed_genesis_sha256,
                max_bytes: self.signed_genesis_max_bytes,
            },
            ProofInputBinding {
                path: self.context,
                sha256: self.context_sha256,
                max_bytes: self.context_max_bytes,
            },
            self.chain_id,
            self.network_id,
            self.genesis_epoch_context_id.into(),
            &self.block_store,
            &self.merge_log,
            self.reader.into_limits()?,
            limits,
            outputs,
        )?;
        collected.finish_reply(|identity| {
            let reply = CollectionReply::new(self.invocation_id, self.reply_max_bytes, identity)?;
            writer.write_all(&reply.bytes[..reply.len])?;
            writer.flush()?;
            Ok(())
        })?;
        Ok(())
    }
}
fn parse_epoch_hash(value: &str) -> std::result::Result<Hash, String> {
    if value.len() != 74 {
        return Err("expected one canonical checked epoch hash literal".into());
    }
    norito::json::from_value(norito::json::Value::String(value.to_owned()))
        .map_err(|_| "expected one canonical checked epoch hash literal".into())
}

struct CollectionReply {
    bytes: [u8; MAX_REPLY_BYTES],
    len: usize,
}
impl CollectionReply {
    fn new(invocation: [u8; 32], maximum: u64, value: NativeCollectionIdentity) -> Result<Self> {
        ensure!(
            (1..=MAX_REPLY_BYTES as u64).contains(&maximum)
                && value.committed_height >= 2
                && value.committed_height == value.carrier_count
                && value.carrier_count <= 1_000_000
                && value.query_count <= 1_000_000,
            "invalid native collection reply admission"
        );
        let identities = [value.genesis, value.context, value.carrier, value.queries];
        ensure!(
            identities
                .iter()
                .all(|identity| (1..=MAX_BYTES).contains(&identity.byte_length)),
            "invalid native collection reply byte identity"
        );
        let mut hexes = [[0u8; 64]; 5];
        for (target, digest) in hexes.iter_mut().zip([
            invocation,
            value.genesis.raw_sha256,
            value.context.raw_sha256,
            value.carrier.raw_sha256,
            value.queries.raw_sha256,
        ]) {
            hex::encode_to_slice(digest, target)?;
        }
        let mut bytes = [0u8; MAX_REPLY_BYTES];
        let mut remaining = &mut bytes[..];
        writeln!(
            remaining,
            "{{\"version\":1,\"operation\":\"collect_native_inputs\",\"invocation_id\":\"{}\",\"genesis_sha256\":\"{}\",\"genesis_bytes\":{},\"committed_height\":{},\"carrier_count\":{},\"query_count\":{},\"context_sha256\":\"{}\",\"context_bytes\":{},\"carrier_sha256\":\"{}\",\"carrier_bytes\":{},\"queries_sha256\":\"{}\",\"queries_bytes\":{}}}",
            std::str::from_utf8(&hexes[0])?,
            std::str::from_utf8(&hexes[1])?,
            value.genesis.byte_length,
            value.committed_height,
            value.carrier_count,
            value.query_count,
            std::str::from_utf8(&hexes[2])?,
            value.context.byte_length,
            std::str::from_utf8(&hexes[3])?,
            value.carrier.byte_length,
            std::str::from_utf8(&hexes[4])?,
            value.queries.byte_length
        )?;
        let len = MAX_REPLY_BYTES - remaining.len();
        ensure!(
            len as u64 <= maximum,
            "native collection reply exceeds reservation"
        );
        Ok(Self { bytes, len })
    }
}

#[cfg(test)]
mod tests {
    use super::super::filesystem::PreparedTransportIdentity;
    use super::*;
    #[test]
    fn collection_reply_has_exact_complete_fields_and_enforces_actual_size() {
        let file = PreparedTransportIdentity {
            raw_sha256: [8; 32],
            byte_length: 19,
        };
        let identity = NativeCollectionIdentity {
            genesis: file,
            context: file,
            carrier: file,
            queries: file,
            committed_height: 3,
            carrier_count: 3,
            query_count: 8,
        };
        let reply = CollectionReply::new([3; 32], 4096, identity).unwrap();
        let value: norito::json::Value =
            norito::json::from_slice(&reply.bytes[..reply.len]).unwrap();
        assert_eq!(value.as_object().unwrap().len(), 14);
        assert_eq!(value["operation"].as_str(), Some("collect_native_inputs"));
        assert_eq!(value["query_count"].as_u64(), Some(8));
        assert_eq!(reply.bytes[reply.len - 1], b'\n');
        assert!(CollectionReply::new([3; 32], reply.len as u64, identity).is_ok());
        assert!(CollectionReply::new([3; 32], reply.len as u64 - 1, identity).is_err());
        for maximum in [0, 4097] {
            assert!(CollectionReply::new([3; 32], maximum, identity).is_err());
        }
        let mut invalid = identity;
        invalid.carrier_count -= 1;
        assert!(CollectionReply::new([3; 32], 4096, invalid).is_err());
    }
    #[derive(clap::Parser)]
    struct Parse {
        #[command(flatten)]
        args: Args,
    }
    #[test]
    fn native_collect_parser_requires_every_independent_flag_and_rejects_retired_inputs() {
        use clap::Parser as _;
        let hash = Hash::new(b"native collector parser genesis");
        let literal = norito::json::to_value(&hash)
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let mut args = vec!["collect".to_owned()];
        for (flag, value) in [
            ("invocation-id", "03".repeat(32)),
            ("chain-id", "native-collection-test".into()),
            ("network-id", literal.clone()),
            ("genesis-epoch-context-id", literal),
            ("signed-genesis", "/original/genesis.nrt".into()),
            ("signed-genesis-sha256", "04".repeat(32)),
            ("signed-genesis-max-bytes", "65536".into()),
            ("context", "/original/context.nrt".into()),
            ("context-sha256", "05".repeat(32)),
            ("context-max-bytes", "65536".into()),
            ("block-store", "/original/kura".into()),
            ("merge-log", "/original/kura/merge.log".into()),
            ("carrier-out", "/output/carrier.nrt".into()),
            ("queries-out", "/output/queries.nrt".into()),
            ("carrier-max-bytes", "65536".into()),
            ("queries-max-bytes", "65536".into()),
            ("total-max-bytes", "1048576".into()),
            ("reply-max-bytes", "4096".into()),
            ("max-total-leaves", "100".into()),
            ("max-leaves-per-carrier", "20".into()),
            ("first-height", "1".into()),
            ("last-height", "100".into()),
            ("max-committed-blocks", "100".into()),
            ("max-store-data-bytes", "8388608".into()),
            ("max-carrier-bytes", "65536".into()),
            ("max-merge-log-bytes", "65536".into()),
            ("max-merge-frames", "100".into()),
            ("reader-max-output-bytes", "65536".into()),
            ("max-decode-allocation-bytes", "65536".into()),
            ("owner-uid", "501".into()),
        ] {
            args.extend([format!("--{flag}"), value]);
        }
        assert_eq!(args.len(), 61);
        Parse::try_parse_from(args.clone()).unwrap();
        let mut public = vec![
            "kagami".into(),
            "advanced".into(),
            "kura".into(),
            "scaling-evidence".into(),
        ];
        public.extend(args.clone());
        crate::Cli::try_parse_from(public).unwrap();
        for index in (1..args.len()).step_by(2) {
            let mut missing = args.clone();
            missing.drain(index..index + 2);
            assert!(Parse::try_parse_from(missing).is_err(), "{}", args[index]);
        }
        for flag in ["--client-config-sha256", "--finality-out", "--config-fd"] {
            let mut retired = args.clone();
            retired.extend([flag.into(), "1".into()]);
            assert!(Parse::try_parse_from(retired).is_err());
        }
        assert!(parse_epoch_hash(&hash.to_string()).is_err());
    }
}
