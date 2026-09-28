"""Execute CUDA host ownership/cleanup code with deterministic runtime failures.

This compiles the actual host helpers and pending-wait function, not a rewritten
cleanup implementation. It qualifies host sequencing; it does not execute a CUDA
kernel or establish device-side erasure on physical hardware.
"""

from pathlib import Path
import shutil
import subprocess

import pytest


RUNTIME = r"""
#pragma once
#include <cstdlib>
#include <cstring>
#include <cassert>
#include <vector>
#include <unordered_map>
#define __device__
#define __constant__
using cudaError_t = int;
using cudaStream_t = void*;
using cudaEvent_t = void*;
constexpr int cudaSuccess = 0, cudaErrorInvalidValue = 1, cudaErrorMemoryAllocation = 2;
constexpr int cudaErrorNotReady = 3, cudaErrorLaunchTimeout = 4, cudaErrorUnknown = 5;
constexpr int cudaStreamNonBlocking = 1, cudaEventDisableTiming = 2, cudaHostAllocDefault = 0;
struct QueuedErase { void* pointer; size_t bytes; };
static std::vector<QueuedErase> queued_erases;
static std::unordered_map<void*, size_t> allocations;
static int erase_calls = 0, fail_erase_at = 0, query_error = 0, device_error = 0;
static int free_calls = 0;
static bool zero_bytes(const void* pointer, size_t size) {
    const unsigned char* bytes = static_cast<const unsigned char*>(pointer);
    for (size_t i = 0; i < size; ++i) if (bytes[i] != 0) return false;
    return true;
}
static cudaError_t cudaMalloc(void** out, size_t bytes) {
    *out = std::malloc(bytes);
    if (*out == nullptr) return cudaErrorMemoryAllocation;
    allocations[*out] = bytes;
    std::memset(*out, 0xa5, bytes);
    return cudaSuccess;
}
static cudaError_t cudaHostAlloc(void** out, size_t bytes, int) { return cudaMalloc(out, bytes); }
static cudaError_t cudaFree(void* pointer) {
    assert(allocations.count(pointer) == 1);
    assert(zero_bytes(pointer, allocations.at(pointer)));
    allocations.erase(pointer); ++free_calls; std::free(pointer); return cudaSuccess;
}
static cudaError_t cudaFreeHost(void* pointer) { return cudaFree(pointer); }
static cudaError_t cudaGetDevice(int* device) { *device = 0; return cudaSuccess; }
static cudaError_t cudaSetDevice(int) { return device_error; }
static cudaError_t cudaStreamCreateWithFlags(cudaStream_t* stream, int) {
    *stream = reinterpret_cast<void*>(1); return cudaSuccess;
}
static cudaError_t cudaStreamDestroy(cudaStream_t) { return cudaSuccess; }
static cudaError_t cudaEventCreateWithFlags(cudaEvent_t* event, int) {
    *event = reinterpret_cast<void*>(2); return cudaSuccess;
}
static cudaError_t cudaEventDestroy(cudaEvent_t) { return cudaSuccess; }
static cudaError_t cudaMemsetAsync(void* pointer, int value, size_t bytes, cudaStream_t stream) {
    assert(stream != nullptr && value == 0);
    ++erase_calls;
    if (erase_calls == fail_erase_at) return cudaErrorUnknown;
    queued_erases.push_back({pointer, bytes}); return cudaSuccess;
}
static cudaError_t cudaStreamQuery(cudaStream_t) {
    if (query_error != 0) return query_error;
    for (const auto& item : queued_erases) std::memset(item.pointer, 0, item.bytes);
    queued_erases.clear(); return cudaSuccess;
}
static cudaError_t cudaEventQuery(cudaEvent_t) { return cudaStreamQuery(nullptr); }
static cudaError_t cudaEventRecord(cudaEvent_t, cudaStream_t) { return cudaSuccess; }
"""


CONTROLS = r"""
static void reset_runtime() {
    queued_erases.clear(); async_dispatch_pools().clear();
    erase_calls = 0; fail_erase_at = 0; query_error = 0; device_error = 0;
    cuda_backend_quarantined().store(false);
}
struct Fixture {
    uint64_t dense[4] = {1,2,3,4}, coeffs[4] = {5,6,7,8}, evals[4] = {9,10,11,12};
    uint64_t host_dense[4] = {13,14,15,16}, host_coeffs[4] = {17,18,19,20};
    uint64_t host_evals[4] = {21,22,23,24};
    template<class T> void bind(T* item) {
        item->dense = dense; item->coeffs = coeffs; item->evals = evals;
        item->host_dense = host_dense; item->host_coeffs = host_coeffs; item->host_evals = host_evals;
        item->dense_capacity_bytes = sizeof(dense); item->coeff_capacity_bytes = sizeof(coeffs);
        item->eval_capacity_bytes = sizeof(evals); item->host_dense_capacity_bytes = sizeof(host_dense);
        item->host_coeff_capacity_bytes = sizeof(host_coeffs); item->host_eval_capacity_bytes = sizeof(host_evals);
        item->stream = reinterpret_cast<void*>(1); item->stream_ready = true;
    }
    bool cleared() const {
        return zero_bytes(dense, sizeof(dense)) && zero_bytes(coeffs, sizeof(coeffs))
            && zero_bytes(evals, sizeof(evals)) && host_cleared();
    }
    bool host_cleared() const {
        return zero_bytes(host_dense, sizeof(host_dense)) && zero_bytes(host_coeffs, sizeof(host_coeffs))
            && zero_bytes(host_evals, sizeof(host_evals));
    }
};
static PendingTransform* pending(Fixture& fixture) {
    auto* buffers = new AsyncDispatchBuffers(0); fixture.bind(buffers);
    return new PendingTransform(0, reinterpret_cast<void*>(2), buffers,
                                fixture.host_dense, sizeof(fixture.host_dense));
}
static void completed_wait_copies_then_clears_and_pools() {
    reset_runtime(); Fixture fixture; uint64_t output[4] = {};
    assert(fastpq_pending_wait_cuda(pending(fixture), output, 4) == cudaSuccess);
    assert(output[0] == 13 && output[3] == 16);
    assert(fixture.cleared() && erase_calls == 3 && queued_erases.empty());
    auto* reused = acquire_async_dispatch_buffers(0);
    assert(reused->dense == fixture.dense && reused->host_dense == fixture.host_dense);
    delete reused;
}
static void invalid_destination_and_drop_still_clear() {
    for (int mode = 0; mode != 2; ++mode) {
        reset_runtime(); Fixture fixture; uint64_t output[4] = {};
        const auto status = fastpq_pending_wait_cuda(pending(fixture), mode ? nullptr : output, mode ? 0 : 3);
        assert(status == (mode ? cudaSuccess : cudaErrorInvalidValue));
        assert(fixture.cleared()); delete acquire_async_dispatch_buffers(0);
    }
}
static void unknown_completion_never_wipes_or_recycles() {
    reset_runtime(); Fixture fixture; query_error = cudaErrorLaunchTimeout;
    assert(destroy_pending_transform(pending(fixture)) == cudaErrorLaunchTimeout);
    assert(cuda_backend_is_quarantined() && erase_calls == 0);
    assert(fixture.dense[0] == 1 && fixture.host_dense[0] == 13);
    assert(async_dispatch_pools().empty() && acquire_async_dispatch_buffers(0) == nullptr);
}
static void failed_erase_cannot_recycle_even_after_known_dispatch_completion() {
    for (int failure = 1; failure <= 3; ++failure) {
        reset_runtime(); Fixture fixture; fail_erase_at = failure;
        assert(destroy_pending_transform(pending(fixture)) == cudaErrorUnknown);
        assert(fixture.host_cleared() && cuda_backend_is_quarantined());
        assert(async_dispatch_pools().empty());
    }
    reset_runtime(); Fixture fixture;
    auto* buffers = new AsyncDispatchBuffers(0); fixture.bind(buffers);
    query_error = cudaErrorLaunchTimeout;
    assert(release_async_dispatch_buffers(buffers) == cudaErrorLaunchTimeout);
    assert(fixture.host_cleared() && !zero_bytes(fixture.dense, sizeof(fixture.dense)));
    assert(cuda_backend_is_quarantined() && async_dispatch_pools().empty());
}
static void pre_dispatch_failure_and_synchronous_workspace_clear_full_capacity() {
    reset_runtime(); Fixture fixture;
    auto* buffers = new AsyncDispatchBuffers(0); fixture.bind(buffers);
    // Pinned input was already copied, but no kernel/async copy was submitted.
    assert(release_async_dispatch_buffers(buffers) == cudaSuccess);
    assert(fixture.cleared()); delete acquire_async_dispatch_buffers(0);
    reset_runtime(); Fixture second; TransformWorkspace workspace; second.bind(&workspace);
    assert(scrub_completed_transform(&workspace) == cudaSuccess && second.cleared());
}
static void growth_erases_before_free_and_abandons_failed_erases() {
    reset_runtime(); uint64_t* device = nullptr; size_t capacity = 0;
    assert(ensure_workspace_buffer(&device, &capacity, 32) == cudaSuccess);
    assert(ensure_workspace_buffer(&device, &capacity, 64) == cudaSuccess);
    assert(capacity == 64 && erase_calls == 1);
    std::memset(device, 0, capacity); assert(cudaFree(device) == cudaSuccess);
    uint64_t* host = nullptr; capacity = 0;
    assert(ensure_pinned_workspace_buffer(&host, &capacity, 32) == cudaSuccess);
    assert(ensure_pinned_workspace_buffer(&host, &capacity, 64) == cudaSuccess);
    wipe_private_host_region(host, capacity); assert(cudaFreeHost(host) == cudaSuccess);
    reset_runtime(); device = nullptr; capacity = 0;
    assert(ensure_workspace_buffer(&device, &capacity, 32) == cudaSuccess);
    auto* original = device; const int before = free_calls; fail_erase_at = 1;
    assert(ensure_workspace_buffer(&device, &capacity, 64) == cudaErrorUnknown);
    assert(device == original && capacity == 32 && free_calls == before);
    assert(cuda_backend_is_quarantined());
    std::memset(device, 0, capacity); assert(cudaFree(device) == cudaSuccess);
}
static void older_poseidon_private_regions_clear_but_public_constants_remain() {
    reset_runtime(); PoseidonWorkspace workspace;
    uint64_t device[4][4], host[4][4], constants[4] = {91,92,93,94};
    std::memset(device, 0x6b, sizeof(device)); std::memset(host, 0x7c, sizeof(host));
    workspace.payloads = device[0]; workspace.slices = reinterpret_cast<PoseidonSlice*>(device[1]);
    workspace.states = device[2]; workspace.hashes = device[3];
    workspace.host_payloads = host[0]; workspace.host_slices = reinterpret_cast<PoseidonSlice*>(host[1]);
    workspace.host_states = host[2]; workspace.host_hashes = host[3];
    workspace.payload_capacity_bytes = workspace.slice_capacity_bytes = workspace.state_capacity_bytes = workspace.hash_capacity_bytes = 32;
    workspace.host_payload_capacity_bytes = workspace.host_slice_capacity_bytes = workspace.host_state_capacity_bytes = workspace.host_hash_capacity_bytes = 32;
    workspace.bn254_round_constants = constants; workspace.stream = reinterpret_cast<void*>(1);
    assert(scrub_completed_poseidon(&workspace) == cudaSuccess);
    assert(zero_bytes(device, sizeof(device)) && zero_bytes(host, sizeof(host)));
    assert(constants[0] == 91 && constants[3] == 94);
}
int main() {
    completed_wait_copies_then_clears_and_pools();
    invalid_destination_and_drop_still_clear();
    unknown_completion_never_wipes_or_recycles();
    failed_erase_cannot_recycle_even_after_known_dispatch_completion();
    pre_dispatch_failure_and_synchronous_workspace_clear_full_capacity();
    growth_erases_before_free_and_abandons_failed_erases();
    older_poseidon_private_regions_clear_but_public_constants_remain();
    assert(allocations.empty());
}
"""


def test_cuda_host_cleanup_uses_completed_ownership_before_erasing(tmp_path):
    compiler = shutil.which("clang++") or shutil.which("c++")
    if compiler is None:
        pytest.skip("a C++ compiler is required for CUDA host ownership controls")
    source = (Path(__file__).resolve().parents[1] / "crates/fastpq_prover/cuda/fastpq_cuda.cu").read_text()
    prefix = source.split("static __device__ __constant__ uint64_t POSEIDON_ROUND_CONSTANTS", 1)[0]
    wait = source.split('extern "C" cudaError_t fastpq_pending_wait_cuda(', 1)[1]
    wait = 'extern "C" cudaError_t fastpq_pending_wait_cuda(' + wait.split(
        'extern "C" cudaError_t fastpq_fft_async_submit_cuda(', 1
    )[0]
    (tmp_path / "cuda_runtime.h").write_text(RUNTIME)
    harness = tmp_path / "cleanup.cpp"
    harness.write_text(prefix + wait + CONTROLS)
    executable = tmp_path / "cleanup"
    subprocess.run([compiler, "-std=c++17", "-O2", "-pthread", "-I", str(tmp_path),
                    str(harness), "-o", str(executable)], check=True, capture_output=True, text=True,
                   timeout=120)
    subprocess.run([str(executable)], check=True, capture_output=True, text=True, timeout=30)
