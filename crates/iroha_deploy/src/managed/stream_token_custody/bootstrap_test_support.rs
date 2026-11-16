//! Test-only preparation through the custody owner using genuine selected native cuts.
use super::*;
use crate::managed::native_operation::attempts::metadata;
use crate::managed::native_operation::test_support::native_fixture::{
    NativeFixture, NativeReadHttp,
};

/// Supplies an actual State proof at the exact retained native checkpoint.
pub(super) struct NativeEnrollmentReads<'a>(pub(super) &'a NativeFixture);
impl body_history::EnrollmentReads for NativeEnrollmentReads<'_> {
    fn historical(
        &self,
        owner: &ManagedStreamTokenCustody,
        policy: &SignerCustodyPolicyV1,
        checkpoint: &FinalityVerifier,
        deadline: Instant,
    ) -> Result<VerifiedStreamTokenCustodyStateV1> {
        require_deadline(deadline)?;
        assert_eq!(checkpoint.checkpoint().height(), self.0.chain.height());
        Ok(self
            .0
            .bootstrap_custody(&owner.authority, policy, checkpoint))
    }
}

impl ManagedStreamTokenCustody {
    pub(super) fn bootstrap_native_body(
        &self,
        native: &NativeFixture,
        current: &VerifiedStreamTokenCustodyStateV1,
        purpose: CustodyPurpose,
        unsigned: body_history::UnsignedEnrollment,
        utc: u64,
        options: &BoundedTransactionOptions,
    ) -> BodyHistory {
        let started = Instant::now();
        let window_ms = utc.saturating_sub(now_ms().unwrap());
        let terms = Terms::new(utc, options).unwrap();
        let turn = SigningTurn::Explicit(&terms);
        // The public enroll/renew paths carry their selected authenticated current proof
        // into finish_pending. The signing boundary still rereads its historical checkpoint.
        let history = BodyHistory::initialize(
            self,
            purpose,
            unsigned,
            &terms.fees,
            &turn,
            options.deadline,
        )
        .unwrap();
        assert!(history.has_pending());
        assert!(history.original().is_none());
        assert!(history.dispatch().is_err());
        let initialized = Instant::now();
        let finished = history.finish_pending_with_reads(
            self,
            current,
            &turn,
            options.deadline,
            &NativeEnrollmentReads(native),
        );
        // Attribute every signing window between body initialization and finish. A body that
        // expired before its signature finishes unsigned: `ok` without `signed`.
        eprintln!(
            "native body timing: body={purpose:?} window_ms={window_ms} initialize_ms={} finish_ms={} ok={} signed={}",
            initialized.duration_since(started).as_millis(),
            initialized.elapsed().as_millis(),
            finished.is_ok(),
            finished
                .as_ref()
                .is_ok_and(|history| history.original().is_some()),
        );
        finished.unwrap()
    }

    pub(in crate::managed) fn bootstrap_native_configure(
        &mut self,
        native: &mut NativeFixture,
        policy: &SignerCustodyPolicyV1,
        utc: u64,
        options: &BoundedTransactionOptions,
    ) -> ManagedCustodyProgress {
        self.bootstrap_native_configure_authorized(native, policy, || (utc, options.clone()))
    }

    // Prepare the genuine native cut first, then take Configure's UTC authorization and I/O
    // budget from `authorize` at its execution boundary. A caller that already holds a fixed
    // authorization (a parent startup's children) passes it through unchanged.
    pub(in crate::managed) fn bootstrap_native_configure_authorized(
        &mut self,
        native: &mut NativeFixture,
        policy: &SignerCustodyPolicyV1,
        authorize: impl FnOnce() -> (u64, BoundedTransactionOptions),
    ) -> ManagedCustodyProgress {
        self.authority.validate_profile().unwrap();
        self.validate_policy(policy).unwrap();
        let verifier = native.observe(&self.authority);
        let current = native.bootstrap_custody(&self.authority, policy, &verifier);
        assert!(current.current().is_none());
        let original = Original {
            selection: self.selection(&policy.binding, &current).unwrap(),
            action: Action::Configure(policy.clone()),
            checkpoint: checkpoint_bytes(&verifier).unwrap(),
        };
        let (utc, options) = authorize();
        self.bootstrap_native_original(native, CustodyPurpose::Configure, &original, utc, &options);
        self.recover_configure_selected_if_present(
            policy,
            &Fees::from_options(&options).unwrap(),
            options.deadline,
        )
        .unwrap()
        .unwrap()
    }

    pub(in crate::managed) fn bootstrap_native_enroll(
        &mut self,
        native: &mut NativeFixture,
        policy: &SignerCustodyPolicyV1,
        interval: ManagedCustodyEnrollmentInterval,
        options: &BoundedTransactionOptions,
    ) -> ManagedCustodyProgress {
        self.bootstrap_native_enroll_with_interval(native, policy, |_| interval, options)
            .1
    }

    // Select a test's original short interval only after authenticating all prerequisites.
    // The interval is then immutable and every ordinary signing/expiry check still applies.
    pub(super) fn bootstrap_native_enroll_with_interval(
        &mut self,
        native: &mut NativeFixture,
        policy: &SignerCustodyPolicyV1,
        select_interval: impl FnOnce(u64) -> ManagedCustodyEnrollmentInterval,
        options: &BoundedTransactionOptions,
    ) -> (ManagedCustodyEnrollmentInterval, ManagedCustodyProgress) {
        self.authority.validate_profile().unwrap();
        self.validate_policy(policy).unwrap();
        let configured = self.authority.directory.open_child("configure").unwrap();
        let configuration = journal::required_original(&configured).unwrap();
        let transaction = self
            .verify_wallet(configuration.directory(), &configuration, options.deadline)
            .unwrap();
        let finalized = self
            .authority
            .retained_finality(configuration.directory(), &transaction)
            .unwrap()
            .unwrap();
        let verifier = native.observe(&self.authority);
        let current = native.bootstrap_custody(&self.authority, policy, &verifier);
        let state = current.current().unwrap();
        assert_eq!(state.control().policy, *policy);
        assert_eq!(state.record().execution_height, finalized.height);
        assert_eq!(state.record().authority, self.authority.config.account);
        assert_eq!(state.record().revision, 1);
        assert!(!state.control().signer_revoked && !state.control().attester_revoked);
        assert!(state.control().active_head.is_none());
        // The fixture's in-process Torii is serving before the interval is selected: starting
        // it is fixture setup, not part of the client's signing window.
        let http = native.read_http(&self.authority.config);
        let now = now_ms().unwrap();
        let interval = select_interval(now);
        validate_interval(interval, now).unwrap();
        let unsigned = self
            .unsigned_enrollment(policy, &current, &verifier, interval, now)
            .unwrap();
        let body = self.bootstrap_native_body(
            native,
            &current,
            CustodyPurpose::InitialEnroll,
            unsigned,
            interval.deadline_unix_ms,
            options,
        );
        self.bootstrap_native_enrollment_original(
            native,
            CustodyPurpose::InitialEnroll,
            body,
            interval.deadline_unix_ms,
            options,
            Some(http),
        );
        let progress = self
            .recover_enroll_selected_if_present(
                policy,
                &Fees::from_options(options).unwrap(),
                options.deadline,
            )
            .unwrap()
            .unwrap();
        (interval, progress)
    }

    pub(super) fn bootstrap_native_original(
        &self,
        native: &mut NativeFixture,
        purpose: CustodyPurpose,
        original: &Original,
        utc: u64,
        options: &BoundedTransactionOptions,
    ) {
        self.validate_original(original, purpose).unwrap();
        let directory = metadata::ensure_operation(
            &self.authority.directory,
            &purpose.directory_name().unwrap(),
        )
        .unwrap();
        journal::publish_intent(&directory, original).unwrap();
        let account = AccountService::new(self.authority.config.clone()).unwrap();
        journal::explicit(
            &directory,
            original,
            utc,
            options,
            &account,
            &HistoryScope::FixedBody,
        )
        .unwrap();
        let original = journal::required_original(&directory).unwrap();
        self.bootstrap_native_selected(native, &original, &account, options, None);
    }

    fn bootstrap_native_enrollment_original(
        &self,
        native: &mut NativeFixture,
        purpose: CustodyPurpose,
        history: BodyHistory,
        utc: u64,
        options: &BoundedTransactionOptions,
        http: Option<NativeReadHttp>,
    ) {
        let started = Instant::now();
        let (directory, original, scope) = history.dispatch().unwrap();
        self.validate_original(original, purpose).unwrap();
        let account = AccountService::new(self.authority.config.clone()).unwrap();
        journal::explicit(directory, original, utc, options, &account, scope).unwrap();
        let journaled = Instant::now();
        let selected = history.into_reparsed_selected(self).unwrap();
        eprintln!(
            "native request timing: body={purpose:?} journal_ms={} reparse_ms={}",
            journaled.duration_since(started).as_millis(),
            journaled.elapsed().as_millis(),
        );
        self.bootstrap_native_selected(native, &selected, &account, options, http);
    }

    fn bootstrap_native_selected(
        &self,
        native: &mut NativeFixture,
        original: &Selected<Original>,
        account: &AccountService,
        options: &BoundedTransactionOptions,
        http: Option<NativeReadHttp>,
    ) {
        let directory = original.directory();
        let mut http = http.unwrap_or_else(|| native.read_http(&self.authority.config));
        let path = directory.path().join("transaction");
        let started = Instant::now();
        match original
            .request(original.terms.signing_deadline(options.deadline).unwrap())
            .unwrap()
        {
            journal::Request::Configure(request) => {
                account.prepare_stream_token_custody_configure(&request, &path)
            }
            journal::Request::Enroll(request) => {
                account.prepare_stream_token_custody_enroll(&request, &path)
            }
        }
        .unwrap();
        let prepared = Instant::now();
        let signed = self
            .verify_wallet(directory, original, options.deadline)
            .unwrap();
        let verified = Instant::now();
        http.finish();
        eprintln!(
            "native wallet timing: prepare_ms={} verify_ms={} signing_window_left_ms={}",
            prepared.duration_since(started).as_millis(),
            verified.duration_since(prepared).as_millis(),
            original
                .terms
                .signing_deadline_unix_ms
                .saturating_sub(now_ms().unwrap()),
        );
        assert!(!path.join("submission.json").exists());
        let carrier = native.bootstrap_commit(&self.authority, &signed);
        assert!(
            carrier.checkpoint().height()
                > self
                    .authority
                    .decode_checkpoint(&original.checkpoint)
                    .unwrap()
                    .checkpoint()
                    .height()
        );
        directory
            .write_atomic(
                "carrier.nrt",
                &checkpoint_bytes(&carrier).unwrap(),
                PublishMode::CreateNew,
            )
            .unwrap();
    }
}

// Component tests retain the ordinary wallet and certified native carrier. The caller must
// supply a real initial interval whose midpoint has elapsed; no generated clock is bypassed.
impl ManagedStreamTokenCustody {
    pub(in crate::managed) fn bootstrap_native_renew(
        &mut self,
        native: &mut NativeFixture,
        policy: &SignerCustodyPolicyV1,
        sequence: u64,
        utc: u64,
        options: &BoundedTransactionOptions,
    ) -> RetainedCustodyEnrollment {
        let verifier = native.observe(&self.authority);
        let current = native.bootstrap_custody(&self.authority, policy, &verifier);
        let terms = Terms::new(utc, options).unwrap();
        let unsigned = self
            .select_renewal_unsigned(
                sequence,
                policy,
                &current,
                &verifier,
                &terms,
                options.deadline,
            )
            .unwrap();
        let body = self.bootstrap_native_body(
            native,
            &current,
            CustodyPurpose::Renewal(sequence),
            unsigned,
            utc,
            options,
        );
        self.bootstrap_native_enrollment_original(
            native,
            CustodyPurpose::Renewal(sequence),
            body,
            utc,
            options,
            None,
        );
        let recovered = self
            .recover_renewal(sequence, options.deadline)
            .unwrap()
            .unwrap();
        let retained = self
            .retained_renewed_enrollment(sequence, policy, options.deadline)
            .unwrap();
        assert_eq!(recovered.finalized.as_ref(), Some(retained.finalized()));
        retained
    }
}

/// Real independently challenged finality and same-cut native custody, with transport replaced
/// only by the actual executed fixture. Used to qualify provider scheduling, not service Ready.
impl ManagedStreamTokenCustody {
    pub(in crate::managed) fn test_native_current(
        &self,
        native: &NativeFixture,
        policy: &SignerCustodyPolicyV1,
        deadline: Instant,
        challenges: &std::sync::Mutex<Vec<[u8; 32]>>,
    ) -> Result<VerifiedStreamTokenCustodyStateV1> {
        self.authority.validate_profile()?;
        let (checkpoint, verified) = self
            .authority
            .observe_finality_with_source(deadline, |_, _| {
                Ok(CountedNativeCurrent { native, challenges })
            })?;
        assert_eq!(verified, 4);
        let current =
            native.bootstrap_custody_before(&self.authority, policy, &checkpoint, deadline)?;
        self.authority.validate_profile()?;
        require_deadline(deadline)?;
        Ok(current)
    }
}
struct CountedNativeCurrent<'a> {
    native: &'a NativeFixture,
    challenges: &'a std::sync::Mutex<Vec<[u8; 32]>>,
}
impl crate::verify::finality::FinalitySource for CountedNativeCurrent<'_> {
    type Error = std::io::Error;
    fn finality_proof(
        &self,
        height: std::num::NonZeroU64,
    ) -> std::io::Result<iroha_data_model::sumeragi_finality::SumeragiFinalityProof> {
        crate::verify::finality::FinalitySource::finality_proof(self.native, height)
    }
    fn latest_attestation(
        &self,
        peer: &iroha_model_base::peer::PeerId,
        challenge: &[u8; 32],
    ) -> std::io::Result<crate::verify::finality::FinalityAttestation> {
        self.challenges.lock().unwrap().push(*challenge);
        crate::verify::finality::FinalitySource::latest_attestation(self.native, peer, challenge)
    }
}
