import XCTest
@testable import IrohaSwift

final class VerifyingKeyBackendTagTests: XCTestCase {
    func testNoritoDiscriminantsPreserveRustOrder() {
        let expected: [(VerifyingKeyBackendTag, UInt32)] = [
            (.halo2IpaPasta, 0),
            (.halo2Bn254, 1),
            (.groth16, 2),
            (.stark, 3),
            (.unsupported, 4),
            (.halo2IpaOrchard, 5),
            (.groth16Bls12377, 6),
            (.fcmpPlusPlusCurveTree, 7),
            (.latticePcsSis, 8),
            (.midenStark, 9),
            (.aztecPlonkishPrivateKernel, 10),
            (.pqMaspStarkFri, 11),
            (.anonymousPgc, 12),
            (.veRange, 13),
            (.zkAt, 14),
            (.recursiveAnonymousAdmission, 15),
            (.vegaExistingCredentialZk, 16),
            (.silentThresholdAnoncred, 17),
            (.zkX509, 18),
            (.sisWithHints, 19)
        ]

        for (backend, tag) in expected {
            XCTAssertEqual(backend.noritoDiscriminant, tag)
            XCTAssertEqual(VerifyingKeyBackendTag(rawValue: tag), backend)
        }
    }

    func testCanonicalLabelsMatchRustCatalog() {
        XCTAssertEqual(VerifyingKeyBackendTag.halo2IpaPasta.canonicalLabel, "halo2-ipa-pasta")
        XCTAssertEqual(VerifyingKeyBackendTag.halo2Bn254.canonicalLabel, "halo2-bn254")
        XCTAssertEqual(VerifyingKeyBackendTag.groth16.canonicalLabel, "groth16")
        XCTAssertEqual(VerifyingKeyBackendTag.stark.canonicalLabel, "stark")
        XCTAssertEqual(VerifyingKeyBackendTag.unsupported.canonicalLabel, "unsupported")
        XCTAssertEqual(VerifyingKeyBackendTag.halo2IpaOrchard.canonicalLabel, "halo2-ipa-orchard")
        XCTAssertEqual(VerifyingKeyBackendTag.groth16Bls12377.canonicalLabel, "groth16-bls12-377")
        XCTAssertEqual(VerifyingKeyBackendTag.fcmpPlusPlusCurveTree.canonicalLabel, "fcmp-plus-plus-curve-tree")
        XCTAssertEqual(VerifyingKeyBackendTag.latticePcsSis.canonicalLabel, "lattice-pcs-sis")
        XCTAssertEqual(VerifyingKeyBackendTag.midenStark.canonicalLabel, "miden-stark")
        XCTAssertEqual(VerifyingKeyBackendTag.aztecPlonkishPrivateKernel.canonicalLabel, "aztec-plonkish-private-kernel")
        XCTAssertEqual(VerifyingKeyBackendTag.pqMaspStarkFri.canonicalLabel, "pq-masp-stark-fri")
        XCTAssertEqual(VerifyingKeyBackendTag.anonymousPgc.canonicalLabel, "anonymous-pgc")
        XCTAssertEqual(VerifyingKeyBackendTag.veRange.canonicalLabel, "verange")
        XCTAssertEqual(VerifyingKeyBackendTag.zkAt.canonicalLabel, "zkat")
        XCTAssertEqual(VerifyingKeyBackendTag.recursiveAnonymousAdmission.canonicalLabel, "recursive-anonymous-admission")
        XCTAssertEqual(VerifyingKeyBackendTag.vegaExistingCredentialZk.canonicalLabel, "vega-existing-credential-zk")
        XCTAssertEqual(VerifyingKeyBackendTag.silentThresholdAnoncred.canonicalLabel, "silent-threshold-anoncred")
        XCTAssertEqual(VerifyingKeyBackendTag.zkX509.canonicalLabel, "zk-x509")
        XCTAssertEqual(VerifyingKeyBackendTag.sisWithHints.canonicalLabel, "sis-with-hints")
    }

    func testPendingProductionAliasesRemainFailClosed() {
        let aliases: [(String, VerifyingKeyBackendTag)] = [
            ("halo2-ipa-orchard", .halo2IpaOrchard),
            ("halo2/ipa/orchard", .halo2IpaOrchard),
            ("orchard", .halo2IpaOrchard),
            ("anonymous-pgc", .anonymousPgc),
            ("anonymous-pgc-k-out-of-n", .anonymousPgc),
            ("verange-transparent-range", .veRange),
            ("zkAt policy-private authenticator", .zkAt),
            ("recursive-anonymous-admission", .recursiveAnonymousAdmission),
            ("zk-ams-recursive-admission-v0", .recursiveAnonymousAdmission),
            ("vega-existing-credential-zk", .vegaExistingCredentialZk),
            ("threshold-anonymous-credentials", .silentThresholdAnoncred),
            ("silent-threshold-anoncred", .silentThresholdAnoncred),
            ("zkvm-x509-identity", .zkX509),
            ("zk-x509-onchain-identity-v0", .zkX509),
            ("sis-with-hints", .sisWithHints),
            ("lattice-anonymous-credentials", .sisWithHints),
            ("groth16-bls12-377", .groth16Bls12377),
            ("groth16/bls12-377", .groth16Bls12377),
            ("penumbra-masp", .groth16Bls12377),
            ("halo2/ipa/penumbra", .groth16Bls12377),
            ("halo2/ipa/masp", .groth16Bls12377),
            ("monero-fcmp++", .fcmpPlusPlusCurveTree),
            ("fcmp++", .fcmpPlusPlusCurveTree),
            ("fcmp-plus-plus-curve-tree", .fcmpPlusPlusCurveTree),
            ("halo2/ipa/monero", .fcmpPlusPlusCurveTree),
            ("halo2/ipa/curve-tree", .fcmpPlusPlusCurveTree),
            ("lattice-pcs-sis", .latticePcsSis),
            ("jindo-lattice-pcs-zk", .latticePcsSis),
            ("miden-stark", .midenStark),
            ("aztec-plonkish-private-kernel", .aztecPlonkishPrivateKernel),
            ("pq-masp-stark-fri", .pqMaspStarkFri),
            ("post-quantum-masp", .pqMaspStarkFri)
        ]

        for (label, expected) in aliases {
            let parsed = VerifyingKeyBackendTag(catalogLabel: label)
            XCTAssertEqual(parsed, expected, "\(label) must parse to the exact pending backend")
            XCTAssertTrue(parsed.isPendingProductionBackend, "\(label) must remain fail-closed")
            XCTAssertTrue(VerifyingKeyBackendTag.isPendingProductionBackendLabel(label))
        }
    }

    func testSupportedLegacyFamiliesDoNotBecomePending() {
        let aliases: [(String, VerifyingKeyBackendTag)] = [
            ("halo2-ipa-pasta", .halo2IpaPasta),
            ("halo2/ipa", .halo2IpaPasta),
            ("halo2/pasta/ipa/vote-bool", .halo2IpaPasta),
            ("halo2-bn254", .halo2Bn254),
            ("halo2/bn254", .halo2Bn254),
            ("groth16", .groth16),
            ("groth16/bn254", .groth16),
            ("stark", .stark),
            ("stark/fri", .stark),
            ("stark/fri/sha256-goldilocks", .stark),
            ("stark/fri/poseidon2-goldilocks", .stark),
            ("stark/fri/sha256_goldilocks.v1", .stark),
            ("unknown/privacy/backend", .unsupported)
        ]

        for (label, expected) in aliases {
            let parsed = VerifyingKeyBackendTag(catalogLabel: label)
            XCTAssertEqual(parsed, expected, "\(label) must preserve legacy mapping")
            XCTAssertFalse(parsed.isPendingProductionBackend)
        }
    }

    func testCatalogAliasesRejectNonAsciiConfusablesBeforeCompaction() {
        for label in [
            "halo2\u{FF0F}ipa",
            "halo2/\u{200B}ipa",
            "h\u{0430}lo2/ipa",
            "stark\u{FF0F}fri/sha256-goldilocks",
            "stark/fri/\u{200B}sha256-goldilocks",
            "st\u{0430}rk/fri/sha256-goldilocks"
        ] {
            let parsed = VerifyingKeyBackendTag(catalogLabel: label)
            XCTAssertEqual(parsed, .unsupported)
            XCTAssertFalse(VerifyingKeyBackendTag.isPendingProductionBackendLabel(label))
        }
    }

    func testAdversarialPendingAliasSplicesStayUnsupported() {
        let aliases = [
            "halo2/ipa/orchard/dev-fixture",
            "stark/fri/miden/claimed-production",
            "anonymous-pgc-k-out-of-n-v1-production",
            "sis-hints-anoncred-pq-v0-devfixture",
            "groth16/bls12-377/../../prod",
            "post-quantum-masp/audit-claimed",
            "halo2/ipa/orchard:kzg",
            "orchard:universal-srs",
            "penumbra-masp:kzg",
            "jindo-lattice-pcs-zk:trusted-setup",
            "miden-stark:ptau",
            "sis-with-hints:groth16",
            "pq-masp-stark-fri:kzg"
        ]

        for label in aliases {
            let parsed = VerifyingKeyBackendTag(catalogLabel: label)
            XCTAssertEqual(parsed, .unsupported, "\(label) must stay unsupported")
            XCTAssertFalse(parsed.isPendingProductionBackend)
            XCTAssertFalse(VerifyingKeyBackendTag.isPendingProductionBackendLabel(label))
        }
    }

    func testProductionVerifierBackendClassifierMirrorsNativeAllowlist() throws {
        let supported = [
            "halo2/ipa",
            "halo2/ipa:ivm-execution-v1",
            "halo2/pasta/ivm-execution-v1",
            "halo2/pasta/kagemusha-folded-v1",
            "halo2/pasta/kaigi-roster-v1",
            "halo2/pasta/kagemusha-recursive-compact-v1",
            "halo2/pasta/anon-transfer-2x2-merkle16-poseidon-diversified",
            "stark/fri",
            "stark/fri/sha256-goldilocks",
            "stark/fri/poseidon2-goldilocks",
            "stark/fri/sha256_goldilocks.v1"
        ]

        for backend in supported {
            XCTAssertTrue(
                VerifyingKeyBackendTag.isProductionVerifyBackendLabel(backend),
                "\(backend) should be production-admissible"
            )
            XCTAssertEqual(
                try VerifyingKeyBackendTag.requireProductionVerifyBackendLabel(backend),
                backend
            )
        }
    }

    func testProductionVerifierBackendClassifierRejectsUnsafeLabels() {
        let unsupported = [
            "",
            "unknown/privacy/backend",
            "halo2/unknown-native-v1",
            "halo2/ipa:unknown-native-v1",
            "stark/unknown-native-v1",
            "halo2/bn254",
            "groth16",
            "groth16/bls12-377",
            " halo2/ipa",
            "halo2/ipa ",
            "\thalo2/ipa",
            "halo2/ipa\n",
            "halo2\u{FF0F}ipa",
            "halo2/\u{200B}ipa",
            "h\u{0430}lo2/ipa",
            "HALO2/IPA",
            "stark/FRI",
            "halo2/ipa::ivm-execution-v1",
            "halo2//ipa",
            "halo2/ipa:",
            "halo2/ipa.",
            "halo2/ipa/.ivm-execution-v1",
            "halo2/ipa:ivm..execution-v1",
            " stark/fri/sha256-goldilocks",
            "stark/fri/sha256-goldilocks ",
            "halo2/ipa/orchard",
            "halo2-ipa-orchard",
            "halo2/ipa/penumbra",
            "halo2/ipa/masp",
            "halo2/ipa/monero",
            "halo2/ipa/curve-tree",
            "halo2/pasta/tiny-add",
            "halo2/ipa/tiny-add",
            "halo2/ipa:tiny-add",
            "halo2/pasta/tiny-commit-open",
            "halo2/pasta/anon-transfer-2x2",
            "halo2/ipa/anon-transfer-2x2",
            "halo2/ipa:anon-transfer-2x2",
            "halo2/pasta/anon-transfer-2x2-merkle2",
            "halo2/ipa/anon-transfer-2x2-merkle8",
            "halo2/ipa:anon-transfer-2x2-merkle16",
            "halo2/pasta/vote-bool-commit",
            "halo2/ipa/vote-bool-commit",
            "halo2/ipa:vote-bool-commit",
            "halo2/pasta/vote-bool-commit-merkle2",
            "halo2/ipa/vote-bool-commit-merkle8",
            "halo2/ipa:vote-bool-commit-merkle16",
            "halo2/pasta/asset-hidden-transfer-public-test",
            "halo2/ipa/asset-hidden-transfer-public-test",
            "halo2/ipa:asset-hidden-transfer-public-test",
            "stark/fri/miden",
            "stark/fri/miden/claimed-production",
            "stark/fri/latest",
            "stark/fri/attestation",
            "stark/fri/contest",
            "stark/fri/random-profile",
            "stark/fri/sha512-goldilocks",
            "stark/fri/audit-proof-v1",
            "stark/fri/sha256 goldilocks",
            "stark/fri/sha256+goldilocks",
            "halo2/ipa+mock",
            "halo2/ipa:production-ready",
            "halo2/ipa:claimed-production",
            "halo2/ipa:mainnet-ready",
            "halo2/ipa:release-ready",
            "halo2/ipa:certified-mainnet",
            "halo2/ipa:third-party-audited",
            "halo2/ipa/orchard:production-ready",
            "orchard:mainnet-ready",
            "penumbra-masp:external-security-review",
            "jindo-lattice-pcs-zk:release-ready",
            "miden-stark:dev-fixture",
            "sis-with-hints:s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
            "halo2/ipa/orchard:kzg",
            "orchard:universal-srs",
            "penumbra-masp:kzg",
            "jindo-lattice-pcs-zk:trusted-setup",
            "miden-stark:ptau",
            "sis-with-hints:groth16",
            "pq-masp-stark-fri:kzg",
            "stark/fri/audit-signoff",
            "stark/fri/externally-audited",
            "stark/fri/boi-audited",
            "stark/fri/external-security-review",
            "stark/fri/security-review-passed",
            "stark/fri/S.e.c.u.r.i.t.yReviewPassed",
            "stark/fri/s-e-c-u-r-i-t-y-a-u-d-i-t-e-d",
            "stark/fri/a-u-d-i-t-c-l-a-i-m",
            "stark/fri/dev-fixture",
            "stark/fri/d-e-v-f-i-x-t-u-r-e",
            "stark/fri/dev",
            "stark/fri/d-e-v",
            "stark/fri/test",
            "stark/fri/t-e-s-t",
            "stark/fri/placeholder",
            "halo2/ipa:dev-fixture",
            "halo2/ipa:dev",
            "halo2/ipa:d-e-v",
            "halo2/ipa:dummy",
            "halo2/ipa:f-a-k-e",
            "halo2/ipa:stub",
            "halo2/ipa:s-a-m-p-l-e",
            "halo2/kzg",
            "halo2/pasta/mock",
            "halo2/pasta/debug-vote",
            "mock/dev",
            "kzg/powersoftau",
            "../halo2/ipa",
            "halo2/ipa\0"
        ]

        for backend in unsupported {
            XCTAssertFalse(
                VerifyingKeyBackendTag.isProductionVerifyBackendLabel(backend),
                "\(backend) should remain fail-closed"
            )
            XCTAssertThrowsError(
                try VerifyingKeyBackendTag.requireProductionVerifyBackendLabel(backend),
                "\(backend) should not pass production backend validation"
            ) { error in
                let trimmed = backend.trimmingCharacters(in: .whitespacesAndNewlines)
                let expected = trimmed.isEmpty
                    ? "must not be blank"
                    : trimmed == backend
                        ? "unsupported production verifier backend"
                        : "surrounding whitespace"
                let description = (error as? LocalizedError)?.errorDescription ?? String(describing: error)
                XCTAssertTrue(description.contains(expected), description)
            }
        }
    }
}
