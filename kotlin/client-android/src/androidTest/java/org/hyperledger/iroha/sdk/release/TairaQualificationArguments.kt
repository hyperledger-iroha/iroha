// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0
package org.hyperledger.iroha.sdk.release

import androidx.test.platform.app.InstrumentationRegistry
import org.hyperledger.iroha.sdk.crypto.NativeSignerBridge

/** Reviewed invocation inputs. There are deliberately no ABI or run-id defaults. */
internal class TairaQualificationArguments private constructor(
    val runId: String,
    val kind: String,
    val expectedBridgeAbiVersion: Int,
    val expectedNativeSignerContractRevision: Int,
    val selectedTests: List<String>,
) {
    fun requireNative() {
        check(expectedBridgeAbiVersion == NativeSignerBridge.REQUIRED_BRIDGE_ABI_VERSION) {
            "Reviewed ABI does not match the compiled bridge contract"
        }
        check(expectedNativeSignerContractRevision ==
            NativeSignerBridge.REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION) {
            "Reviewed signer revision does not match the compiled bridge contract"
        }
        // The public bridge loader probes both actual JNI versions before returning true.
        check(NativeSignerBridge.isNativeAvailable()) { "Exact native bridge is unavailable" }
    }

    companion object {
        const val TEST_PACKAGE = "org.hyperledger.iroha.sdk.android.test"
        const val LISTENER = "org.hyperledger.iroha.sdk.release.TairaOriginalJUnitListener"
        const val MANAGED_CLASS = "org.hyperledger.iroha.sdk.release.TairaManagedNativeDeviceTest"
        const val WIRE_CLASS = "org.hyperledger.iroha.sdk.release.TairaNativeLedgerWireTest"
        val MANAGED_TESTS = listOf(
            "$MANAGED_CLASS/exactAbiSignsAndVerifiesWithNativeAlgorithms",
            "$MANAGED_CLASS/nativeAccountCodecRejectsWrongNetworkAndMalformedInput",
        )
        val WIRE_TESTS = listOf(
            "$WIRE_CLASS/liveAccountListI105MatchesNativeCodec",
            "$WIRE_CLASS/liveAccountDetailI105MatchesNativeCodec",
        )

        fun read(): TairaQualificationArguments {
            val instrumentation = InstrumentationRegistry.getInstrumentation()
            check(instrumentation.context.packageName == TEST_PACKAGE) {
                "Unexpected instrumentation package"
            }
            val arguments = InstrumentationRegistry.getArguments()
            fun required(name: String): String = checkNotNull(arguments.getString(name)) {
                "Missing qualification argument $name"
            }
            fun positive(name: String): Int {
                val literal = required(name)
                check(Regex("[1-9][0-9]{0,8}").matches(literal)) { "Invalid $name" }
                return literal.toInt()
            }
            val runId = required("bpngEvidenceRunId")
            check(Regex("[0-9a-f]{64}").matches(runId) && runId.any { it != '0' }) {
                "Invalid evidence run id"
            }
            check(required("listener") == LISTENER) { "Exact original listener is required" }
            val selected = required("class").split(',').map { it.replace('#', '/') }
            check(selected.size == 2 && selected.toSet().size == 2) {
                "Two unique exact selectors are required"
            }
            val kind = when (selected.toSet()) {
                MANAGED_TESTS.toSet() -> "managed-native"
                WIRE_TESTS.toSet() -> "native-ledger-wire"
                else -> error("Unknown or mixed qualification selectors")
            }
            if (kind == "native-ledger-wire") {
                check(required("bpngLiveWire") == "1") { "Read-only live wire opt-in is required" }
            } else {
                check(!arguments.containsKey("bpngLiveWire")) { "Live wire argument on managed run" }
            }
            return TairaQualificationArguments(
                runId, kind, positive("bpngExpectedBridgeAbiVersion"),
                positive("bpngExpectedNativeSignerContractRevision"),
                if (kind == "managed-native") MANAGED_TESTS else WIRE_TESTS,
            )
        }
    }
}
