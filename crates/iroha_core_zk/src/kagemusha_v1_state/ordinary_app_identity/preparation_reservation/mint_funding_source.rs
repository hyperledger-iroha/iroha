//! Actual retained funding policy/clock loans; transported originals never select authority.
use super::*;
impl KagemushaOrdinaryEnrolledFinancialOwnerV1 {
    pub(crate) fn mint_funding_issuer_policy(
        &self,
    ) -> Result<&KagemushaRetailEnrollmentIssuerPolicyV1> {
        self.recheck_historical_proof_custody()?;
        Ok(&self.reservation.selected.issuer)
    }
    pub(crate) fn authenticate_mint_funding_clock(
        &self,
        raw: &[u8],
        context: &KagemushaOrdinaryCashClockContextV1,
    ) -> Result<crate::kagemusha_v1_state::KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1>
    {
        self.recheck_historical_proof_custody()?;
        let verified = match &self.reservation.selected.clock {
            SelectedClock::Native(clock) => {
                let clock = clock.lock().map_err(|_| Custody)?;
                if Some(clock.installed_selection_digest().map_err(|_| Custody)?)
                    != self.reservation.selected.clock_selection_digest
                {
                    return Err(Rejected);
                }
                clock
                    .authenticate_received_historical_signed_original(raw)
                    .map_err(|_| Custody)?
            }
            #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
            SelectedClock::Fixture(_) => return Err(Rejected),
        };
        verified
            .recheck_cash_context(context)
            .map_err(|_| Rejected)?;
        self.recheck_historical_proof_custody()?;
        Ok(verified)
    }
}
