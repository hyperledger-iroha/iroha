//! Local consumed-subset opening against direct decryption and an independent CRT oracle.
//!
//! These tests establish local arithmetic and ownership behavior, not a distributed
//! malicious-party-secure opening protocol or a qualified cryptographic profile.
mod support;

use iroha_fhe::bgv::{
    BgvError, BgvParameters, BgvProfile, Ciphertext, Plaintext, SecretKey, SwitchingKeyKind,
    fixtures::{self, FixtureEntropy},
    threshold::{
        DealerKeyShares, SubsetSecretKeyShare, ThresholdContext, ThresholdError, ThresholdSelection,
    },
};
use support::oracle;

fn profile() -> BgvProfile {
    BgvProfile::new(BgvParameters::derive(4, 257, 1, (40, 3), (41, 2), 2).expect("parameters"))
        .expect("profile")
}

fn context(profile: &BgvProfile, threshold: u16, epoch: u64) -> ThresholdContext {
    ThresholdContext::new(profile, [81; 32], epoch, threshold, vec![1, 2, 7, 19, 211])
        .expect("context")
}

fn secret(profile: &BgvProfile) -> SecretKey {
    fixtures::secret_key(profile, &mut FixtureEntropy::new([33; 64])).expect("secret")
}

fn ciphertext(profile: &BgvProfile, secret: &SecretKey) -> (Plaintext, Ciphertext) {
    let public = fixtures::public_key(profile, secret, &mut FixtureEntropy::new([34; 64]))
        .expect("public key");
    let message = Plaintext::new(
        profile,
        profile.master_plaintext(),
        (0..profile.degree())
            .map(|i| (i as u64 * 91 + 129) % profile.master_plaintext_modulus())
            .collect(),
    )
    .expect("message");
    let ciphertext = fixtures::encrypt(
        profile,
        &public,
        &message,
        &mut FixtureEntropy::new([35; 64]),
    )
    .expect("ciphertext");
    (message, ciphertext)
}

fn shares(
    profile: &BgvProfile,
    context: &ThresholdContext,
    participants: &[u16],
    tag: u8,
) -> ([u8; 32], Vec<SubsetSecretKeyShare>) {
    let limit = DealerKeyShares::required_secret_bytes(profile, context).expect("dealer budget");
    let mut dealer = fixtures::threshold_dealer(
        profile,
        context.clone(),
        secret(profile),
        limit,
        &mut FixtureEntropy::new([tag; 64]),
    )
    .expect("dealer");
    let instance = *dealer.instance();
    let plan = ThresholdSelection::new(profile, context, instance, participants).expect("subset");
    let shares = participants
        .iter()
        .map(|&participant| {
            dealer
                .take_share(participant)
                .expect("share")
                .try_into_subset_additive(profile, &plan)
                .expect("weight share")
        })
        .collect();
    (instance, shares)
}

/// Independent scalar negacyclic convolution and centered arbitrary-integer CRT.
fn oracle_open(profile: &BgvProfile, key: &SecretKey, ciphertext: &Ciphertext) -> Vec<u64> {
    assert_eq!(ciphertext.component_count(), 2);
    let primes = &profile.parameters().main_primes[..=ciphertext.level()];
    let modulus = oracle::product(primes);
    let component_words = primes.len() * profile.degree();
    let words = ciphertext.canonical_words();
    let c0 = oracle::lift(&words[..component_words], primes);
    let c1 = oracle::lift(&words[component_words..], primes);
    let s = oracle::small(&fixtures::secret_coefficients(key));
    let inner = oracle::poly_add(&c0, &oracle::poly_mul(&c1, &s, &modulus), &modulus);
    inner
        .iter()
        .map(|value| {
            oracle::word_mod(
                &oracle::centered(value, &modulus),
                profile.master_plaintext_modulus(),
            )
        })
        .collect()
}

#[test]
fn every_admitted_subset_and_superset_opens_at_every_level() {
    let profile = profile();
    let key = secret(&profile);
    let (message, encrypted) = ciphertext(&profile, &key);
    for threshold in 1..=5 {
        let context = context(&profile, threshold, 17);
        for mask in 1_u32..32 {
            if mask.count_ones() < u32::from(threshold) {
                continue;
            }
            let selected: Vec<_> = context
                .participants()
                .iter()
                .enumerate()
                .filter_map(|(i, &participant)| ((mask >> i) & 1 == 1).then_some(participant))
                .collect();
            for level in 0..=profile.top_level() {
                let cipher = profile.switch_to_level(&encrypted, level).expect("switch");
                let reference = oracle_open(&profile, &key, &cipher);
                let direct = key.decrypt(&profile, &cipher).expect("direct decrypt");
                assert_eq!(reference, message.coefficients());
                assert_eq!(reference, direct.coefficients());
                let (instance, local) = shares(&profile, &context, &selected, 36);
                let plan = ThresholdSelection::new(&profile, &context, instance, &selected)
                    .expect("selection");
                let budget = plan
                    .required_opening_secret_bytes(&profile, level)
                    .expect("budget");
                let opened = plan
                    .open_local(&profile, &cipher, local, budget)
                    .expect("open");
                assert_eq!(
                    opened.coefficients(),
                    reference,
                    "threshold {threshold}, mask {mask}, level {level}"
                );
            }
        }
    }
}

#[test]
fn relinearized_product_opens_with_the_scalar_convolution_result() {
    let profile = profile();
    let key = secret(&profile);
    let (message, encrypted) = ciphertext(&profile, &key);
    let relin = fixtures::switching_key(
        &profile,
        &key,
        SwitchingKeyKind::Relinearization,
        &mut FixtureEntropy::new([37; 64]),
    )
    .expect("relinearization key");
    let product = profile
        .multiply(&encrypted, &encrypted, &relin)
        .expect("product");
    let modulus = oracle::big(profile.master_plaintext_modulus());
    let message: Vec<_> = message
        .coefficients()
        .iter()
        .map(|&word| oracle::big(word))
        .collect();
    let expected: Vec<_> = oracle::poly_mul(&message, &message, &modulus)
        .iter()
        .map(|value| oracle::word_mod(value, profile.master_plaintext_modulus()))
        .collect();
    let context = context(&profile, 2, 17);
    for level in 0..=profile.top_level() {
        let cipher = profile
            .switch_to_level(&product, level)
            .expect("switch product");
        assert_eq!(oracle_open(&profile, &key, &cipher), expected);
        assert_eq!(
            key.decrypt(&profile, &cipher)
                .expect("direct")
                .coefficients(),
            expected
        );
        let (instance, local) = shares(&profile, &context, &[1, 7, 211], 38);
        let plan =
            ThresholdSelection::new(&profile, &context, instance, &[1, 7, 211]).expect("plan");
        let budget = plan
            .required_opening_secret_bytes(&profile, level)
            .expect("budget");
        assert_eq!(
            plan.open_local(&profile, &cipher, local, budget)
                .expect("open")
                .coefficients(),
            expected
        );
    }
}

fn refusal(
    plan: &ThresholdSelection<'_>,
    profile: &BgvProfile,
    cipher: &Ciphertext,
    local: Vec<SubsetSecretKeyShare>,
    budget: usize,
    reason: ThresholdError,
) -> Vec<SubsetSecretKeyShare> {
    let vector_address = local.as_ptr();
    let addresses: Vec<_> = local
        .iter()
        .map(fixtures::threshold_subset_share_buffer_address)
        .collect();
    let words: Vec<_> = local
        .iter()
        .map(fixtures::threshold_subset_share_words)
        .collect();
    let error = plan
        .open_local(profile, cipher, local, budget)
        .expect_err("refusal");
    assert_eq!(error.reason(), reason);
    assert_eq!(error.to_string(), reason.to_string());
    assert!(std::error::Error::source(&error).is_some());
    let (returned_reason, retained) = error.into_parts();
    assert_eq!(returned_reason, reason);
    assert_eq!(retained.as_ptr(), vector_address);
    for ((share, address), original) in retained.iter().zip(addresses).zip(words) {
        assert_eq!(
            fixtures::threshold_subset_share_buffer_address(share),
            address
        );
        assert_eq!(*fixtures::threshold_subset_share_words(share), *original);
    }
    retained
}

#[test]
fn exact_selection_metadata_and_component_refusals_preserve_every_share() {
    let profile = profile();
    let key = secret(&profile);
    let (_, cipher) = ciphertext(&profile, &key);
    let context = context(&profile, 2, 17);
    let (instance, local) = shares(&profile, &context, &[1, 2], 39);
    let plan = ThresholdSelection::new(&profile, &context, instance, &[1, 2]).expect("plan");
    let budget = plan
        .required_opening_secret_bytes(&profile, cipher.level())
        .expect("budget");
    let local = refusal(
        &plan,
        &profile,
        &cipher,
        local,
        budget - 1,
        ThresholdError::SecretBufferLimit {
            required: budget,
            limit: budget - 1,
        },
    );
    // A refusal is retryable with the same owners, without reissuing any share.
    assert!(plan.open_local(&profile, &cipher, local, budget).is_ok());

    let (_, mut local) = shares(&profile, &context, &[1, 2], 39);
    local.pop();
    refusal(
        &plan,
        &profile,
        &cipher,
        local,
        budget,
        ThresholdError::InsufficientShares,
    );
    // Meeting the threshold is insufficient if this exact weighted selection
    // included another participant: those Lagrange coefficients cannot be reused.
    let (_, mut superset) = shares(&profile, &context, &[1, 2, 7], 39);
    superset.pop();
    let superset_plan =
        ThresholdSelection::new(&profile, &context, instance, &[1, 2, 7]).expect("superset plan");
    refusal(
        &superset_plan,
        &profile,
        &cipher,
        superset,
        usize::MAX,
        ThresholdError::InsufficientShares,
    );
    let (_, local) = shares(&profile, &context, &[1, 2, 7], 39);
    refusal(
        &plan,
        &profile,
        &cipher,
        local,
        budget,
        ThresholdError::TooManyShares,
    );
    let (_, mut local) = shares(&profile, &context, &[1, 2], 39);
    local.swap(0, 1);
    refusal(
        &plan,
        &profile,
        &cipher,
        local,
        budget,
        ThresholdError::InvalidRoster,
    );
    let (_, mut local) = shares(&profile, &context, &[1, 2], 39);
    let (_, mut duplicate) = shares(&profile, &context, &[1, 2], 39);
    local[1] = duplicate.remove(0);
    refusal(
        &plan,
        &profile,
        &cipher,
        local,
        budget,
        ThresholdError::DuplicateParticipant,
    );
    let (_, local) = shares(&profile, &context, &[1, 7], 39);
    refusal(
        &plan,
        &profile,
        &cipher,
        local,
        budget,
        ThresholdError::SelectionMismatch,
    );
    let (_, local) = shares(&profile, &context, &[1, 2], 40);
    refusal(
        &plan,
        &profile,
        &cipher,
        local,
        budget,
        ThresholdError::InstanceMismatch,
    );
    let next_epoch =
        ThresholdContext::new(&profile, [81; 32], 18, 2, context.participants().to_vec())
            .expect("epoch");
    let (_, local) = shares(&profile, &next_epoch, &[1, 2], 39);
    refusal(
        &plan,
        &profile,
        &cipher,
        local,
        budget,
        ThresholdError::ContextMismatch,
    );
    let tensor = profile.tensor(&cipher, &cipher).expect("tensor");
    let (_, local) = shares(&profile, &context, &[1, 2], 39);
    refusal(
        &plan,
        &profile,
        &tensor,
        local,
        budget,
        ThresholdError::Bgv(BgvError::ComponentCount { found: 3 }),
    );
}

#[test]
fn profile_binding_and_public_buffer_accounting_are_exact() {
    let profile = profile();
    let key = secret(&profile);
    let (_, cipher) = ciphertext(&profile, &key);
    let context = context(&profile, 2, 17);
    let (instance, local) = shares(&profile, &context, &[1, 2], 41);
    let plan = ThresholdSelection::new(&profile, &context, instance, &[1, 2]).expect("plan");
    let foreign =
        BgvProfile::new(BgvParameters::derive(4, 257, 1, (39, 3), (42, 2), 2).expect("params"))
            .expect("profile");
    let local = refusal(
        &plan,
        &foreign,
        &cipher,
        local,
        usize::MAX,
        ThresholdError::ProfileMismatch,
    );
    let (_, foreign_cipher) = ciphertext(&foreign, &secret(&foreign));
    refusal(
        &plan,
        &profile,
        &foreign_cipher,
        local,
        usize::MAX,
        ThresholdError::ProfileMismatch,
    );
    let foreign_context =
        ThresholdContext::new(&foreign, [81; 32], 17, 2, context.participants().to_vec())
            .expect("context");
    let (_, local) = shares(&foreign, &foreign_context, &[1, 2], 41);
    refusal(
        &plan,
        &profile,
        &cipher,
        local,
        usize::MAX,
        ThresholdError::ProfileMismatch,
    );
    let full_words = (profile.top_level() + 1 + profile.special_count()) * profile.degree();
    for level in 0..=profile.top_level() {
        assert_eq!(
            plan.required_opening_secret_bytes(&profile, level)
                .expect("size"),
            (2 * full_words + 3 * (level + 1) * profile.degree()) * size_of::<u64>()
        );
    }
    assert_eq!(
        plan.required_opening_secret_bytes(&foreign, 0),
        Err(ThresholdError::ProfileMismatch)
    );
    assert_eq!(
        plan.required_opening_secret_bytes(&profile, profile.top_level() + 1),
        Err(ThresholdError::Bgv(BgvError::LevelOutOfRange {
            level: profile.top_level() + 1,
            top: profile.top_level()
        }))
    );
}
