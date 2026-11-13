//! Bounded operator-authenticated HTTP delivery followed by ordinary copied strict replay.

use super::*;
use iroha_allocation::{AllocatorRefusal, ChargedBuffer, ChargedBufferError};
use iroha_config::parameters::actual::ToriiRuntimeCheckpoint;
use iroha_core::{
    execution_attempt::{ExecutionAttemptError, json_decode_attempt_error},
    snapshot::transfer::{
        CheckpointTransferV1,
        transport::{
            self, ArtifactV1, Attempt, CheckpointTransport, PackageDescriptorV1, TransferLimits,
        },
    },
};
use std::{io::Read as _, time::Duration};

const PACKAGE: &str = "/v1/runtime/checkpoint/package";
const ROUTE_ROOT: &str = "/v1/runtime/checkpoint";
const METADATA_BYTES: usize = 16 * 1024;
const REQUEST_BYTES: usize = 1024;
const TOKEN_BYTES: usize = 4096;

#[derive(norito::json::JsonSerialize)]
struct ArtifactRequest {
    inventory_sha256: [u8; 32],
    index: u64,
}
#[derive(norito::json::JsonSerialize)]
struct RangeRequest {
    inventory_sha256: [u8; 32],
    index: u64,
    offset: u64,
    length: u32,
}
#[derive(norito::json::JsonSerialize)]
struct FinishRequest {
    inventory_sha256: [u8; 32],
}

fn invalid(message: &'static str) -> ExecutionAttemptError<transport::Error> {
    ExecutionAttemptError::Rejected(transport::Error::Invalid(message))
}
fn io(error: std::io::Error) -> ExecutionAttemptError<transport::Error> {
    ExecutionAttemptError::Rejected(transport::Error::Io(error))
}
fn http(error: reqwest::Error) -> ExecutionAttemptError<transport::Error> {
    io(std::io::Error::other(error))
}

fn read_api_token(path: &Path) -> Attempt<reqwest::header::HeaderValue> {
    let bytes =
        crate::runtime_credential::load_bounded_runtime_credential_v1(path, 1, TOKEN_BYTES + 1)
            .map_err(|_| {
                invalid("checkpoint API token requires a bounded owner-private credential file")
            })?;
    let token = bytes.strip_suffix(b"\n").unwrap_or(&bytes);
    if token.is_empty()
        || token.len() > TOKEN_BYTES
        || !token.iter().all(|byte| (33..=126).contains(byte))
    {
        return Err(invalid(
            "checkpoint API token is not a bounded single visible-ASCII value",
        ));
    }
    let mut header = reqwest::header::HeaderValue::from_bytes(token)
        .map_err(|_| invalid("checkpoint API token cannot form a request header"))?;
    header.set_sensitive(true);
    Ok(header)
}

/// One original request deadline and independently pinned network, never response-selected trust.
struct HttpCheckpointTransport {
    client: reqwest::blocking::Client,
    origin: reqwest::Url,
    signer: KeyPair,
    network: NetworkId,
    api_token: Option<reqwest::header::HeaderValue>,
    request_timeout: Duration,
    deadline: Instant,
    metadata: ChargedBuffer<u8>,
    pinned: Option<PackageDescriptorV1>,
}

impl HttpCheckpointTransport {
    fn new(
        endpoint: &str,
        signer: &KeyPair,
        network: NetworkId,
        api_token: Option<reqwest::header::HeaderValue>,
        policy: &ToriiRuntimeCheckpoint,
        budget: &AllocationBudget,
    ) -> Attempt<Self> {
        let mut origin = reqwest::Url::parse(endpoint)
            .map_err(|error| io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error)))?;
        if !matches!(origin.scheme(), "http" | "https")
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.query().is_some()
            || origin.fragment().is_some()
            || !matches!(origin.path(), "" | "/" | PACKAGE)
        {
            return Err(invalid(
                "checkpoint URL must name an HTTP(S) origin or exact package route",
            ));
        }
        if policy.request_timeout.is_zero() || policy.transfer_timeout < policy.request_timeout {
            return Err(invalid(
                "checkpoint request and transfer deadlines are invalid",
            ));
        }
        let deadline = Instant::now()
            .checked_add(policy.transfer_timeout)
            .ok_or_else(|| invalid("checkpoint transfer deadline is unrepresentable"))?;
        origin.set_path("/");
        let mut metadata =
            ChargedBuffer::new(METADATA_BYTES, budget).map_err(|error| match error {
                ChargedBufferError::Admission(original) => {
                    ExecutionAttemptError::Deferred(original.into())
                }
                original @ ChargedBufferError::Allocator { .. } => ExecutionAttemptError::Deferred(
                    AllocatorRefusal::try_from(original)
                        .expect("original metadata allocator refusal")
                        .into(),
                ),
            })?;
        for _ in 0..METADATA_BYTES {
            metadata.push_reserved(0);
        }
        let client = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .no_gzip()
            .no_brotli()
            .no_deflate()
            .no_zstd()
            .build()
            .map_err(http)?;
        Ok(Self {
            client,
            origin,
            signer: signer.clone(),
            network,
            api_token,
            request_timeout: policy.request_timeout,
            deadline,
            metadata,
            pinned: None,
        })
    }

    fn request(&self, suffix: &str, body: Option<Vec<u8>>) -> Attempt<reqwest::blocking::Response> {
        let remaining = self
            .deadline
            .checked_duration_since(Instant::now())
            .filter(|value| !value.is_zero())
            .ok_or_else(|| {
                io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "original checkpoint transfer deadline elapsed",
                ))
            })?;
        let mut url = self.origin.clone();
        if suffix.is_empty() {
            url.set_path(PACKAGE);
        } else {
            url.set_path(&format!("{ROUTE_ROOT}{suffix}"));
        }
        let method = if body.is_some() {
            reqwest::Method::POST
        } else {
            reqwest::Method::GET
        };
        let body = body.unwrap_or_default();
        if body.len() > REQUEST_BYTES {
            return Err(invalid("checkpoint request exceeds its fixed body bound"));
        }
        let uri: iroha_torii::Uri = url
            .path()
            .parse()
            .map_err(|error| io(std::io::Error::new(std::io::ErrorKind::InvalidInput, error)))?;
        let headers = iroha_torii::operator_signed_request_headers(
            &self.signer,
            &self.network,
            &method,
            &uri,
            &body,
        )
        .map_err(|error| io(std::io::Error::other(error)))?;
        let mut request = self
            .client
            .request(method, url)
            .timeout(self.request_timeout.min(remaining))
            .headers(headers)
            .header(
                reqwest::header::ACCEPT,
                if suffix == "/range" {
                    "application/octet-stream"
                } else {
                    "application/json"
                },
            )
            .header(reqwest::header::ACCEPT_ENCODING, "identity")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body);
        if let Some(token) = &self.api_token {
            request = request.header("x-api-token", token);
        }
        request.send().map_err(http)
    }

    fn post<T: norito::json::JsonSerialize>(
        &self,
        suffix: &str,
        body: &T,
    ) -> Attempt<reqwest::blocking::Response> {
        let body = norito::json::to_vec(body)
            .map_err(|error| json_decode_attempt_error(error, transport::Error::Json))?;
        self.request(suffix, Some(body))
    }

    fn check_response(
        response: &reqwest::blocking::Response,
        status: reqwest::StatusCode,
        content_type: Option<&str>,
    ) -> Attempt<()> {
        if response.status() != status {
            return Err(invalid(
                "checkpoint HTTP status differs from the requested operation",
            ));
        }
        if response
            .headers()
            .get(reqwest::header::CONTENT_ENCODING)
            .is_some_and(|value| value != "identity")
        {
            return Err(invalid(
                "checkpoint response must use its original identity encoding",
            ));
        }
        if let Some(expected) = content_type
            && response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(';').next())
                .map(str::trim)
                != Some(expected)
        {
            return Err(invalid("checkpoint response has another content type"));
        }
        Ok(())
    }

    fn json<T: norito::json::JsonDeserialize>(
        &mut self,
        mut response: reqwest::blocking::Response,
    ) -> Attempt<T> {
        Self::check_response(&response, reqwest::StatusCode::OK, Some("application/json"))?;
        if response
            .content_length()
            .is_some_and(|length| length > METADATA_BYTES as u64)
        {
            return Err(invalid(
                "checkpoint metadata response exceeds its fixed bound",
            ));
        }
        let mut len = 0;
        while len < METADATA_BYTES {
            let read = response
                .read(&mut self.metadata.as_mut_slice()[len..])
                .map_err(io)?;
            if read == 0 {
                break;
            }
            len += read;
        }
        let mut extra = [0];
        if response.read(&mut extra).map_err(io)? != 0 {
            return Err(invalid(
                "checkpoint metadata response exceeds its fixed bound",
            ));
        }
        norito::json::from_slice(&self.metadata.as_slice()[..len])
            .map_err(|error| json_decode_attempt_error(error, transport::Error::Json))
    }

    fn inventory(&self) -> Attempt<[u8; 32]> {
        self.pinned
            .as_ref()
            .map(|value| value.inventory_sha256)
            .ok_or_else(|| invalid("checkpoint descriptor must be pinned before artifact delivery"))
    }
}

impl CheckpointTransport for HttpCheckpointTransport {
    fn descriptor(&mut self) -> Attempt<PackageDescriptorV1> {
        let response = self.request("", None)?;
        let value: PackageDescriptorV1 = self.json(response)?;
        if self
            .pinned
            .as_ref()
            .is_some_and(|original| original != &value)
        {
            return Err(invalid("remote checkpoint inventory changed after pinning"));
        }
        self.pinned = Some(value.clone());
        Ok(value)
    }
    fn artifact(&mut self, index: u64) -> Attempt<ArtifactV1> {
        let response = self.post(
            "/artifact",
            &ArtifactRequest {
                inventory_sha256: self.inventory()?,
                index,
            },
        )?;
        self.json(response)
    }
    fn read_range(&mut self, index: u64, offset: u64, output: &mut [u8]) -> Attempt<()> {
        let max = self
            .pinned
            .as_ref()
            .map_or(0, |value| value.max_chunk_bytes as usize)
            .min(transport::MAX_CHECKPOINT_TRANSFER_CHUNK_BYTES);
        if output.is_empty() || output.len() > max {
            return Err(invalid(
                "checkpoint HTTP range exceeds the original chunk bound",
            ));
        }
        let mut response = self.post(
            "/range",
            &RangeRequest {
                inventory_sha256: self.inventory()?,
                index,
                offset,
                length: output.len() as u32,
            },
        )?;
        Self::check_response(
            &response,
            reqwest::StatusCode::OK,
            Some("application/octet-stream"),
        )?;
        if response
            .content_length()
            .is_some_and(|length| length != output.len() as u64)
        {
            return Err(invalid("checkpoint HTTP range has another extent"));
        }
        response.read_exact(output).map_err(io)?;
        let mut extra = [0];
        if response.read(&mut extra).map_err(io)? != 0 {
            return Err(invalid("checkpoint HTTP range exceeds its exact extent"));
        }
        Ok(())
    }
    fn finish(&mut self, expected: &PackageDescriptorV1) -> Attempt<()> {
        if self.pinned.as_ref() != Some(expected) {
            return Err(invalid("checkpoint finalization names another inventory"));
        }
        let mut response = self.post(
            "/finish",
            &FinishRequest {
                inventory_sha256: self.inventory()?,
            },
        )?;
        Self::check_response(&response, reqwest::StatusCode::NO_CONTENT, None)?;
        let mut extra = [0];
        if response.content_length().is_some_and(|length| length != 0)
            || response.read(&mut extra).map_err(io)? != 0
        {
            return Err(invalid(
                "checkpoint finalization must have no response body",
            ));
        }
        Ok(())
    }
}

/// Receive into a fresh private directory, then independently authenticate and replay its copy.
/// The transferred snapshot key authenticates file bytes only; configured genesis pins persist.
pub(crate) fn run_runtime_checkpoint_receive(
    config: &Config,
    endpoint: &str,
    api_token_file: Option<&Path>,
    output: &Path,
    config_path: Option<&Path>,
) -> ReportResult<(), MainError> {
    let output = sorafs_authority_inspect::output_destination(config, output, config_path)
        .map_err(failure)?;
    let policy = &config.torii.runtime_checkpoint;
    let budget = AllocationBudget::new(policy.source_memory_bytes.get());
    let network = NetworkId::from_genesis_hash(config.genesis.expected_hash);
    let api_token = api_token_file
        .map(read_api_token)
        .transpose()
        .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    let mut transport = HttpCheckpointTransport::new(
        endpoint,
        &config.common.key_pair,
        network,
        api_token,
        policy,
        &budget,
    )
    .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    let descriptor = transport
        .descriptor()
        .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    let manifest = transport::receive_checkpoint(
        &mut transport,
        &descriptor,
        &config.common.chain,
        network,
        TransferLimits {
            max_artifacts: policy.max_artifacts,
            max_total_bytes: policy.max_total_bytes,
            max_artifact_bytes: policy.max_artifact_bytes,
            chunk_bytes: policy.chunk_bytes,
        },
        &budget,
        &output,
    )
    .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
    let replay = authenticate_received(config, &output, &manifest)?;
    #[derive(norito::json::JsonSerialize)]
    struct Received {
        transfer: CheckpointTransferV1,
        replay: ReplayCheckReportV1,
    }
    println!(
        "{}",
        norito::json::to_json(&Received {
            transfer: manifest,
            replay
        })
        .map_err(|error| failure(error.to_string()))?
    );
    Ok(())
}

fn authenticate_received(
    config: &Config,
    output: &Path,
    manifest: &CheckpointTransferV1,
) -> ReportResult<ReplayCheckReportV1, MainError> {
    if manifest.version != 1
        || manifest.height < 2
        || manifest.chain_id != config.common.chain
        || manifest.network_id != NetworkId::from_genesis_hash(config.genesis.expected_hash)
    {
        return Err(failure(
            "received checkpoint differs from the independently pinned root",
        ));
    }
    let mut isolated = config.clone();
    isolated.kura.store_dir = WithOrigin::inline(output.join("kura"));
    isolated.kura.init_mode = InitMode::Strict;
    isolated.snapshot.store_dir = WithOrigin::inline(output.join("snapshots"));
    isolated.snapshot.mode = SnapshotMode::Readonly;
    isolated.snapshot.verification_public_key = Some(manifest.snapshot_verification_key.clone());
    isolated.snapshot.signing_private_key = None;
    let successor = manifest
        .height
        .checked_add(1)
        .ok_or_else(|| failure("checkpoint successor height overflows"))?;
    let replay = check_replay_with_matched(&isolated, |matched| {
        let view = matched.state.view();
        let index =
            usize::try_from(manifest.height - 1).map_err(|error| failure(error.to_string()))?;
        if view.block_hashes().get(index).copied() != Some(manifest.block_hash) {
            return Err(failure(
                "received manifest checkpoint hash differs from authenticated history",
            ));
        }
        let certified = iroha_core::sumeragi::certified_chain::CertifiedChain::new(&view)
            .and_then(|chain| chain.certified(successor))
            .map_err(|error| Report::new(error).change_context(MainError::CheckReplay))?;
        if certified
            .committed()
            .commitment()
            .execution
            .parent_world_state_root
            != manifest.published_state_root
        {
            return Err(failure(
                "received manifest State root differs from the authenticated successor",
            ));
        }
        Ok(())
    })?;
    if !replay.is_ok()
        || replay.start.source != START_SNAPSHOT
        || replay.start.height != manifest.height
        || replay.tip_height != successor
        || replay.replayed_to != successor
    {
        return Err(failure(
            "received checkpoint did not pass complete-State restoration and actual H+1 replay",
        ));
    }
    Ok(replay)
}

#[cfg(test)]
mod tests;
