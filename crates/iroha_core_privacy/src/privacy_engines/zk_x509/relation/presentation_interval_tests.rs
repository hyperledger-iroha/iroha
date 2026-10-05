//! Certificate and CRL interval controls over genuinely signed paths.
//!
//! Every case signs a real leaf-to-root path and a real complete CRL with the
//! requested validity periods, then presents the same public window to each
//! layer: the canonical data-model definition, the DER-to-bounds helper, the
//! native reference relation, authoritative state admission, prover preflight,
//! the DER/RFC 5280 AIR trace builder and the in-relation numeric rows. They
//! must all agree.
//!
//! The DER-to-bounds helper, the native relation and prover preflight are
//! compiled with the prover, for tests and the `privacy-release-evidence`
//! feature only; an ordinary node build verifies through state admission and
//! the proof. The constraint-level control for a forged window end is
//! `rfc5280_presentation_interval_tests.rs` beside the RFC 5280 STARK.
//!
//! The binding upper bound is the **earliest** `notAfter` in the path. A bound
//! computed from the latest expiry would present a window after an
//! intermediate or root certificate has expired; that is a security defect and
//! `earliest_expiry_*` fail on it.

use super::*;
use crate::{
    privacy_engines::zk_x509::{
        der_air::{ZkX509DerAirErrorV1, ZkX509Rfc5280TraceV1, build_zk_x509_rfc5280_trace_v1},
        engine::{ZkX509EngineErrorV1, prepare_zk_x509_prover_input_v1},
        rfc5280_stark::{
            ZkX509Rfc5280StarkErrorV1, ZkX509Rfc5280StarkShapeV1,
            build_zk_x509_rfc5280_semantic_witness_v1,
            build_zk_x509_rfc5280_stark_base_material_v1,
        },
        verifier_profile::rfc_statement_with_crl_number_v1,
    },
    privacy_state::validate_privacy_zk_x509_statement_state_v1,
};
use iroha_data_model::privacy::{
    PrivacyZkX509PresentationIntervalErrorV1, ZK_X509_MAX_PRESENTATION_WINDOW_SECONDS_V1,
    validate_zk_x509_presentation_interval_v1,
};
use norito::json::{self, Value};

/// Shared cross-language vectors; regenerate with
/// `python3 scripts/check_zk_x509_presentation_interval.py --write`.
const VECTORS: &str = include_str!("../../../../../../fixtures/zk/x509/interval_vectors_v1.json");

/// 2023-01-01T00:00:00Z, the fixed release CRL `thisUpdate`.
const T: u64 = CRL_THIS_UPDATE;
/// The slack range enforced by the in-relation numeric rows.
const NUMERIC_SLACK_BITS: u32 = 38;

fn validity(not_before: u64, not_after: u64) -> PrivacyZkX509CertificateValidityV1 {
    PrivacyZkX509CertificateValidityV1::new(not_before, not_after)
}

fn wide() -> PrivacyZkX509CertificateValidityV1 {
    FIXED_CERTIFICATE_VALIDITY_V1
}

fn window(start: u64, end: u64) -> PrivacyZkX509PresentationWindowV1 {
    PrivacyZkX509PresentationWindowV1::new(start, end)
}

fn fresh_crl() -> PrivacyZkX509CrlUpdateIntervalV1 {
    PrivacyZkX509CrlUpdateIntervalV1::new(T, T + 301)
}

/// A genuinely signed path and CRL, bound to an arbitrary first window.
fn signed_fixture(
    certificates: &[PrivacyZkX509CertificateValidityV1],
    crl: PrivacyZkX509CrlUpdateIntervalV1,
) -> ZkX509ReleaseFixtureV1 {
    let fixture = build_zk_x509_interval_fixture_v1(
        certificates,
        crl,
        window(
            crl.this_update_unix_seconds,
            crl.this_update_unix_seconds + 1,
        ),
    )
    .expect("genuinely signed interval fixture");
    assert_eq!(
        fixture.witness.certificate_chain_der.len(),
        certificates.len()
    );
    // The DER carries exactly the requested signed intervals.
    for (der, expected) in fixture
        .witness
        .certificate_chain_der
        .iter()
        .zip(certificates)
    {
        let parsed = parse_certificate_v1(der).expect("strictly parsed fixture certificate");
        assert_eq!(parsed.not_before, expected.not_before_unix_seconds());
        assert_eq!(parsed.not_after, expected.not_after_unix_seconds());
    }
    let parsed_crl = parse_crl_v1(&fixture.witness.crl_der).expect("strictly parsed fixture CRL");
    assert_eq!(parsed_crl.this_update, crl.this_update_unix_seconds);
    assert_eq!(parsed_crl.next_update, crl.next_update_unix_seconds);
    fixture
}

/// One fixture rebound to one public window, with a matching ownership
/// signature, so a rejection can only come from the interval predicates.
struct Presentation<'a> {
    fixture: &'a ZkX509ReleaseFixtureV1,
    statement: IrohaZkX509StarkP256StatementV1,
    witness: ZkX509WitnessV1,
}

impl<'a> Presentation<'a> {
    fn new(fixture: &'a ZkX509ReleaseFixtureV1, window: PrivacyZkX509PresentationWindowV1) -> Self {
        let (statement, witness) =
            rewindow_zk_x509_fixture_v1(fixture, window).expect("rebound ownership signature");
        assert_eq!(statement.presentation_window(), window);
        Self {
            fixture,
            statement,
            witness,
        }
    }

    /// The complete native reference relation.
    fn native_relation(&self) -> Result<ZkX509RelationOutputV1, ZkX509RelationErrorV1> {
        let trust_anchor = self.fixture.authoritative_state.trust_anchor();
        let crl = self.fixture.authoritative_state.crl_record();
        validate_reference_relation_v1(
            &self.statement,
            ZkX509GovernanceV1 {
                trust_anchor: &trust_anchor,
                certificate_policy: self.fixture.authoritative_state.certificate_policy(),
                crl: &crl,
            },
            &self.witness,
        )
    }

    /// The independent DER/RFC 5280 AIR trace builder.
    fn der_air_trace(&self) -> Result<ZkX509Rfc5280TraceV1, ZkX509DerAirErrorV1> {
        build_zk_x509_rfc5280_trace_v1(
            &self.witness.certificate_chain_der,
            &self.witness.crl_der,
            rfc_statement_with_crl_number_v1(
                &self.statement,
                self.fixture.authoritative_state.crl_record().crl_number,
            ),
        )
    }

    /// Prover preflight (state admission plus the reference relation) at one
    /// trusted block timestamp. It is compiled only with the prover.
    fn prover_preflight(&self, trusted_block_timestamp_ms: u64) -> Result<(), ZkX509EngineErrorV1> {
        prepare_zk_x509_prover_input_v1(
            &self.statement,
            &self.fixture.authoritative_state,
            trusted_block_timestamp_ms,
            &PrivacyConsensusLimitsV1::taira_default(),
            &self
                .witness
                .encode_v1()
                .expect("canonical witness encoding"),
        )
        .map(|_| ())
    }

    /// Authoritative-state admission at one block timestamp.
    fn state_admission(&self, trusted_block_timestamp_ms: u64) -> Result<(), String> {
        validate_privacy_zk_x509_statement_state_v1(
            &self.statement,
            &self.fixture.authoritative_state,
            trusted_block_timestamp_ms,
            &PrivacyConsensusLimitsV1::taira_default(),
        )
    }
}

fn native_relation(
    fixture: &ZkX509ReleaseFixtureV1,
    window: PrivacyZkX509PresentationWindowV1,
) -> Result<ZkX509RelationOutputV1, ZkX509RelationErrorV1> {
    Presentation::new(fixture, window).native_relation()
}

fn der_air_trace(
    fixture: &ZkX509ReleaseFixtureV1,
    window: PrivacyZkX509PresentationWindowV1,
) -> Result<ZkX509Rfc5280TraceV1, ZkX509DerAirErrorV1> {
    Presentation::new(fixture, window).der_air_trace()
}

fn prover_preflight(
    fixture: &ZkX509ReleaseFixtureV1,
    window: PrivacyZkX509PresentationWindowV1,
    trusted_block_timestamp_ms: u64,
) -> Result<(), ZkX509EngineErrorV1> {
    Presentation::new(fixture, window).prover_preflight(trusted_block_timestamp_ms)
}

fn state_admission(
    fixture: &ZkX509ReleaseFixtureV1,
    window: PrivacyZkX509PresentationWindowV1,
    trusted_block_timestamp_ms: u64,
) -> Result<(), String> {
    Presentation::new(fixture, window).state_admission(trusted_block_timestamp_ms)
}

/// Check the in-relation numeric rows of one admitted trace against the
/// signed intervals, and return how many certificate rows it carries.
fn assert_numeric_rows_bind_every_certificate(
    trace: &ZkX509Rfc5280TraceV1,
    certificates: &[PrivacyZkX509CertificateValidityV1],
    crl: PrivacyZkX509CrlUpdateIntervalV1,
    window: PrivacyZkX509PresentationWindowV1,
) {
    let semantic =
        build_zk_x509_rfc5280_semantic_witness_v1(trace).expect("admitted semantic witness");
    let relation = |code: u16, instance: usize| {
        let mut rows = semantic
            .numeric_relations
            .iter()
            .filter(|row| row.relation == code && usize::from(row.instance) == instance);
        let row = rows
            .next()
            .unwrap_or_else(|| panic!("numeric relation {code}/{instance} is missing"));
        assert!(
            rows.next().is_none(),
            "numeric relation {code}/{instance} is duplicated"
        );
        assert_eq!(row.left, row.right + row.slack);
        assert!(row.slack < 1 << NUMERIC_SLACK_BITS);
        assert!(!row.strict || row.slack != 0);
        *row
    };
    for (index, certificate) in certificates.iter().enumerate() {
        // Relation 1: start >= notBefore_i. Relation 2: notAfter_i >= end.
        let after_not_before = relation(1, index);
        assert_eq!(after_not_before.left, window.not_before_unix_seconds);
        assert_eq!(
            after_not_before.right,
            certificate.not_before_unix_seconds()
        );
        assert!(!after_not_before.strict);
        let before_not_after = relation(2, index);
        assert_eq!(before_not_after.left, certificate.not_after_unix_seconds());
        assert_eq!(before_not_after.right, window.not_after_unix_seconds);
        assert!(!before_not_after.strict);
        // The DER AIR path rows carry the same two differences.
        assert_eq!(
            trace.path_rows[index].after_not_before.value.0,
            window.not_before_unix_seconds - certificate.not_before_unix_seconds()
        );
        assert_eq!(
            trace.path_rows[index].before_not_after.value.0,
            certificate.not_after_unix_seconds() - window.not_after_unix_seconds
        );
    }
    assert_eq!(
        semantic
            .numeric_relations
            .iter()
            .filter(|row| matches!(row.relation, 1 | 2))
            .count(),
        2 * certificates.len(),
        "exactly one lower and one upper relation per certificate"
    );
    // Relation 4: start >= thisUpdate. Relation 5: nextUpdate > end, strict.
    // Relation 6: thisUpdate + 300 >= end.
    let after_this_update = relation(4, 0);
    assert_eq!(after_this_update.left, window.not_before_unix_seconds);
    assert_eq!(after_this_update.right, crl.this_update_unix_seconds);
    let before_next_update = relation(5, 0);
    assert_eq!(before_next_update.left, crl.next_update_unix_seconds);
    assert_eq!(before_next_update.right, window.not_after_unix_seconds);
    assert!(before_next_update.strict && before_next_update.slack >= 1);
    let fresh = relation(6, 0);
    assert_eq!(
        fresh.left,
        crl.this_update_unix_seconds + ZK_X509_MAX_CRL_AGE_SECONDS_V1
    );
    assert_eq!(fresh.right, window.not_after_unix_seconds);
    trace
        .validate()
        .expect("admitted DER/RFC trace is self-consistent");
}

/// Present one window to every verifier layer and require the same verdict.
///
/// `admitted_trace` is a trace of some admitted window of the same fixture, if
/// one exists; it lets a rejected window reach the in-relation numeric rows
/// without the DER AIR preflight in front of them.
fn assert_all_layers_agree(
    label: &str,
    fixture: &ZkX509ReleaseFixtureV1,
    certificates: &[PrivacyZkX509CertificateValidityV1],
    crl: PrivacyZkX509CrlUpdateIntervalV1,
    presentation: PrivacyZkX509PresentationWindowV1,
    admitted_trace: Option<&ZkX509Rfc5280TraceV1>,
) -> bool {
    let canonical = validate_zk_x509_presentation_interval_v1(certificates, crl, presentation);
    let admitted = canonical.is_ok();
    let context = format!(
        "{label}: window [{}, {}]",
        presentation.not_before_unix_seconds, presentation.not_after_unix_seconds
    );
    // DER-to-bounds helper: derived from the same DER the relation verifies.
    let derived = derive_zk_x509_presentation_bounds_v1(
        &fixture.witness.certificate_chain_der,
        &fixture.witness.crl_der,
    );
    assert_eq!(
        derived.is_ok_and(|bounds| bounds.admit(presentation).is_ok()),
        admitted,
        "{context}: builder bounds"
    );
    let rebound = Presentation::new(fixture, presentation);
    // Native reference relation.
    match rebound.native_relation() {
        Ok(_) => assert!(admitted, "{context}: native relation accepted"),
        Err(error) => {
            assert!(
                !admitted,
                "{context}: native relation rejected with {error}"
            );
            assert!(
                matches!(
                    error,
                    ZkX509RelationErrorV1::CertificateNotValid | ZkX509RelationErrorV1::InvalidCrl
                ),
                "{context}: non-temporal native rejection {error}"
            );
        }
    }
    // Independent DER/RFC 5280 AIR trace builder.
    match rebound.der_air_trace() {
        Ok(trace) => {
            assert!(admitted, "{context}: DER AIR accepted");
            assert_numeric_rows_bind_every_certificate(&trace, certificates, crl, presentation);
        }
        Err(error) => {
            assert!(!admitted, "{context}: DER AIR rejected with {error:?}");
            assert_eq!(error, ZkX509DerAirErrorV1::Input, "{context}");
        }
    }
    // In-relation numeric rows: a rejected window has no satisfying slack, even
    // when it is substituted behind the DER AIR preflight.
    if let (false, Some(admitted_trace)) = (admitted, admitted_trace) {
        let mut substituted = admitted_trace.clone();
        substituted.statement.presentation_not_before_unix_seconds =
            presentation.not_before_unix_seconds;
        substituted.statement.presentation_not_after_unix_seconds =
            presentation.not_after_unix_seconds;
        if presentation.validate().is_ok() {
            // A well-shaped window outside a signed interval has a negative
            // slack in at least one numeric relation.
            assert!(
                matches!(
                    build_zk_x509_rfc5280_semantic_witness_v1(&substituted),
                    Err(ZkX509Rfc5280StarkErrorV1::Semantic)
                ),
                "{context}: in-relation rows admitted a rejected window"
            );
        } else {
            // A malformed window never reaches the rows: the verifier-owned
            // public shape rejects it.
            assert_eq!(
                ZkX509Rfc5280StarkShapeV1::from_statement(&substituted.statement).map(|_| ()),
                Err(ZkX509Rfc5280StarkErrorV1::Shape),
                "{context}: public shape admitted a malformed window"
            );
        }
        assert!(substituted.validate().is_err(), "{context}: path rows");
    }
    // Authoritative state sees the public window and the governed CRL only.
    let crl_admits = PrivacyZkX509PresentationBoundsV1::from_crl(crl)
        .and_then(|bounds| bounds.admit(presentation))
        .is_ok();
    assert!(
        crl_admits || !admitted,
        "{context}: CRL bounds are necessary"
    );
    if let Ok(first_ms) = presentation.first_admissible_block_timestamp_ms()
        && presentation.validate().is_ok()
    {
        let last_ms = presentation
            .last_admissible_block_timestamp_ms()
            .expect("window end fits milliseconds");
        for (timestamp_ms, inside) in [
            (first_ms - 1, false),
            (first_ms, true),
            (last_ms, true),
            (last_ms + 1, false),
        ] {
            assert_eq!(
                rebound.state_admission(timestamp_ms).is_ok(),
                crl_admits && inside,
                "{context}: state admission at {timestamp_ms}"
            );
            // Prover preflight is state admission and the complete relation.
            assert_eq!(
                rebound.prover_preflight(timestamp_ms).is_ok(),
                admitted && inside,
                "{context}: prover preflight at {timestamp_ms}"
            );
        }
    } else {
        assert!(
            rebound
                .state_admission(presentation.not_before_unix_seconds.saturating_mul(1_000))
                .is_err(),
            "{context}: malformed window reached state admission"
        );
    }
    admitted
}

fn field<'a>(value: &'a Value, name: &str) -> &'a Value {
    value
        .get(name)
        .unwrap_or_else(|| panic!("vector field `{name}` is missing"))
}

fn number(value: &Value, name: &str) -> u64 {
    field(value, name)
        .as_u64()
        .unwrap_or_else(|| panic!("vector field `{name}` is not an unsigned integer"))
}

fn rows<'a>(value: &'a Value, name: &str) -> &'a [Value] {
    field(value, name)
        .as_array()
        .unwrap_or_else(|| panic!("vector field `{name}` is not an array"))
}

#[test]
fn shared_interval_vectors_agree_across_builder_native_state_der_air_and_numeric_rows() {
    let document: Value = json::from_str(VECTORS).expect("tracked interval vectors");
    let mut signed_cases = 0_usize;
    let mut admitted_windows = 0_usize;
    let mut rejected_windows = 0_usize;
    let mut substituted_rejections = 0_usize;
    let mut deadline_rows = 0_usize;
    let mut deadlines_at_preflight = 0_usize;
    let mut deadlines_without_window = 0_usize;
    for case in rows(&document, "cases") {
        let name = field(case, "name").as_str().expect("case name");
        if !field(case, "signable").as_bool().expect("signable flag") {
            // Depth and reversed-interval inputs cannot be signed under the
            // closed profile: the strict parsers reject them before any
            // interval predicate. The data-model tests cover those rows.
            continue;
        }
        let certificates: Vec<_> = rows(case, "certificates")
            .iter()
            .map(|row| validity(number(row, "not_before"), number(row, "not_after")))
            .collect();
        let crl = PrivacyZkX509CrlUpdateIntervalV1::new(
            number(field(case, "crl"), "this_update"),
            number(field(case, "crl"), "next_update"),
        );
        let fixture = signed_fixture(&certificates, crl);
        // The DER-to-bounds helper reproduces the vector bounds from the DER.
        let derived = derive_zk_x509_presentation_bounds_v1(
            &fixture.witness.certificate_chain_der,
            &fixture.witness.crl_der,
        );
        match case.get("bounds") {
            Some(expected) => {
                let bounds = derived.unwrap_or_else(|error| panic!("{name}: {error}"));
                assert_eq!(
                    bounds.earliest_start_unix_seconds(),
                    number(expected, "earliest_start"),
                    "{name}"
                );
                assert_eq!(
                    bounds.latest_end_unix_seconds(),
                    number(expected, "latest_end"),
                    "{name}"
                );
            }
            None => assert_eq!(
                derived.map(|_| ()),
                Err(ZkX509RelationErrorV1::CertificateNotValid),
                "{name}: disjoint signed intervals"
            ),
        }
        let windows: Vec<(PrivacyZkX509PresentationWindowV1, bool)> = rows(case, "windows")
            .iter()
            .map(|row| {
                (
                    window(number(row, "not_before"), number(row, "not_after")),
                    field(row, "result").as_str() == Some("admitted"),
                )
            })
            .collect();
        let admitted_trace =
            windows
                .iter()
                .find(|(_, admitted)| *admitted)
                .map(|(presentation, _)| {
                    der_air_trace(&fixture, *presentation).expect("admitted vector window trace")
                });
        for (presentation, expected) in windows {
            let admitted = assert_all_layers_agree(
                name,
                &fixture,
                &certificates,
                crl,
                presentation,
                admitted_trace.as_ref(),
            );
            assert_eq!(admitted, expected, "{name}: vector expectation");
            if admitted {
                admitted_windows += 1;
            } else {
                rejected_windows += 1;
                substituted_rejections += usize::from(admitted_trace.is_some());
            }
        }
        // Holder deadlines: for the window that ends at the budgeted second,
        // the block at the deadline is admitted and the next millisecond is
        // not.
        for row in rows(case, "deadlines") {
            let bounds = derived.expect("deadline vectors carry bounds");
            let presentation_not_after = number(row, "presentation_not_after");
            let deadline_ms = bounds
                .presentation_deadline_unix_ms(presentation_not_after)
                .expect("deadline fits milliseconds");
            assert_eq!(deadline_ms, number(row, "deadline_unix_ms"), "{name}");
            deadline_rows += 1;
            // The latest admissible end not after the requested one, and the
            // earliest start that keeps the window inside the width ceiling.
            let end = presentation_not_after.min(bounds.latest_end_unix_seconds());
            let start = bounds
                .earliest_start_unix_seconds()
                .max(end.saturating_sub(ZK_X509_MAX_PRESENTATION_WINDOW_SECONDS_V1));
            let built = window(start, end);
            if bounds.admit(built).is_err() {
                // Only signed intervals that share a single second have no
                // window to present; the deadline is still that second.
                assert_eq!(
                    bounds.earliest_start_unix_seconds(),
                    bounds.latest_end_unix_seconds(),
                    "{name}: deadline row {presentation_not_after} has no window"
                );
                assert!(
                    bounds
                        .widest_window_from(bounds.earliest_start_unix_seconds())
                        .is_err(),
                    "{name}"
                );
                deadlines_without_window += 1;
                continue;
            }
            assert_eq!(
                built.last_admissible_block_timestamp_ms(),
                Ok(deadline_ms),
                "{name}: the window deadline is the budgeted deadline"
            );
            assert!(
                prover_preflight(&fixture, built, deadline_ms).is_ok(),
                "{name}: [{start}, {end}] at the deadline"
            );
            assert!(
                prover_preflight(&fixture, built, deadline_ms + 1).is_err(),
                "{name}: [{start}, {end}] one millisecond after the deadline"
            );
            deadlines_at_preflight += 1;
        }
        signed_cases += 1;
    }
    // Every signable vector window reached every layer: nothing was skipped.
    let signable_windows = |admitted: bool| {
        rows(&document, "cases")
            .iter()
            .filter(|case| field(case, "signable").as_bool() == Some(true))
            .flat_map(|case| rows(case, "windows"))
            .filter(|row| (field(row, "result").as_str() == Some("admitted")) == admitted)
            .count()
    };
    assert_eq!(admitted_windows, signable_windows(true));
    assert_eq!(rejected_windows, signable_windows(false));
    assert!(signed_cases >= 16, "signed {signed_cases} vector cases");
    assert!(
        admitted_windows >= 25,
        "admitted {admitted_windows} windows"
    );
    assert!(
        rejected_windows >= 42,
        "rejected {rejected_windows} windows"
    );
    assert!(
        substituted_rejections >= 25,
        "{substituted_rejections} rejected windows reached the in-relation rows"
    );
    // Every signable deadline row was checked, and all but the one
    // single-shared-second row reached prover preflight on both sides of the
    // deadline.
    let signable_deadline_rows = rows(&document, "cases")
        .iter()
        .filter(|case| field(case, "signable").as_bool() == Some(true))
        .map(|case| rows(case, "deadlines").len())
        .sum::<usize>();
    assert_eq!(deadline_rows, signable_deadline_rows);
    assert_eq!(deadlines_without_window, 1);
    assert_eq!(
        deadlines_at_preflight + deadlines_without_window,
        deadline_rows
    );
    assert!(
        deadlines_at_preflight >= 22,
        "{deadlines_at_preflight} deadline rows reached prover preflight"
    );
}

/// Earliest expiry at the leaf, at the intermediate and at the root, each with
/// boundary equality, the one-second overflow on both sides, and the window the
/// retired latest-expiry formula would have built.
#[test]
fn earliest_expiry_at_leaf_intermediate_and_root_binds_every_verifier_layer() {
    let earliest_expiry = T + 150;
    let later_expiry = T + 86_400;
    let cases: [(&str, [PrivacyZkX509CertificateValidityV1; 3]); 3] = [
        (
            "leaf",
            [
                validity(T - 3_600, earliest_expiry),
                validity(T - 7_200, later_expiry),
                wide(),
            ],
        ),
        (
            "intermediate",
            [
                validity(T - 3_600, later_expiry),
                validity(T - 7_200, earliest_expiry),
                wide(),
            ],
        ),
        (
            "root",
            [
                validity(T - 3_600, later_expiry),
                wide(),
                validity(T - 7_200, earliest_expiry),
            ],
        ),
    ];
    let crl = fresh_crl();
    for (label, certificates) in cases {
        let fixture = signed_fixture(&certificates, crl);
        let bounds = derive_zk_x509_presentation_bounds_v1(
            &fixture.witness.certificate_chain_der,
            &fixture.witness.crl_der,
        )
        .expect("overlapping signed intervals");
        assert_eq!(bounds.earliest_start_unix_seconds(), T, "{label}");
        assert_eq!(bounds.latest_end_unix_seconds(), earliest_expiry, "{label}");
        // The retired formula took the latest expiry in the path.
        let latest_expiry = certificates
            .iter()
            .map(|certificate| certificate.not_after_unix_seconds())
            .max()
            .expect("non-empty path");
        assert!(latest_expiry > earliest_expiry + ZK_X509_MAX_CRL_AGE_SECONDS_V1);
        let retired = window(
            T,
            (T + ZK_X509_MAX_CRL_AGE_SECONDS_V1)
                .min(crl.next_update_unix_seconds - 1)
                .min(latest_expiry),
        );
        assert_eq!(retired, window(T, T + 300));
        // The builder ends at the earliest expiry instead.
        let built = bounds
            .widest_window_from(T)
            .expect("admissible builder window");
        assert_eq!(built, window(T, earliest_expiry), "{label}");
        let admitted_trace = der_air_trace(&fixture, built).expect("builder window trace");
        build_zk_x509_rfc5280_stark_base_material_v1(&admitted_trace)
            .expect("complete native RFC constructor admits the builder window");
        for (presentation, expected) in [
            // Boundary equality on the binding certificate.
            (built, true),
            (window(earliest_expiry - 1, earliest_expiry), true),
            // One second past the earliest expiry, although the leaf (or the
            // other CA) and the CRL are still valid.
            (window(T, earliest_expiry + 1), false),
            (window(earliest_expiry, earliest_expiry + 1), false),
            // The retired latest-expiry window: an expired credential.
            (retired, false),
            // One second before the CRL thisUpdate.
            (window(T - 1, earliest_expiry), false),
        ] {
            assert_eq!(
                assert_all_layers_agree(
                    label,
                    &fixture,
                    &certificates,
                    crl,
                    presentation,
                    Some(&admitted_trace),
                ),
                expected,
                "{label}: [{}, {}]",
                presentation.not_before_unix_seconds,
                presentation.not_after_unix_seconds
            );
        }
        // The retired window is rejected for the certificate, not the CRL.
        assert_eq!(
            native_relation(&fixture, retired),
            Err(ZkX509RelationErrorV1::CertificateNotValid),
            "{label}"
        );
        assert!(state_admission(&fixture, retired, T * 1_000).is_ok());
        assert!(matches!(
            prover_preflight(&fixture, retired, T * 1_000),
            Err(ZkX509EngineErrorV1::ReferenceRelation(
                ZkX509RelationErrorV1::CertificateNotValid
            ))
        ));
        // Holder deadline: the block second Cmin is accepted and Cmin + 1 is
        // rejected; the budgeted deadline is Cmin, not the latest expiry.
        let deadline_ms = bounds
            .presentation_deadline_unix_ms(retired.not_after_unix_seconds)
            .expect("deadline fits milliseconds");
        assert_eq!(deadline_ms, (earliest_expiry + 1) * 1_000 - 1, "{label}");
        assert_eq!(
            built.last_admissible_block_timestamp_ms(),
            Ok(deadline_ms),
            "{label}"
        );
        assert!(prover_preflight(&fixture, built, earliest_expiry * 1_000).is_ok());
        assert!(prover_preflight(&fixture, built, deadline_ms).is_ok());
        assert!(prover_preflight(&fixture, built, (earliest_expiry + 1) * 1_000).is_err());
        let retired_deadline_ms = (retired.not_after_unix_seconds + 1) * 1_000 - 1;
        assert!(retired_deadline_ms > deadline_ms);
        assert!(prover_preflight(&fixture, built, retired_deadline_ms).is_err());
    }
}

/// Latest `notBefore` at each path position, boundary equality and the
/// one-second underflow.
#[test]
fn latest_not_before_at_leaf_intermediate_and_root_binds_every_verifier_layer() {
    let latest_start = T + 40;
    let cases: [(&str, [PrivacyZkX509CertificateValidityV1; 3]); 3] = [
        (
            "leaf",
            [
                validity(latest_start, wide().not_after_unix_seconds()),
                validity(T - 7_200, wide().not_after_unix_seconds()),
                wide(),
            ],
        ),
        (
            "intermediate",
            [
                validity(T - 3_600, wide().not_after_unix_seconds()),
                validity(latest_start, wide().not_after_unix_seconds()),
                wide(),
            ],
        ),
        (
            "root",
            [
                validity(T - 3_600, wide().not_after_unix_seconds()),
                wide(),
                validity(latest_start, wide().not_after_unix_seconds()),
            ],
        ),
    ];
    let crl = fresh_crl();
    for (label, certificates) in cases {
        let fixture = signed_fixture(&certificates, crl);
        let bounds = derive_zk_x509_presentation_bounds_v1(
            &fixture.witness.certificate_chain_der,
            &fixture.witness.crl_der,
        )
        .expect("overlapping signed intervals");
        assert_eq!(
            bounds.earliest_start_unix_seconds(),
            latest_start,
            "{label}"
        );
        assert_eq!(bounds.latest_end_unix_seconds(), T + 300, "{label}");
        assert_eq!(
            bounds.widest_window_from(T),
            Err(PrivacyZkX509PresentationIntervalErrorV1::StartsBeforeBounds),
            "{label}: the builder refuses a start before the latest notBefore"
        );
        let built = bounds
            .widest_window_from(latest_start)
            .expect("admissible builder window");
        assert_eq!(built, window(latest_start, T + 300), "{label}");
        let admitted_trace = der_air_trace(&fixture, built).expect("builder window trace");
        for (presentation, expected) in [
            (built, true),
            (window(latest_start, latest_start + 1), true),
            (window(latest_start - 1, T + 300), false),
            (window(latest_start - 1, latest_start), false),
            // The CRL alone would admit this start.
            (window(T, T + 300), false),
        ] {
            assert_eq!(
                assert_all_layers_agree(
                    label,
                    &fixture,
                    &certificates,
                    crl,
                    presentation,
                    Some(&admitted_trace),
                ),
                expected,
                "{label}: [{}, {}]",
                presentation.not_before_unix_seconds,
                presentation.not_after_unix_seconds
            );
        }
        assert_eq!(
            native_relation(&fixture, window(latest_start - 1, T + 300)),
            Err(ZkX509RelationErrorV1::CertificateNotValid),
            "{label}"
        );
    }
}

/// CRL `nextUpdate` is exclusive and `thisUpdate` is inclusive; the age cap is
/// exactly 300 seconds. All other revocation predicates are untouched.
#[test]
fn crl_next_update_is_excluded_and_freshness_predicates_are_preserved() {
    let certificates = [wide(), wide()];
    // nextUpdate binds before the age cap.
    let short_crl = PrivacyZkX509CrlUpdateIntervalV1::new(T, T + 200);
    let fixture = signed_fixture(&certificates, short_crl);
    let bounds = derive_zk_x509_presentation_bounds_v1(
        &fixture.witness.certificate_chain_der,
        &fixture.witness.crl_der,
    )
    .expect("overlapping signed intervals");
    assert_eq!(bounds.latest_end_unix_seconds(), T + 199);
    let built = bounds
        .widest_window_from(T)
        .expect("admissible builder window");
    assert_eq!(built, window(T, T + 199));
    let admitted_trace = der_air_trace(&fixture, built).expect("builder window trace");
    for (presentation, expected) in [
        (built, true),
        (window(T + 198, T + 199), true),
        // Equality with nextUpdate rejects.
        (window(T, T + 200), false),
        (window(T + 199, T + 200), false),
        (window(T - 1, T + 199), false),
    ] {
        assert_eq!(
            assert_all_layers_agree(
                "nextUpdate",
                &fixture,
                &certificates,
                short_crl,
                presentation,
                Some(&admitted_trace),
            ),
            expected
        );
    }
    assert_eq!(
        native_relation(&fixture, window(T, T + 200)),
        Err(ZkX509RelationErrorV1::InvalidCrl)
    );
    assert!(
        state_admission(&fixture, window(T, T + 200), T * 1_000)
            .expect_err("a window ending at nextUpdate must reject")
            .contains("signed-CRL freshness window")
    );
    // The age cap binds before nextUpdate.
    let long_crl = PrivacyZkX509CrlUpdateIntervalV1::new(T, T + 3_600);
    let fixture = signed_fixture(&certificates, long_crl);
    let bounds = derive_zk_x509_presentation_bounds_v1(
        &fixture.witness.certificate_chain_der,
        &fixture.witness.crl_der,
    )
    .expect("overlapping signed intervals");
    assert_eq!(
        bounds.latest_end_unix_seconds(),
        T + ZK_X509_MAX_CRL_AGE_SECONDS_V1
    );
    let built = bounds
        .widest_window_from(T)
        .expect("admissible builder window");
    let admitted_trace = der_air_trace(&fixture, built).expect("builder window trace");
    for (presentation, expected) in [
        (built, true),
        (window(T + 1, T + 300), true),
        (window(T + 1, T + 301), false),
        (window(T + 200, T + 301), false),
        (window(T, T + 301), false),
    ] {
        assert_eq!(
            assert_all_layers_agree(
                "age",
                &fixture,
                &certificates,
                long_crl,
                presentation,
                Some(&admitted_trace),
            ),
            expected
        );
    }
    assert_eq!(
        native_relation(&fixture, window(T + 1, T + 301)),
        Err(ZkX509RelationErrorV1::InvalidCrl)
    );
    // Revocation still takes precedence over a fully covered window.
    let revoked = build_zk_x509_fixture_v1(
        reference_statement_context_v1(),
        false,
        &[CANONICAL_LEAF_SERIAL_V1.to_vec()],
        ZkX509ReleaseTimesV1 {
            presentation: ZkX509ReleasePresentationV1::Unchecked(window(
                VALIDATION_TIME,
                VALIDATION_TIME + 60,
            )),
            ..ZkX509ReleaseTimesV1::FIXED_V1
        },
        fixed_release_wallet_account_v1().expect("wallet"),
        &ZkX509ReleaseCrlLineageV1::Origin,
    )
    .expect("signed fixture whose CRL revokes the leaf");
    assert_eq!(
        native_relation(&revoked, window(VALIDATION_TIME, VALIDATION_TIME + 60)),
        Err(ZkX509RelationErrorV1::CertificateRevoked)
    );
}

/// Signed intervals that never overlap admit nothing, and an expired
/// credential is rejected by every layer.
#[test]
fn expired_or_disjoint_signed_intervals_admit_no_window() {
    let crl = fresh_crl();
    for (label, certificates) in [
        // The leaf expired one second before the CRL was issued.
        ("expired leaf", vec![validity(T - 86_400, T - 1), wide()]),
        // The intermediate expired before the leaf became valid.
        (
            "disjoint path",
            vec![
                validity(T + 200, T + 400),
                validity(T - 86_400, T + 100),
                wide(),
            ],
        ),
        // The root expired before the CRL was issued.
        (
            "expired root",
            vec![wide(), wide(), validity(T - 86_400, T - 1)],
        ),
    ] {
        let fixture = signed_fixture(&certificates, crl);
        assert_eq!(
            derive_zk_x509_presentation_bounds_v1(
                &fixture.witness.certificate_chain_der,
                &fixture.witness.crl_der,
            )
            .map(|_| ()),
            Err(ZkX509RelationErrorV1::CertificateNotValid),
            "{label}: the builder has no window to offer"
        );
        for presentation in [
            window(T, T + 1),
            window(T, T + 300),
            window(T + 100, T + 200),
            window(T + 200, T + 300),
            window(T - 2, T - 1),
        ] {
            assert!(
                !assert_all_layers_agree(label, &fixture, &certificates, crl, presentation, None),
                "{label}"
            );
        }
        // Inside the CRL freshness window the rejection is the certificate's.
        assert_eq!(
            native_relation(&fixture, window(T, T + 1)),
            Err(ZkX509RelationErrorV1::CertificateNotValid),
            "{label}"
        );
    }
}

/// The reference relation rejects a malformed public window on its own; it no
/// longer relies on an earlier statement check.
#[test]
fn native_relation_rejects_empty_reversed_and_over_wide_windows() {
    let certificates = [wide(), wide()];
    let crl = PrivacyZkX509CrlUpdateIntervalV1::new(T, T + 3_600);
    let fixture = signed_fixture(&certificates, crl);
    assert!(native_relation(&fixture, window(T + 10, T + 11)).is_ok());
    for presentation in [
        window(T + 10, T + 10),
        window(T + 10, T + 9),
        window(T, T + ZK_X509_MAX_PRESENTATION_WINDOW_SECONDS_V1 + 1),
    ] {
        assert_eq!(
            native_relation(&fixture, presentation),
            Err(ZkX509RelationErrorV1::CertificateNotValid)
        );
        assert!(der_air_trace(&fixture, presentation).is_err());
    }
}

/// The release fixtures take their window from the canonical builder and keep
/// the fixed KAT bytes.
#[test]
fn release_fixture_windows_come_from_the_canonical_builder() {
    assert_eq!(
        validity_der_v1(FIXED_CERTIFICATE_VALIDITY_V1).expect("fixed validity"),
        sequence(&[tlv(0x17, b"220101000000Z"), tlv(0x17, b"300101000000Z")]),
        "the fixed KAT validity bytes are unchanged"
    );
    assert_eq!(
        time_tlv_v1(2_524_607_999).expect("last UTCTime second"),
        tlv(0x17, b"491231235959Z")
    );
    assert_eq!(
        time_tlv_v1(2_524_608_000).expect("first GeneralizedTime second"),
        tlv(0x18, b"20500101000000Z")
    );
    assert!(time_tlv_v1(253_402_300_800).is_err());
    for (unix_seconds, utc_time) in [
        (CRL_THIS_UPDATE, b"230101000000Z"),
        (CRL_NEXT_UPDATE, b"230101000500Z"),
        (CRL_THIS_UPDATE - 86_400, b"221231000000Z"),
    ] {
        assert_eq!(
            time_tlv_v1(unix_seconds).expect("fixed CRL time"),
            tlv(0x17, utc_time)
        );
    }
    let fixed = build_zk_x509_reference_fixture_v1().expect("fixed KAT fixture");
    // The reference-relation tests encode the same two-certificate fixture
    // from literal DER time strings. Selecting the window through the builder
    // and encoding times from seconds leaves every signed byte, the statement
    // and the ownership signature identical.
    let literal = super::super::tests::fixture();
    assert_eq!(
        fixed.witness.certificate_chain_der,
        literal.witness.certificate_chain_der
    );
    assert_eq!(fixed.witness.crl_der, literal.witness.crl_der);
    assert_eq!(fixed.statement, literal.statement);
    assert_eq!(fixed.witness, literal.witness);
    let bounds = derive_zk_x509_presentation_bounds_v1(
        &fixed.witness.certificate_chain_der,
        &fixed.witness.crl_der,
    )
    .expect("fixed KAT bounds");
    assert_eq!(bounds.earliest_start_unix_seconds(), CRL_THIS_UPDATE);
    assert_eq!(bounds.latest_end_unix_seconds(), CRL_NEXT_UPDATE - 1);
    assert_eq!(
        fixed.statement.presentation_window(),
        window(VALIDATION_TIME, VALIDATION_TIME + 60)
    );
    assert_eq!(bounds.admit(fixed.statement.presentation_window()), Ok(()));
    // A fixed window outside the signed intervals is refused by the builder.
    for presentation in [
        window(VALIDATION_TIME, CRL_NEXT_UPDATE),
        window(CRL_THIS_UPDATE - 1, VALIDATION_TIME),
    ] {
        assert_eq!(
            build_zk_x509_fixture_v1(
                reference_statement_context_v1(),
                false,
                &[],
                ZkX509ReleaseTimesV1 {
                    presentation: ZkX509ReleasePresentationV1::Exact(presentation),
                    ..ZkX509ReleaseTimesV1::FIXED_V1
                },
                fixed_release_wallet_account_v1().expect("wallet"),
                &ZkX509ReleaseCrlLineageV1::Origin,
            )
            .map(|_| ()),
            Err("deterministic release window is outside the signed intervals")
        );
    }
    // The network fixture is the widest window from the trusted second.
    let trusted_block_timestamp_ms = 1_785_024_000_123;
    let network = build_zk_x509_network_release_fixture_v1(
        reference_statement_context_v1(),
        trusted_block_timestamp_ms,
        fixed_release_wallet_account_v1().expect("wallet"),
    )
    .expect("network fixture");
    let bounds = derive_zk_x509_presentation_bounds_v1(
        &network.witness.certificate_chain_der,
        &network.witness.crl_der,
    )
    .expect("network bounds");
    let trusted_unix_seconds = trusted_block_timestamp_ms / 1_000;
    assert_eq!(
        Ok(network.statement.presentation_window()),
        bounds.widest_window_from(trusted_unix_seconds)
    );
    assert_eq!(
        network.statement.presentation_window(),
        window(
            trusted_unix_seconds,
            trusted_unix_seconds + ZK_X509_MAX_CRL_AGE_SECONDS_V1
        )
    );
    // A trusted second after the signed intervals has no admissible window.
    assert_eq!(
        build_zk_x509_fixture_v1(
            reference_statement_context_v1(),
            false,
            &[],
            ZkX509ReleaseTimesV1 {
                presentation: ZkX509ReleasePresentationV1::WidestFrom(CRL_NEXT_UPDATE - 1),
                ..ZkX509ReleaseTimesV1::FIXED_V1
            },
            fixed_release_wallet_account_v1().expect("wallet"),
            &ZkX509ReleaseCrlLineageV1::Origin,
        )
        .map(|_| ()),
        Err("deterministic release start is outside the signed intervals")
    );
    // Rebinding the window changes only the ownership signature.
    let (statement, witness) =
        rewindow_zk_x509_fixture_v1(&fixed, fixed.statement.presentation_window())
            .expect("identity rebinding");
    assert_eq!(statement, fixed.statement);
    assert_eq!(witness, fixed.witness);
    let (_, rebound) =
        rewindow_zk_x509_fixture_v1(&fixed, window(VALIDATION_TIME, VALIDATION_TIME + 1))
            .expect("rebound window");
    assert_ne!(
        rebound.wallet_ownership_signature_rs,
        fixed.witness.wallet_ownership_signature_rs
    );
    assert_eq!(
        rebound.certificate_chain_der,
        fixed.witness.certificate_chain_der
    );
    assert_eq!(rebound.crl_der, fixed.witness.crl_der);
    // The interval fixture rejects a path outside the closed depth.
    assert!(build_zk_x509_interval_fixture_v1(&[wide()], fresh_crl(), window(T, T + 1)).is_err());
    assert!(
        build_zk_x509_interval_fixture_v1(&[wide(); 4], fresh_crl(), window(T, T + 1)).is_err()
    );
}

/// Parsing failures surface from the builder instead of a default window.
#[test]
fn bounds_builder_rejects_malformed_der_and_depth() {
    let fixed = build_zk_x509_reference_fixture_v1().expect("fixed KAT fixture");
    let chain = &fixed.witness.certificate_chain_der;
    assert_eq!(
        derive_zk_x509_presentation_bounds_v1(&chain[..1], &fixed.witness.crl_der).map(|_| ()),
        Err(ZkX509RelationErrorV1::WitnessMismatch)
    );
    let four = vec![chain[0].clone(); 4];
    assert_eq!(
        derive_zk_x509_presentation_bounds_v1(&four, &fixed.witness.crl_der).map(|_| ()),
        Err(ZkX509RelationErrorV1::WitnessMismatch)
    );
    let mut truncated = chain.clone();
    truncated[1].pop();
    assert!(derive_zk_x509_presentation_bounds_v1(&truncated, &fixed.witness.crl_der).is_err());
    assert!(derive_zk_x509_presentation_bounds_v1(chain, &fixed.witness.crl_der[..10]).is_err());
}
