package org.hyperledger.iroha.sdk.tools

import org.junit.jupiter.api.Assertions.assertThrows
import org.junit.jupiter.api.Assertions.assertTrue
import org.junit.jupiter.api.Test

/** Draft parser-only DATA controls. Every call must refuse before opening any fixture path. */
class AndroidAttestationCommandFirstDeviceV1Test {
    @Test fun firstDeviceRequiresEachNewIndependentInputBeforeFileAccess() {
        for (flag in listOf("--signed-challenge-original", "--expected-app-package", "--expected-app-version-code",
            "--expected-app-signing-identity-sha256", "--allowed-security-levels")) {
            val values = validFirstDeviceArguments().toMutableList()
            val index = values.indexOf(flag); values.removeAt(index); values.removeAt(index)
            rejectsFirstDevice(values.toTypedArray(), "$flag is required")
        }
    }

    @Test fun firstDeviceRejectsOldSelectorFlagsWithoutFallbackOrSelfPopulatedSpki() {
        for ((flag, value) in listOf("--challenge-hex" to "11", "--challenge-file" to "unopened.hex",
            "--expected-leaf-spki-sha256" to "11".repeat(32))) {
            rejectsFirstDevice(validFirstDeviceArguments() + arrayOf(flag, value), "Unknown argument: $flag")
        }
        rejectsFirstDevice(validFirstDeviceArguments() + "--require-strongbox", "--require-strongbox is not a first-device argument")
        val failure = assertThrows(IllegalArgumentException::class.java) {
            AndroidAttestationCommand.run(validFirstDeviceArguments())
        }
        assertTrue(failure.message!!.contains("Unknown argument: --signed-challenge-original"))
    }

    @Test fun firstDeviceRejectsDuplicateInputsAndBothOrMissingChainSources() {
        rejectsFirstDevice(validFirstDeviceArguments() + arrayOf("--signed-challenge-original", "unopened-again.original"),
            "Duplicate --signed-challenge-original")
        rejectsFirstDevice(validFirstDeviceArguments() + arrayOf("--bundle-dir", "unopened-directory"),
            "Exactly one --chain or --bundle-dir is required")
        rejectsFirstDevice(validFirstDeviceArguments().drop(2).toTypedArray(), "Exactly one --chain or --bundle-dir is required")
    }

    @Test fun externalAliasMustBeCanonicalDataAndRootsStayIndependent() {
        val alias = validFirstDeviceArguments().toMutableList()
        alias[alias.indexOf("--alias") + 1] = " noncanonical-label "
        rejectsFirstDevice(alias.toTypedArray(), "canonical separately trusted --alias")
        val roots = validFirstDeviceArguments().toMutableList()
        val index = roots.indexOf("--trust-root"); roots.removeAt(index); roots.removeAt(index)
        rejectsFirstDevice(roots.toTypedArray(), "At least one separately trusted root source is required")
        // A different canonical label is not a certificate-authenticated alias; no such claim/test.
    }

    @Test fun oldModeStillRequiresItsIndependentSpkiSelector() {
        val values = arrayOf("--chain", "unopened.der", "--alias", "selected-label", "--trust-root", "unopened-root.der",
            "--challenge-hex", "11", "--revocation-snapshot", "unopened.snapshot",
            "--revocation-snapshot-sha256", "11".repeat(32), "--evaluation-time-ms", "1800000000000")
        val failure = assertThrows(IllegalArgumentException::class.java) { AndroidAttestationCommand.run(values) }
        assertTrue(failure.message!!.contains("--expected-leaf-spki-sha256 is required"))
    }

    private fun rejectsFirstDevice(args: Array<String>, expected: String) {
        val failure = assertThrows(IllegalArgumentException::class.java) { AndroidAttestationCommand.runFirstDevice(args) }
        assertTrue(failure.message!!.contains(expected), "Unexpected refusal: ${failure.message}")
    }

    private fun validFirstDeviceArguments(): Array<String> = arrayOf(
        "--chain", "unopened.der", "--alias", "selected-label", "--trust-root", "unopened-root.der",
        "--revocation-snapshot", "unopened.snapshot", "--revocation-snapshot-sha256", "11".repeat(32),
        "--evaluation-time-ms", "1800000000000", "--signed-challenge-original", "unopened.original",
        "--expected-app-package", "test.known.public", "--expected-app-version-code", "1",
        "--expected-app-signing-identity-sha256", "44".repeat(32), "--allowed-security-levels", "STRONG_BOX",
    )
}
