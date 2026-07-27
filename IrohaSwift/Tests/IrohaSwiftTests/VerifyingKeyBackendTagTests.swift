import XCTest
@testable import IrohaSwift

final class VerifyingKeyBackendTagTests: XCTestCase {
    func testNoritoDiscriminantsMatchRustExactly() {
        let expected: [(VerifyingKeyBackendTag, UInt32)] = [
            (.halo2IpaPasta, 0),
            (.stark, 1)
        ]

        XCTAssertEqual(VerifyingKeyBackendTag.allCases.count, expected.count)
        for (backend, discriminant) in expected {
            XCTAssertEqual(backend.noritoDiscriminant, discriminant)
            XCTAssertEqual(VerifyingKeyBackendTag(rawValue: discriminant), backend)
        }
        XCTAssertNil(VerifyingKeyBackendTag(rawValue: 2))
        XCTAssertNil(VerifyingKeyBackendTag(rawValue: UInt32.max))
    }

    func testCanonicalLabelsRoundTripExactly() {
        let expected: [(VerifyingKeyBackendTag, String)] = [
            (.halo2IpaPasta, "halo2-ipa-pasta"),
            (.stark, "stark")
        ]

        for (backend, label) in expected {
            XCTAssertEqual(backend.canonicalLabel, label)
            XCTAssertEqual(VerifyingKeyBackendTag(canonicalLabel: label), backend)
        }
    }

    func testCanonicalParserRejectsAliasesRetiredBackendsAndConfusables() {
        let rejected = [
            "",
            " ",
            "\t",
            "\n",
            " halo2-ipa-pasta",
            "halo2-ipa-pasta ",
            "HALO2-IPA-PASTA",
            "Halo2-Ipa-Pasta",
            "halo2/ipa",
            "halo2/pasta",
            "halo2-bn254",
            "groth16",
            "groth16-bls12-377",
            "stark ",
            "STARK",
            "stark/fri",
            "halo2-ipa-orchard",
            "anonymous-pgc",
            "verange",
            "zkat",
            "silent-threshold-anoncred",
            "aztec-plonkish-private-kernel",
            "penumbra-masp",
            "stark\u{0}",
            "st\u{0430}rk",
            "halo2\u{FF0F}ipa",
            "stark\u{200B}"
        ]

        for label in rejected {
            XCTAssertNil(
                VerifyingKeyBackendTag(canonicalLabel: label),
                "\(label.debugDescription) must be rejected"
            )
        }
    }

    func testVerifierRegistryAcceptsOnlyPinnedRustProfiles() throws {
        let supported = [
            "halo2/ipa",
            "halo2/pasta/kaigi-roster-v1",
            "halo2/pasta/kaigi-usage-v1",
            "halo2/pasta/ivm-overlay-bind",
            "halo2/pasta/ivm-execution-v1",
            "halo2/pasta/kagemusha-topup-shield-merkle16-axiom-poseidon-v3",
            "halo2/pasta/kagemusha-recursive-spend-step-eq-two-parent-operation-protocol-v2",
            "halo2/pasta/kagemusha-recursive-spend-step-ep-two-parent-operation-protocol-v2",
            "halo2/pasta/confidential-transfer-2x2-merkle16-axiom-poseidon-v3",
            "halo2/pasta/confidential-unshield-full-merkle16-axiom-poseidon-v3",
            "halo2/pasta/confidential-unshield-change-merkle16-axiom-poseidon-v4",
            "stark/fri",
            "stark/fri/sha256-goldilocks",
            "stark/fri/poseidon2-goldilocks",
            "stark/fri/sha256_goldilocks.v1"
        ]

        for label in supported {
            XCTAssertTrue(VerifierBackendRegistryLabels.isSupported(label), label)
            XCTAssertEqual(
                try VerifierBackendRegistryLabels.requireSupported(label),
                label
            )
        }
    }

    func testVerifierRegistryRejectsAliasesRetiredProfilesAndConfusables() {
        let rejected: [String?] = [
            nil,
            "",
            " ",
            " halo2/ipa",
            "halo2/ipa ",
            "HALO2/IPA",
            "halo2-ipa-pasta",
            "halo2/ipa-pasta-cycle-v1",
            "halo2/pasta/ipa/ivm-execution-v1",
            "halo2/ipa:ivm-execution-v1",
            "halo2/ipa::ivm-execution-v1",
            "stark",
            "STARK/FRI",
            "stark/fri/",
            "stark/fri/latest",
            "stark/fri/sha512-goldilocks",
            "halo2/bn254",
            "groth16",
            "groth16/bls12-377",
            "halo2-ipa-orchard",
            "anonymous-pgc",
            "verange",
            "zkat",
            "silent-threshold-anoncred",
            "aztec-plonkish-private-kernel",
            "penumbra-masp",
            "stark/fri/sha256-goldilocks\u{0}",
            "st\u{0430}rk/fri",
            "stark\u{FF0F}fri",
            "stark/fri/\u{200B}sha256-goldilocks"
        ]

        for label in rejected {
            XCTAssertFalse(VerifierBackendRegistryLabels.isSupported(label))
            guard let label else {
                continue
            }
            XCTAssertThrowsError(
                try VerifierBackendRegistryLabels.requireSupported(
                    label,
                    context: "registryBackend"
                )
            ) { error in
                XCTAssertEqual(
                    error as? VerifierBackendRegistryLabelValidationError,
                    .unsupported(context: "registryBackend", label: label)
                )
            }
        }
    }
}
