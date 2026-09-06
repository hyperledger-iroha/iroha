package org.hyperledger.iroha.sdk.gpu;

import java.nio.file.Paths;
import java.util.Arrays;
import org.junit.jupiter.api.Test;
import static org.junit.jupiter.api.Assertions.*;

/** Java consumers exercise the one Kotlin CUDA API without loading or impersonating a device. */
final class CudaAcceleratorsJavaConsumerTest {
    private static final long[] MODULUS = {
        0x43e1f593f0000001L, 0x2833e84879b97091L, 0xb85045b68181585dL, 0x30644e72e131a029L
    };

    @Test
    void disabledContextHasExplicitStateAndNoComputedResults() {
        CudaAccelerators api = CudaAccelerators.disabled();
        assertEquals(CudaAccelerators.Status.DISABLED, api.getStatus());
        assertNull(api.poseidon2(new long[][] {{1, 2}}));
        assertNull(api.poseidon6(new long[][] {{1, 2, 3, 4, 5, 6}}));
        long[][] field = {{1, 0, 0, 0}};
        assertNull(api.bn254Add(field, field));
        assertNull(api.bn254Sub(field, field));
        assertNull(api.bn254Mul(field, field));
    }

    @Test
    void injectedContextsDoNotReplaceEachOthersBackend() {
        RecordingBackend first = new RecordingBackend();
        RecordingBackend second = new RecordingBackend();
        first.output = new long[] {11};
        second.output = new long[] {22};
        CudaAccelerators a = new CudaAccelerators(first);
        CudaAccelerators b = new CudaAccelerators(second);
        assertArrayEquals(new long[] {11}, a.poseidon2(new long[][] {{1, 2}}));
        assertArrayEquals(new long[] {22}, b.poseidon2(new long[][] {{1, 2}}));
        second.status = CudaAccelerators.Status.UNAVAILABLE;
        second.output = null;
        assertEquals(CudaAccelerators.Status.READY, a.getStatus());
        assertEquals(CudaAccelerators.Status.UNAVAILABLE, b.getStatus());
        assertNull(b.poseidon2(new long[][] {{1, 2}}));
        assertArrayEquals(new long[] {11}, a.poseidon2(new long[][] {{1, 2}}));
    }

    @Test
    void poseidonUsesOrderedBatchesAndOwnsInputsAndOutputs() {
        RecordingBackend backend = new RecordingBackend();
        CudaAccelerators api = new CudaAccelerators(backend);
        backend.output = new long[] {7, 9};
        long[][] pairs = {{-1, 2}, {3, 4}};
        long[] hashes = api.poseidon2(pairs);
        assertEquals("poseidon2", backend.operation);
        assertArrayEquals(new long[] {-1, 2, 3, 4}, backend.left);
        backend.left[0] = 100;
        backend.output[0] = 200;
        assertEquals(-1, pairs[0][0]);
        assertArrayEquals(new long[] {7, 9}, hashes);
        pairs[1][0] = 300;
        assertEquals(3, backend.left[2]);
        backend.output = new long[] {13};
        long[][] six = {{1, 2, 3, 4, 5, 6}};
        assertArrayEquals(new long[] {13}, api.poseidon6(six));
        assertEquals("poseidon6", backend.operation);
        assertArrayEquals(six[0], backend.left);
        backend.left[1] = 300;
        assertEquals(2, six[0][1]);
    }

    @Test
    void fieldOperationsKeepOrderAndDoNotShareMutableRows() {
        RecordingBackend backend = new RecordingBackend();
        CudaAccelerators api = new CudaAccelerators(backend);
        long[][] lhs = {{1, 0, 0, 0}, {2, 0, 0, 0}};
        long[][] rhs = {{3, 0, 0, 0}, {4, 0, 0, 0}};
        for (String operation : new String[] {"add", "sub", "mul"}) {
            backend.output = new long[] {5, 0, 0, 0, 6, 0, 0, 0};
            long[][] result = fieldOperation(api, operation, lhs, rhs);
            assertEquals(operation, backend.operation);
            assertArrayEquals(new long[] {1, 0, 0, 0, 2, 0, 0, 0}, backend.left);
            assertArrayEquals(new long[] {3, 0, 0, 0, 4, 0, 0, 0}, backend.right);
            backend.left[0] = 99;
            backend.right[0] = 99;
            backend.output[0] = 99;
            assertEquals(1, lhs[0][0]);
            assertEquals(3, rhs[0][0]);
            assertArrayEquals(new long[] {5, 0, 0, 0}, result[0]);
            result[0][0] = 77;
            assertEquals(6, result[1][0]);
        }
    }

    @Test
    void shapeAndBatchBoundsRejectBeforeProviderInvocation() {
        RecordingBackend backend = new RecordingBackend();
        CudaAccelerators api = new CudaAccelerators(backend);
        assertThrows(IllegalArgumentException.class, () -> api.poseidon2(new long[][] {{1}}));
        assertThrows(IllegalArgumentException.class, () -> api.poseidon6(new long[][] {{1, 2}}));
        assertThrows(IllegalArgumentException.class, () ->
            api.poseidon2(new long[CudaAccelerators.MAXIMUM_BATCH_SIZE + 1][]));
        for (String operation : new String[] {"add", "sub", "mul"}) {
            assertThrows(IllegalArgumentException.class, () ->
                fieldOperation(api, operation, new long[][] {{1, 0, 0, 0}}, new long[][] {}));
            assertThrows(IllegalArgumentException.class, () ->
                fieldOperation(api, operation, new long[][] {{1, 2, 3}}, new long[][] {{1, 0, 0, 0}}));
            assertThrows(IllegalArgumentException.class, () ->
                fieldOperation(api, operation, new long[][] {MODULUS}, new long[][] {{1, 0, 0, 0}}));
            assertThrows(IllegalArgumentException.class, () ->
                fieldOperation(api, operation, new long[][] {{0, 0, 0, -1}}, new long[][] {{1, 0, 0, 0}}));
        }
        assertEquals(0, backend.calls);
    }

    @Test
    void backendMustReturnExactDimensionsAndCanonicalFields() {
        RecordingBackend backend = new RecordingBackend();
        CudaAccelerators api = new CudaAccelerators(backend);
        backend.output = new long[0];
        assertThrows(IllegalStateException.class, () -> api.poseidon2(new long[][] {{1, 2}}));
        assertThrows(IllegalStateException.class, () -> api.poseidon6(new long[][] {{1, 2, 3, 4, 5, 6}}));
        long[][] field = {{1, 0, 0, 0}};
        for (String operation : new String[] {"add", "sub", "mul"}) {
            backend.output = new long[3];
            assertThrows(IllegalStateException.class, () -> fieldOperation(api, operation, field, field));
            backend.output = MODULUS.clone();
            assertThrows(IllegalStateException.class, () -> fieldOperation(api, operation, field, field));
        }
    }

    @Test
    void nativeLoadingRequiresAnExplicitAbsoluteLibraryAndDoesNotHideFailure() {
        assertThrows(IllegalArgumentException.class, () -> CudaAccelerators.loadNative("bridge.so"));
        String missing = Paths.get("build", "missing-cuda-library", "does-not-exist.so").toAbsolutePath().toString();
        assertThrows(UnsatisfiedLinkError.class, () -> CudaAccelerators.loadNative(missing));
        assertEquals(CudaAccelerators.Status.DISABLED, CudaAccelerators.disabled().getStatus());
    }

    @Test
    void maximumBatchAndUnsignedCanonicalBoundaryAreAccepted() {
        RecordingBackend backend = new RecordingBackend();
        CudaAccelerators api = new CudaAccelerators(backend);
        long[][] inputs = new long[CudaAccelerators.MAXIMUM_BATCH_SIZE][];
        Arrays.fill(inputs, new long[] {-1, -1});
        backend.output = new long[inputs.length];
        assertEquals(inputs.length, api.poseidon2(inputs).length);
        assertEquals(inputs.length * 2, backend.left.length);
        long[] largest = MODULUS.clone();
        largest[0]--;
        backend.output = largest.clone();
        assertArrayEquals(largest, api.bn254Add(new long[][] {largest}, new long[][] {{0, 0, 0, 0}})[0]);
        backend.output = new long[0];
        assertEquals(0, api.poseidon2(new long[0][]).length);
        assertEquals(0, api.bn254Mul(new long[0][], new long[0][]).length);
    }

    private static long[][] fieldOperation(CudaAccelerators api, String operation, long[][] lhs, long[][] rhs) {
        switch (operation) {
            case "add": return api.bn254Add(lhs, rhs);
            case "sub": return api.bn254Sub(lhs, rhs);
            case "mul": return api.bn254Mul(lhs, rhs);
            default: throw new AssertionError(operation);
        }
    }

    private static final class RecordingBackend implements CudaAccelerators.Backend {
        CudaAccelerators.Status status = CudaAccelerators.Status.READY;
        long[] output;
        long[] left;
        long[] right;
        String operation;
        int calls;
        @Override public CudaAccelerators.Status getStatus() { return status; }
        private long[] record(String name, long[] lhs, long[] rhs) {
            calls++;
            operation = name;
            left = lhs;
            right = rhs;
            return output;
        }
        @Override public long[] poseidon2(long[] inputs) { return record("poseidon2", inputs, null); }
        @Override public long[] poseidon6(long[] inputs) { return record("poseidon6", inputs, null); }
        @Override public long[] bn254Add(long[] lhs, long[] rhs) { return record("add", lhs, rhs); }
        @Override public long[] bn254Sub(long[] lhs, long[] rhs) { return record("sub", lhs, rhs); }
        @Override public long[] bn254Mul(long[] lhs, long[] rhs) { return record("mul", lhs, rhs); }
    }
}
