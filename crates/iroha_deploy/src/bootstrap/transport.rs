//! Bounded native retrieval of original parent frames for private-root AMX bootstrap.

use std::time::Instant;

use iroha::http::{MAX_PUBLIC_READ_BYTES, PublicHttpClient, PublicHttpError};

use super::{AuthenticatedBootstrap, BootstrapError};

/// Failure to retrieve or authenticate native parent bootstrap material.
#[derive(Debug, thiserror::Error)]
pub enum CheckpointReadError {
    /// Credential-free bounded HTTP retrieval failed.
    #[error(transparent)]
    Http(#[from] PublicHttpError),
    /// Native proof validation or retained bootstrap custody failed.
    #[error(transparent)]
    Bootstrap(#[from] BootstrapError),
}

/// Reusable native HTTP context containing no ledger signer or response-selected authority.
#[derive(Clone, Debug, Default)]
pub struct CheckpointTransport {
    client: PublicHttpClient,
}

impl CheckpointTransport {
    /// Retain the native SDK's credential-free lazy HTTPS transport.
    /// Native transport construction occurs during retrieval and can fail there.
    #[must_use]
    pub fn new() -> Self {
        Self {
            client: PublicHttpClient::new(),
        }
    }

    /// Use an explicitly injected SDK public-read owner; installation trust remains separate.
    pub fn with_client(client: PublicHttpClient) -> Self {
        Self { client }
    }

    /// Read the original result-bearing global G1 and genuine H2 from one approved root.
    /// Transport is credential-free; only the shared native prefix verifier establishes trust.
    pub(super) fn fetch_amx_pair(
        &self,
        bootstrap: &AuthenticatedBootstrap,
        deadline: Instant,
    ) -> Result<(Vec<u8>, Vec<u8>), CheckpointReadError> {
        let root: url::Url = bootstrap
            .metadata()
            .torii_roots
            .first()
            .ok_or(BootstrapError::Invalid(
                "selected AMX parent has no approved endpoint",
            ))?
            .parse()
            .map_err(|_| BootstrapError::Invalid("invalid approved AMX parent endpoint"))?;
        let root_scope = bootstrap
            .verifier
            .root_scope()
            .map_err(|_| BootstrapError::Invalid("authenticated AMX parent scope"))?;
        let read = |height: u64| -> Result<Vec<u8>, CheckpointReadError> {
            let route = iroha_torii_shared::route_catalog::sumeragi::BRIDGE_FINALITY
                .path()
                .replace("{height}", &height.to_string());
            let url = root
                .join(route.trim_start_matches('/'))
                .map_err(|_| BootstrapError::Invalid("invalid exact AMX source route"))?;
            let bytes =
                self.client
                    .get_norito_bytes_blocking(&url, deadline, MAX_PUBLIC_READ_BYTES)?;
            let proof: iroha_data_model::sumeragi_finality::SumeragiFinalityProof =
                norito::decode_canonical_with_limits(
                    &bytes,
                    norito::canonical_decode_limits(bytes.len()),
                )
                .map_err(|_| BootstrapError::Invalid("invalid canonical AMX source proof"))?;
            if proof.height() != height {
                return Err(BootstrapError::Invalid(
                    "AMX source proof differs from requested height",
                )
                .into());
            }
            proof.decode_checked(root_scope.into()).map_err(|_| {
                BootstrapError::Invalid("invalid AMX source framing or certificate")
            })?;
            if Instant::now() >= deadline {
                return Err(PublicHttpError::Deadline.into());
            }
            Ok(proof.block_wire)
        };
        Ok((read(1)?, read(2)?))
    }
}

#[cfg(test)]
mod tests;
