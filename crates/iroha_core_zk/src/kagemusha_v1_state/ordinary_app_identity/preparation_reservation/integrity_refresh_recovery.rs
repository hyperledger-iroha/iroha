//! Completed private financial custody for PI recovery, deliberately separate from live cash.
use super::super::super::{
    KagemushaOrdinaryAppEnrollmentAttemptV1 as Attempt,
    KagemushaOrdinaryAppPossessionAttemptV1 as Possession,
    KagemushaOrdinaryRetailEnrollmentAttemptV1 as Retail,
};
use super::*;
use std::path::PathBuf;

/// Exclusive Native recovery staging holder. Only an exact completed financial WAL and
/// genuine retained FI/C/possession admission can construct it. Old/pending PI does not grant
/// live FI, a State publication, current S/W, a money loan or a platform reinvocation.
/// Its separately fresh PI workflow stays reachable before Bootstrap composition.
/// This type has no decoder, raw certificate constructor, Clone, key or financial-secret getter.
pub struct KagemushaOrdinaryRetainedFinancialIntegrityRecoveryV1 {
    root: PathBuf,
    directory: iroha_fs::ReaderDirectory,
    financial: KagemushaOrdinaryEnrolledFinancialOwnerV1,
    refresh: Option<KagemushaOrdinaryIntegrityRefreshOwnerV1>,
    create_attempted: bool,
    ceremony: Option<Box<CompletedCeremony>>,
}
struct CompletedCeremony {
    attempt: Attempt,
    possession: Possession,
    retail: Retail,
}
impl KagemushaOrdinaryRetainedFinancialIntegrityRecoveryV1 {
    /// Cold-open only genuine completed Native enrollment and its full upstream journals.
    /// True incomplete completion returns None without creating or invoking anything. Missing,
    /// malformed or changed existing originals never select a new enrollment or a fresh lease.
    /// Current native account/runtime/static policy and signed clock custody remain required.
    /// # Errors
    /// Refuses storage replacement, torn prefix or any different C/E/FI/key/completed original.
    pub fn open_completed_native_custody_if_present(
        root: &Path,
        selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
    ) -> Result<Option<Self>> {
        let now = selected.trusted_time_interval()?.lower_ms();
        let reservation = KagemushaOrdinaryPreparationReservationV1::open_originals(
            root,
            Arc::clone(&selected),
            now,
        )?;
        if reservation.completed.is_none() {
            return Ok(None);
        }
        let prepared = reservation.retained_prepared_owner()?;
        let enrollment_id = prepared.preparation.challenge.enrollment_id;
        let attempt =
            Attempt::open_with_native_selected(root, prepared, Arc::clone(&selected), true)?;
        let pending = attempt.retained_pending_identity()?;
        let ceremony_root = root.join(hex::encode(enrollment_id));
        let possession = Possession::open_completed_history(
            &ceremony_root.join("possession"),
            pending,
            selected,
        )?;
        let retail = Retail::open_completed_history(
            &ceremony_root.join("retail-enrollment"),
            pending,
            &possession,
            &reservation,
        )?;
        let enrollment = Arc::clone(retail.completed_enrollment(pending, &possession)?);
        let mut held = reservation
            .recover_completed_integrity_custody_or_retain(enrollment, root)
            .map_err(|(_, error)| error)?;
        held.ceremony = Some(Box::new(CompletedCeremony {
            attempt,
            possession,
            retail,
        }));
        held.recheck_retained_custody()?;
        Ok(Some(held))
    }
    /// Original completed retail ticket, for byte-identical C20 completion retry only.
    /// # Errors
    /// Refuses changed historical custody; fresh holders have no cold ceremony ticket.
    pub fn completed_retail_ticket(&self) -> Result<Option<u64>> {
        self.recheck_retained_custody()?;
        Ok(self.ceremony.as_ref().map(|held| held.retail.ticket()))
    }
    /// Immutable native scope from the original completed financial reservation.
    /// # Errors
    /// Refuses altered C/FI/reservation storage or upstream custody.
    pub fn native_scope(&self) -> Result<[u8; 32]> {
        self.recheck_retained_custody()?;
        Ok(self
            .financial
            .reservation
            .retained_prepared_owner()?
            .native_scope)
    }
    /// Full same-owner FI original from the exact completed Native WAL, for read-only retry.
    /// This original does not lend a current credential, FI control or monetary authority.
    /// # Errors
    /// Refuses changed completed private history or upstream originals.
    pub fn completed_financial_enrollment_original(&self) -> Result<Vec<u8>> {
        self.recheck_retained_custody()?;
        let original = self
            .financial
            .enrollment
            .certificate()
            .canonical_bytes()
            .map_err(|_| Custody)?;
        self.recheck_retained_custody()?;
        Ok(original)
    }
    /// Same immutable enrollment selector; no frame can choose a new enrolled owner/key.
    /// # Errors
    /// Refuses changed completed Native custody.
    pub fn enrollment_id(&self) -> Result<[u8; 32]> {
        self.recheck_retained_custody()?;
        Ok(self
            .financial
            .enrollment
            .certificate()
            .subject
            .enrollment_id)
    }

    /// Recover the exact acknowledged local PI catalog without renewing expired originals.
    /// True PI-directory absence is retained without creating it; malformed existing storage
    /// closes recovery. A previously live Financial owner may enter this stage after expiry.
    /// # Errors
    /// Returns original financial custody on any completed-record/policy/storage mismatch.
    pub fn from_completed_financial(
        root: &Path,
        financial: KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> std::result::Result<
        Self,
        (
            KagemushaOrdinaryEnrolledFinancialOwnerV1,
            KagemushaOrdinaryIdentityErrorV1,
        ),
    > {
        let setup = (|| {
            financial.recheck_historical_proof_custody()?;
            #[cfg(unix)]
            if root.canonicalize().map_err(|_| Custody)? != root {
                return Err(Custody);
            }
            let directory = iroha_fs::ReaderDirectory::open(root).map_err(|_| Custody)?;
            let refresh = KagemushaOrdinaryIntegrityRefreshOwnerV1::open_existing_if_present(
                root, &financial,
            )?;
            financial.recheck_historical_proof_custody()?;
            Ok((directory, refresh))
        })();
        let (directory, refresh) = match setup {
            Ok(parts) => parts,
            Err(error) => return Err((financial, error)),
        };
        let recovered = Self {
            root: root.to_path_buf(),
            directory,
            financial,
            refresh,
            create_attempted: false,
            ceremony: None,
        };
        if let Err(error) = recovered.recheck_retained_custody() {
            return Err((recovered.financial, error));
        }
        Ok(recovered)
    }
    /// Authenticate completed financial originals, same held directory and complete PI prefix.
    /// This check neither samples a live PI interval nor promotes expired historical tokens.
    /// # Errors
    /// Refuses swapped/changed originals, journal, selected owner or unsafe private storage.
    pub fn recheck_retained_custody(&self) -> Result<()> {
        self.financial.recheck_historical_proof_custody()?;
        if let Some(ceremony) = &self.ceremony {
            let pending = ceremony.attempt.retained_pending_identity()?;
            ceremony.possession.recheck_completed_history(pending)?;
            let enrolled = ceremony
                .retail
                .completed_enrollment(pending, &ceremony.possession)?;
            if !Arc::ptr_eq(enrolled, &self.financial.enrollment) {
                return Err(Custody);
            }
        }
        self.directory.revalidate().map_err(|_| Custody)?;
        if let Some(refresh) = &self.refresh {
            refresh.require_financial(&self.financial)?;
        } else if self.create_attempted {
            // An uncertain create cannot be retried or silently treated as an empty catalog.
            return Err(Custody);
        } else {
            match std::fs::symlink_metadata(self.root.join("ordinary-integrity-refresh")) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                _ => return Err(Custody),
            }
        }
        self.financial.recheck_historical_proof_custody()
    }
    /// Return only actual fully acknowledged verified leases from this same exclusive local WAL.
    /// The old lease expiry and raw pending lease remain unchanged; catalog bytes create no grant.
    /// # Errors
    /// Refuses changed owned prefix, C/FI/selected identity or unknown creation outcome.
    pub fn retained_integrity_catalog(
        &self,
    ) -> Result<Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>> {
        self.recheck_retained_custody()?;
        let result = self
            .refresh
            .as_ref()
            .map(|refresh| refresh.retained_verified_leases(&self.financial))
            .transpose()?
            .unwrap_or_default();
        self.recheck_retained_custody()?;
        Ok(result)
    }
    /// Immutable completion coordinates under the same genuine Financial WAL.
    /// # Errors
    /// Refuses changed originals; no live FI/PI or money capability is lent.
    pub fn completed_enrollment_fields(&self) -> Result<Vec<Vec<u8>>> {
        self.recheck_retained_custody()?;
        self.financial.retained_enrollment_completion_fields()
    }
    /// Exact completed retail originals from the same genuine Financial WAL.
    /// # Errors
    /// Refuses changed original custody; this read grants no current FI/PI or money loan.
    pub fn completed_retail_recovery_fields(&self) -> Result<Vec<Vec<u8>>> {
        self.recheck_retained_custody()?;
        self.financial.retained_enrollment_recovery_fields()
    }
    /// Same completed key metadata for recovery-only hardware binding. No PI owner is created.
    /// # Errors
    /// Refuses any changed complete Native ceremony, directory, C/key/FI or journal prefix.
    pub fn completed_app_key_fields(&self) -> Result<Vec<Vec<u8>>> {
        self.recheck_retained_custody()?;
        let fields = self.financial.retained_completed_app_key_fields()?;
        self.recheck_retained_custody()?;
        Ok(fields)
    }
    /// Recover exact completed FI/C and any retained pending PI originals without creating
    /// a journal, nonce or live permission. Unknown platform effects remain closed.
    /// # Errors
    /// Refuses substituted completed custody or an unknown prior platform invocation.
    pub fn recovery_fields(&self) -> Result<Vec<Vec<u8>>> {
        self.recheck_retained_custody()?;
        if let Some(refresh) = &self.refresh {
            return refresh.recovery_fields(&self.financial);
        }
        Ok(vec![
            self.completed_financial_enrollment_original()?,
            self.financial
                .enrollment
                .app_credential()
                .original()
                .to_vec(),
            vec![],
            vec![0],
            vec![],
            vec![],
            vec![],
            vec![],
        ])
    }
    /// Prepare a separately fresh refresh under actual Native clock/static C/FI custody.
    /// Only true absent storage creates the sole PI journal. Existing uncertainty never does.
    /// # Errors
    /// Refuses unsafe/changed history, expired static C/FI/clock or uncertain journal creation.
    pub fn prepare_integrity_refresh(&mut self) -> Result<Vec<Vec<u8>>> {
        self.recheck_retained_custody()?;
        if self.refresh.is_none() {
            self.create_attempted = true;
            self.refresh = Some(KagemushaOrdinaryIntegrityRefreshOwnerV1::create(
                &self.root,
                &self.financial,
            )?);
        }
        let fields = self
            .refresh
            .as_mut()
            .ok_or(Custody)?
            .prepare(&self.financial)?;
        self.recheck_retained_custody()?;
        Ok(fields)
    }
    /// Borrow the genuine refresh protocol owner, without lending live cash/secret custody.
    /// The owner itself enforces durable nonce/invocation/raw/signature/ack ordering, real
    /// signatures and the separately fresh Native interval at every protocol effect.
    /// # Errors
    /// Refuses absent preparation, swapped prefix, incomplete or changed completed originals.
    pub fn with_integrity_refresh_owner<T>(
        &mut self,
        consume: impl FnOnce(
            &mut KagemushaOrdinaryIntegrityRefreshOwnerV1,
            &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        ) -> Result<T>,
    ) -> Result<T> {
        self.recheck_retained_custody()?;
        let result = consume(self.refresh.as_mut().ok_or(Custody)?, &self.financial)?;
        self.recheck_retained_custody()?;
        Ok(result)
    }
    /// Activate only after actual current C/FI/clock/PI checks. Historical catalog admission
    /// itself never activates this holder. The newest acknowledged lease is checked at both
    /// genuine interval bounds before selecting the SAME retained Arc; no expiry is extended.
    /// Bootstrap must next fsync the actual lease in its own logical journal before publication.
    /// # Errors
    /// Returns this same recovery holder when no current lease is valid or storage changes.
    pub fn activate_for_bootstrap(
        mut self,
    ) -> std::result::Result<
        (
            KagemushaOrdinaryEnrolledFinancialOwnerV1,
            Option<KagemushaOrdinaryIntegrityRefreshOwnerV1>,
            Vec<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
        ),
        (Self, KagemushaOrdinaryIdentityErrorV1),
    > {
        let result = (|| {
            self.recheck_retained_custody()?;
            let leases = self.retained_integrity_catalog()?;
            if let Some(lease) = leases.last() {
                // The actual WAL's latest acknowledged original is the only chosen current
                // candidate. A later invalid/expired lease never falls back to an older one.
                self.financial
                    .select_verified_integrity_lease(Arc::clone(lease))?;
            }
            self.financial.recheck()?;
            self.recheck_retained_custody()?;
            Ok(leases)
        })();
        match result {
            Ok(leases) => Ok((self.financial, self.refresh, leases)),
            Err(error) => Err((self, error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{financial, originals, time};
    use super::*;

    fn recovery(
        root: &Path,
        financial: KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> KagemushaOrdinaryRetainedFinancialIntegrityRecoveryV1 {
        match KagemushaOrdinaryRetainedFinancialIntegrityRecoveryV1::from_completed_financial(
            root, financial,
        ) {
            Ok(held) => held,
            Err(_) => panic!("known-public complete fixture recovery unexpectedly rejected"),
        }
    }
    #[test]
    fn expired_baseline_recovery_reaches_separate_refresh_then_actual_activation() {
        // Genuine Native private WAL and known-public issuer/P256 fixture equations only.
        // No installed policy, hardware, live FI-control or money proof is manufactured.
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut financial = financial(&root, true);
        time(&mut financial, 1400);
        assert!(financial.recheck().is_err());
        let held = recovery(&root, financial);
        assert!(held.retained_integrity_catalog().unwrap().is_empty());
        let mut held = match held.activate_for_bootstrap() {
            Err((same, _)) => same,
            Ok(_) => panic!("expired baseline became current before genuine refresh"),
        };
        let prepare = held.prepare_integrity_refresh().unwrap();
        let nonce = prepare[1].as_slice().try_into().unwrap();
        let (challenge, der, lease) = originals(&held.financial, nonce);
        // TEST ONLY fixture clock after known-public cryptographic setup.
        time(&mut held.financial, 1400);
        held.with_integrity_refresh_owner(|owner, financial| {
            owner.accept_challenge(financial, &challenge.to_transport_bytes().unwrap())?;
            owner.fence_platform_invocation(financial)?;
            owner.capture_signature(financial, &der)?;
            owner.retain_integrity_token(financial, b"untrusted-google-fixture-original")?;
            Ok(())
        })
        .unwrap();
        time(&mut held.financial, 1500);
        let lease_raw = lease.canonical_bytes().unwrap();
        let actual = held
            .with_integrity_refresh_owner(|owner, financial| {
                owner.accept_lease(financial, &lease_raw)
            })
            .unwrap();
        assert!(held.financial.recheck().is_err());
        let (financial, owner, leases) = match held.activate_for_bootstrap() {
            Ok(parts) => parts,
            Err(_) => panic!("actual acknowledged current lease did not activate"),
        };
        financial.recheck().unwrap();
        assert!(Arc::ptr_eq(&actual, &leases[0]));
        assert!(Arc::ptr_eq(
            &actual,
            financial.retained_integrity_lease().unwrap()
        ));
        assert_eq!(
            owner.unwrap().retained_verified_leases(&financial).unwrap()[0].original(),
            lease_raw
        );
    }
    #[test]
    fn cold_catalog_authenticates_expired_ack_without_renewing_or_activating_it() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut financial = financial(&root, true);
        time(&mut financial, 1400);
        let mut held = recovery(&root, financial);
        let prepare = held.prepare_integrity_refresh().unwrap();
        let (challenge, der, lease) =
            originals(&held.financial, prepare[1].as_slice().try_into().unwrap());
        // TEST ONLY fixture clock after known-public cryptographic setup.
        time(&mut held.financial, 1400);
        held.with_integrity_refresh_owner(|owner, financial| {
            owner.accept_challenge(financial, &challenge.to_transport_bytes().unwrap())?;
            owner.fence_platform_invocation(financial)?;
            owner.capture_signature(financial, &der)?;
            owner.retain_integrity_token(financial, b"untrusted-google-fixture-original")?;
            Ok(())
        })
        .unwrap();
        time(&mut held.financial, 1500);
        let raw = lease.canonical_bytes().unwrap();
        held.with_integrity_refresh_owner(|owner, financial| owner.accept_lease(financial, &raw))
            .unwrap();
        let (mut financial, owner, _) = match held.activate_for_bootstrap() {
            Ok(parts) => parts,
            Err(_) => panic!("fixture lease unexpectedly rejected"),
        };
        drop(owner);
        time(&mut financial, 2500);
        assert!(financial.recheck().is_err());
        let held = recovery(&root, financial);
        assert_eq!(
            held.retained_integrity_catalog().unwrap()[0].original(),
            raw
        );
        match held.activate_for_bootstrap() {
            Err((held, _)) => {
                assert_eq!(
                    held.retained_integrity_catalog().unwrap()[0].original(),
                    raw
                );
                assert!(held.financial.recheck().is_err());
            }
            Ok(_) => panic!("expired historical lease was renewed by cold catalog"),
        }
    }
    #[cfg(unix)]
    #[test]
    fn actual_absence_never_masks_symlink_or_existing_missing_wal() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let financial = financial(&root, true);
        assert!(
            KagemushaOrdinaryIntegrityRefreshOwnerV1::open_existing_if_present(&root, &financial)
                .unwrap()
                .is_none()
        );
        std::os::unix::fs::symlink(&root, root.join("ordinary-integrity-refresh")).unwrap();
        assert!(
            KagemushaOrdinaryIntegrityRefreshOwnerV1::open_existing_if_present(&root, &financial)
                .is_err()
        );
        std::fs::remove_file(root.join("ordinary-integrity-refresh")).unwrap();
        std::fs::create_dir(root.join("ordinary-integrity-refresh")).unwrap();
        assert!(
            KagemushaOrdinaryIntegrityRefreshOwnerV1::open_existing_if_present(&root, &financial)
                .is_err()
        );
        assert!(
            !root
                .join("ordinary-integrity-refresh/ordinary-integrity-refresh.norito.wal")
                .exists()
        );
    }
}
