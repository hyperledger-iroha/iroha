//! Retain entropy failures across the native proof callback without inventing a proof verdict.

use iroha_kagemusha_proof::finality::continuity::producer::Error as ProofError;
use iroha_pasta::CancellationToken;

use super::*;
use crate::kagemusha_wallet_proofs_v1::randomness as salts;

pub(super) struct NodeEntropy {
    cancellation: CancellationToken,
    failure: Option<salts::Error>,
}

impl NodeEntropy {
    pub(super) fn new(cancellation: &CancellationToken) -> Self {
        Self {
            cancellation: cancellation.clone(),
            failure: None,
        }
    }

    pub(super) fn draw(
        &mut self,
        mut fill: impl FnMut(&mut [u8; 32]) -> std::result::Result<(), salts::Error>,
    ) -> std::result::Result<NodeRandomness<'static>, ProofError> {
        let result = (|| {
            Ok(NodeRandomness {
                inner_salt: salts::sample(Some(&self.cancellation), &mut fill)?,
                outer_salt: salts::sample::<Fq>(Some(&self.cancellation), &mut fill)?.to_repr(),
                source: ProverRandomness::hedged(),
                wrapper: ProverRandomness::hedged(),
            })
        })();
        result.map_err(|error| {
            self.failure.get_or_insert(error);
            match error {
                salts::Error::Cancelled => ProofError::Cancelled,
                salts::Error::Unavailable => ProofError::Prover,
            }
        })
    }

    pub(super) fn finish<T>(
        &self,
        result: std::result::Result<T, ProofError>,
        original_failure: impl FnOnce() -> Option<ServerFinalityErrorV1>,
    ) -> Result<T> {
        // A callback consumer cannot hide a recorded entropy failure by returning
        // success or by replacing its provisional Prover/Cancelled carrier.
        match self.failure {
            Some(salts::Error::Unavailable) => Err(ServerFinalityErrorV1::EntropyUnavailable),
            Some(salts::Error::Cancelled) => {
                Err(ServerFinalityErrorV1::Proof(ProofError::Cancelled))
            }
            None => result
                .map_err(|error| original_failure().unwrap_or(ServerFinalityErrorV1::Proof(error))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_salts_accept_canonical_zero_on_both_curves() {
        let mut entropy = NodeEntropy::new(&CancellationToken::new());
        let mut calls = 0;
        let value = entropy
            .draw(|bytes| {
                calls += 1;
                *bytes = [0; 32];
                Ok(())
            })
            .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(value.inner_salt, Fp::ZERO);
        assert_eq!(value.outer_salt, [0; 32]);
        assert_eq!(
            entropy
                .finish(Ok(7), || panic!("successful source"))
                .unwrap(),
            7
        );
    }

    #[test]
    fn node_entropy_failure_from_either_draw_remains_unavailable() {
        for fail_at in [1, 2] {
            let mut entropy = NodeEntropy::new(&CancellationToken::new());
            let mut calls = 0;
            let error = entropy
                .draw(|bytes| {
                    calls += 1;
                    *bytes = [0; 32];
                    if calls == fail_at {
                        Err(salts::Error::Unavailable)
                    } else {
                        Ok(())
                    }
                })
                .err()
                .unwrap();
            assert_eq!(calls, fail_at);
            assert_eq!(error, ProofError::Prover);
            let reported = entropy
                .finish::<()>(Err(error), || panic!("entropy failure retained"))
                .unwrap_err();
            assert!(matches!(
                reported,
                ServerFinalityErrorV1::EntropyUnavailable
            ));
            assert!(!reported.is_cancelled());
            assert!(matches!(
                entropy.finish(Ok(()), || panic!("cannot hide failed entropy")),
                Err(ServerFinalityErrorV1::EntropyUnavailable)
            ));
        }
    }

    #[test]
    fn node_entropy_cancellation_is_distinct_and_shared() {
        let token = CancellationToken::new();
        let mut entropy = NodeEntropy::new(&token);
        token.cancel();
        assert_eq!(
            entropy.draw(|_| panic!("cancelled before entropy")).err(),
            Some(ProofError::Cancelled)
        );
        assert!(entropy.finish(Ok(()), || None).unwrap_err().is_cancelled());
        let token = CancellationToken::new();
        let mut entropy = NodeEntropy::new(&token);
        let mut calls = 0;
        assert_eq!(
            entropy
                .draw(|bytes| {
                    calls += 1;
                    *bytes = [0; 32];
                    if calls == 2 {
                        token.cancel();
                    }
                    Ok(())
                })
                .err(),
            Some(ProofError::Cancelled)
        );
        assert_eq!(calls, 2);
        assert!(entropy.finish(Ok(()), || None).unwrap_err().is_cancelled());
    }

    #[test]
    fn node_entropy_preserves_unrelated_original_and_proof_failures() {
        let entropy = NodeEntropy::new(&CancellationToken::new());
        assert!(matches!(
            entropy.finish::<()>(Err(ProofError::Prover), || Some(
                ServerFinalityErrorV1::Binding
            )),
            Err(ServerFinalityErrorV1::Binding)
        ));
        assert!(matches!(
            entropy.finish::<()>(Err(ProofError::Prover), || None),
            Err(ServerFinalityErrorV1::Proof(ProofError::Prover))
        ));
    }
}
