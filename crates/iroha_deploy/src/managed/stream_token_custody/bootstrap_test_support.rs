//! Test-only preparation through the custody owner using genuine selected native cuts.
use super::*;
use crate::managed::native_operation::test_support::native_fixture::{
    NativeFixture, NativeReadHttp,
};
use std::sync::Arc;

impl ManagedStreamTokenCustody {
    pub(in crate::managed) fn bootstrap_native_configure(
        &mut self,
        native: &mut NativeFixture,
        policy: &SignerCustodyPolicyV1,
        utc: u64,
        options: &BoundedTransactionOptions,
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
        self.bootstrap_native_original(native, CustodyPurpose::Configure, &original, utc, options);
        self.recover_configure_selected_if_present(
            policy,
            &Fees::from_options(options).unwrap(),
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
        let now = now_ms().unwrap();
        validate_interval(interval, now).unwrap();
        let original = self
            .enrollment_original(policy, &current, &verifier, interval, now)
            .unwrap();
        self.bootstrap_native_original(
            native,
            CustodyPurpose::InitialEnroll,
            &original,
            interval.deadline_unix_ms,
            options,
        );
        self.recover_enroll_selected_if_present(
            policy,
            &Fees::from_options(options).unwrap(),
            options.deadline,
        )
        .unwrap()
        .unwrap()
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
        let directory = self
            .authority
            .directory
            .ensure_child(&purpose.directory_name().unwrap())
            .unwrap();
        journal::publish_intent(&directory, original).unwrap();
        let account = AccountService::new(self.authority.config.clone()).unwrap();
        journal::explicit(&directory, original, utc, options, &account).unwrap();
        let original = journal::required_original(&directory).unwrap();
        let directory = original.directory();
        let mut http =
            NativeReadHttp::start_config(&self.authority.config, Arc::clone(native.chain.state()));
        let path = directory.path().join("transaction");
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
        let signed = self
            .verify_wallet(directory, &original, options.deadline)
            .unwrap();
        http.finish();
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
        let original = self
            .select_renewal_original(
                sequence,
                policy,
                &current,
                &verifier,
                Terms::new(utc, options).unwrap(),
                options.deadline,
            )
            .unwrap();
        self.bootstrap_native_original(
            native,
            CustodyPurpose::Renewal(sequence),
            &original,
            utc,
            options,
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
