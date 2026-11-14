//! Client paths of the anonymous Parliament family (`specs/parliament_private_ballot_design.md`
//! sections 6 and 12.3, slice 5): account- and session-free ballot submission and the two
//! public whole-record reads.
//!
//! A ballot is submitted with **none** of this client's configured headers: no API token, no
//! canonical account authentication, no witness and no caller correlation header. The client
//! sends the canonical envelope frame and its media type alone, so the request carries nothing
//! that links it to an account. Network-origin hiding still needs a qualified anonymity
//! transport, which this path does not provide (design section 6).
//!
//! The wallet that proves and stamps a ballot is `iroha_parliament_ballot::wallet`; this
//! module only transports its canonical envelope and reads the public records it needs.

use iroha_data_model::governance::{
    anonymous_ballot::{BallotScopeV1, CanonicalAnonymousBallotEnvelopeV1},
    types::GovernanceAttemptId,
};

use super::*;

/// Path of the account- and session-free submission route.
pub(crate) const ANONYMOUS_BALLOT_SUBMIT_PATH: &str = "v1/gov/anonymous-ballots";
/// Prefix of the public board read; the scope follows as 128 lowercase hex digits.
pub(crate) const ANONYMOUS_BALLOT_BOARD_PATH: &str = "v1/gov/anonymous-ballots/boards";
/// Prefix of the public attempt read; the attempt id follows as 64 lowercase hex digits.
pub(crate) const ANONYMOUS_BALLOT_ATTEMPT_PATH: &str = "v1/gov/anonymous-ballots/attempts";
/// Largest board projection the node serves (`MAX_ANONYMOUS_BALLOT_BOARD_READ_BYTES_V1`).
pub(crate) const ANONYMOUS_BALLOT_BOARD_MAX_BYTES: usize = 1024 * 1024;
/// Largest attempt projection the node serves
/// (`MAX_ANONYMOUS_PARLIAMENT_ATTEMPT_READ_BYTES_V1`).
pub(crate) const ANONYMOUS_PARLIAMENT_ATTEMPT_MAX_BYTES: usize = 16 * 1024 * 1024;

/// Lowercase hexadecimal of `bytes`.
fn lower_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            use core::fmt::Write as _;
            let _ = write!(out, "{byte:02x}");
            out
        })
}

impl Client {
    /// The submission request: the envelope frame and its media type, and none of this client's
    /// configured headers.
    pub(crate) fn anonymous_ballot_submission_request(
        &self,
        envelope: &CanonicalAnonymousBallotEnvelopeV1,
    ) -> DefaultRequestBuilder {
        let url = join_torii_url(&self.torii_url, ANONYMOUS_BALLOT_SUBMIT_PATH);
        let mut builder = DefaultRequestBuilder::new(HttpMethod::POST, url)
            .with_transport(self.http_transport.clone())
            .header("Content-Type", APPLICATION_NORITO)
            .body(envelope.as_bytes().to_vec());
        if self.torii_request_timeout != Duration::ZERO {
            builder = builder.timeout(self.torii_request_timeout);
        }
        builder
    }

    /// POST `/v1/gov/anonymous-ballots`: submit one canonical signerless ballot envelope.
    ///
    /// `Ok(())` means the node handled the ballot: it is resident in its queue, or its nullifier
    /// is already pending or committed. It is **not** finalized acceptance; confirm acceptance
    /// from the scope's corpus ([`Self::anonymous_ballot_board`]), and resubmit through several
    /// peers until it is there (design section 6).
    ///
    /// # Errors
    /// Transport failure, or any response other than `202`: a stable rejection code (`400`),
    /// a body above the node's limit (`413`), local back-pressure or deferral (`429`) and an
    /// unavailable node (`503`). The error carries the node's stable code, never a reason that
    /// identifies the ballot.
    pub fn submit_anonymous_ballot(
        &self,
        envelope: &CanonicalAnonymousBallotEnvelopeV1,
    ) -> Result<()> {
        let response = self
            .anonymous_ballot_submission_request(envelope)
            .build()?
            .send_blocking()?;
        if response.status() != StatusCode::ACCEPTED {
            return Err(
                ResponseReport::with_msg("Anonymous ballot was not handled", &response)
                    .unwrap_or_else(core::convert::identity)
                    .into(),
            );
        }
        Ok(())
    }

    /// GET `/v1/gov/anonymous-ballots/boards/{scope}`: the canonical Norito bytes of the public
    /// board of `scope` (`ParliamentBallotBoardRecordV1`): its sealed context, casting window and
    /// whole corpus. A wallet builds its proof context from it and confirms acceptance in it.
    ///
    /// # Errors
    /// Transport failure, a missing board (`404`) or any other non-`200` response.
    pub fn anonymous_ballot_board(&self, scope: &BallotScopeV1) -> Result<Vec<u8>> {
        let url = join_torii_url_with_path_segments(
            &self.torii_url,
            ANONYMOUS_BALLOT_BOARD_PATH,
            &[&lower_hex(scope.as_bytes())],
        );
        self.anonymous_family_read(
            url,
            ANONYMOUS_BALLOT_BOARD_MAX_BYTES,
            "anonymous-ballot board",
        )
    }

    /// GET `/v1/gov/anonymous-ballots/attempts/{governance_attempt_id}`: the canonical Norito
    /// bytes of one anonymous Parliament attempt (its elections, assignments, bodies and
    /// committed schedule).
    ///
    /// # Errors
    /// Transport failure, a missing attempt (`404`) or any other non-`200` response.
    pub fn anonymous_parliament_attempt(
        &self,
        governance_attempt_id: &GovernanceAttemptId,
    ) -> Result<Vec<u8>> {
        let url = join_torii_url_with_path_segments(
            &self.torii_url,
            ANONYMOUS_BALLOT_ATTEMPT_PATH,
            &[&lower_hex(governance_attempt_id.as_bytes())],
        );
        self.anonymous_family_read(
            url,
            ANONYMOUS_PARLIAMENT_ATTEMPT_MAX_BYTES,
            "anonymous Parliament attempt",
        )
    }

    fn anonymous_family_read(&self, url: Url, max_bytes: usize, what: &str) -> Result<Vec<u8>> {
        let response = self.send_builder(
            self.default_request(HttpMethod::GET, url)
                .header("Accept", APPLICATION_NORITO)
                .max_response_bytes(max_bytes),
        )?;
        if response.status() != StatusCode::OK {
            return Err(
                ResponseReport::with_msg(format!("Failed to read the {what}"), &response)
                    .unwrap_or_else(core::convert::identity)
                    .into(),
            );
        }
        Ok(response.into_body())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::evidence_http_tests::{base_url, client_with_base_url};

    /// A canonical envelope frame on `network`: structural bytes only, no valid proof.
    fn envelope(network_id: iroha_data_model::NetworkId) -> CanonicalAnonymousBallotEnvelopeV1 {
        use iroha_data_model::governance::anonymous_ballot::{
            AdmissionNonceV1, AdmissionStampV1, AnonymousBallotEnvelopeV1, AnonymousBallotV1,
            BallotChoiceV1, BallotProofBytesV1, NullifierV1, ProofProfileIdV1, VoteAuthorizationV1,
        };
        CanonicalAnonymousBallotEnvelopeV1::from_envelope(&AnonymousBallotEnvelopeV1 {
            network_id,
            ballot: AnonymousBallotV1 {
                scope: BallotScopeV1::new([0x44; 64]),
                vote: BallotChoiceV1::Aye,
                nullifier: NullifierV1::new([0x33; 128]),
                auth_v: VoteAuthorizationV1::new([0x55; 64]),
                profile_id: ProofProfileIdV1::V1,
                stamp: AdmissionStampV1 {
                    nonce: AdmissionNonceV1::new([0x66; 32]),
                },
                proof: BallotProofBytesV1::new(vec![0x6b; BallotProofBytesV1::LENGTH])
                    .expect("exact proof length"),
            },
        })
        .expect("a canonical frame")
    }

    #[test]
    fn a_ballot_request_carries_none_of_the_clients_credentials() {
        let mut client = client_with_base_url(base_url());
        for (name, value) in [
            ("X-API-Token", "listener-secret"),
            ("Authorization", "Bearer session"),
            (HEADER_ACCOUNT, "alice"),
            (HEADER_SIGNATURE, "c2lnbmF0dXJl"),
            (HEADER_WITNESS, "d2l0bmVzcw"),
            ("X-Request-Id", "correlate-me"),
        ] {
            client.headers.insert(name.to_owned(), value.to_owned());
        }
        let envelope = envelope(*client.network_id());
        let snapshot = client
            .anonymous_ballot_submission_request(&envelope)
            .build()
            .expect("a request")
            .snapshot();
        assert_eq!(snapshot.method, HttpMethod::POST);
        assert!(snapshot.url.path().ends_with("/v1/gov/anonymous-ballots"));
        // Only the media type: no token, account authentication, witness or correlation.
        assert_eq!(
            snapshot.headers,
            vec![("content-type".to_owned(), APPLICATION_NORITO.to_owned())]
        );
        assert_eq!(snapshot.body, envelope.as_bytes());
        // An ordinary read of this client keeps its configured listener credential.
        let read = client
            .default_request(HttpMethod::GET, base_url())
            .build()
            .expect("a request")
            .snapshot();
        assert!(read.headers.iter().any(|(name, _)| name == "x-api-token"));
    }

    #[test]
    fn read_paths_carry_one_canonical_lowercase_key() {
        assert_eq!(lower_hex(&[0x00, 0xab, 0x5f]), "00ab5f");
        let scope = BallotScopeV1::new([0xcd; 64]);
        let client = client_with_base_url(base_url());
        let url = join_torii_url_with_path_segments(
            &client.torii_url,
            ANONYMOUS_BALLOT_BOARD_PATH,
            &[&lower_hex(scope.as_bytes())],
        );
        assert!(url.path().ends_with(&format!(
            "/v1/gov/anonymous-ballots/boards/{}",
            "cd".repeat(64)
        )));
        let attempt = GovernanceAttemptId::new([0x5a; 32]);
        let url = join_torii_url_with_path_segments(
            &client.torii_url,
            ANONYMOUS_BALLOT_ATTEMPT_PATH,
            &[&lower_hex(attempt.as_bytes())],
        );
        assert!(url.path().ends_with(&format!(
            "/v1/gov/anonymous-ballots/attempts/{}",
            "5a".repeat(32)
        )));
    }
}
