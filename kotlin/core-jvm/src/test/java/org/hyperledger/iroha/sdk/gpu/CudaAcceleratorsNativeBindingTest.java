package org.hyperledger.iroha.sdk.gpu;

import java.io.File;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

/** Executes every canonical JNI binding; empty batches do not qualify CUDA computation. */
final class CudaAcceleratorsNativeBindingTest {
    @Test
    void rebuiltHostBridgeResolvesAllSevenKotlinDeclarations() {
        String directory = System.getProperty("java.library.path");
        assertNotNull(directory, "The test task must configure its host bridge directory");
        File bridge = new File(directory, System.mapLibraryName("connect_norito_bridge"));
        assertTrue(bridge.isAbsolute(), "The host bridge directory must be explicit and absolute");
        assertTrue(bridge.isFile(), "Build the host JNI bridge before running the native suite: " + bridge);
        CudaAccelerators api = CudaAccelerators.loadNative(bridge.getAbsolutePath());
        assertNotNull(api.getStatus());
        assertEmptyOrUnavailable(api.poseidon2(new long[0][]));
        assertEmptyOrUnavailable(api.poseidon6(new long[0][]));
        assertEmptyOrUnavailable(api.bn254Add(new long[0][], new long[0][]));
        assertEmptyOrUnavailable(api.bn254Sub(new long[0][], new long[0][]));
        assertEmptyOrUnavailable(api.bn254Mul(new long[0][], new long[0][]));
    }

    private static void assertEmptyOrUnavailable(long[] result) {
        if (result != null) assertEquals(0, result.length);
    }

    private static void assertEmptyOrUnavailable(long[][] result) {
        if (result != null) assertEquals(0, result.length);
    }
}
