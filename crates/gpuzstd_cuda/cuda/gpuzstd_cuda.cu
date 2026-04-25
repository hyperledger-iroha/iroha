#include <cuda_runtime.h>
#include <chrono>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <thread>

enum {
    RC_OK = 0,
    RC_INVALID = 1,
    RC_NO_SPACE = 2,
    RC_GPU_UNAVAILABLE = 3,
    RC_ZSTD = 4,
};

static constexpr uint32_t HASH_SIZE = 4096U;
static constexpr auto CUDA_COMMAND_TIMEOUT = std::chrono::seconds(120);

typedef struct {
    uint32_t len;
    uint32_t chunk_size;
    uint32_t min_match;
    uint32_t max_match;
} GpuZstdParams;

typedef struct {
    uint32_t lit_len;
    uint32_t match_len;
    uint32_t offset;
    uint32_t reserved;
} GpuZstdSequence;

__device__ __forceinline__ uint32_t hash4(const uint8_t* input, uint32_t pos) {
    uint32_t v = (uint32_t)input[pos]
               | ((uint32_t)input[pos + 1] << 8)
               | ((uint32_t)input[pos + 2] << 16)
               | ((uint32_t)input[pos + 3] << 24);
    v ^= v >> 16;
    v *= 0x7feb352dU;
    v ^= v >> 15;
    v *= 0x846ca68bU;
    v ^= v >> 16;
    return v;
}

__device__ __forceinline__ void init_hash_table(int* hash_table) {
    for (uint32_t i = 0; i < HASH_SIZE; ++i) {
        hash_table[i] = -1;
    }
}

extern "C" __global__ void gpuzstd_count_sequences_kernel(const uint8_t* input,
                                                           uint32_t* out_counts,
                                                           GpuZstdParams params) {
    uint32_t gid = (uint32_t)blockIdx.x * blockDim.x + threadIdx.x;
    uint32_t start = gid * params.chunk_size;
    if (start >= params.len) {
        return;
    }
    uint32_t end = start + params.chunk_size;
    if (end > params.len) {
        end = params.len;
    }

    int hash_table[HASH_SIZE];
    init_hash_table(hash_table);

    uint32_t seq_count = 0;
    uint32_t pos = start;
    while (pos + params.min_match <= end) {
        uint32_t match_len = 0;
        if (pos + 4 <= end) {
            uint32_t h = hash4(input, pos) & (HASH_SIZE - 1U);
            int match_pos = hash_table[h];
            hash_table[h] = (int)pos;
            if (match_pos >= (int)start && pos > (uint32_t)match_pos) {
                uint32_t max_len = params.max_match;
                uint32_t max_from_pos = end - pos;
                uint32_t max_from_match = end - (uint32_t)match_pos;
                if (max_len > max_from_pos) {
                    max_len = max_from_pos;
                }
                if (max_len > max_from_match) {
                    max_len = max_from_match;
                }
                while (match_len < max_len
                    && input[(uint32_t)match_pos + match_len] == input[pos + match_len]) {
                    ++match_len;
                }
                if (match_len < params.min_match) {
                    match_len = 0;
                }
            }
        }

        if (match_len >= params.min_match) {
            ++seq_count;
            pos += match_len;
        } else {
            ++pos;
        }
    }
    out_counts[gid] = seq_count + 1U;
}

extern "C" __global__ void gpuzstd_write_sequences_kernel(const uint8_t* input,
                                                           const uint32_t* offsets,
                                                           GpuZstdSequence* out_seqs,
                                                           uint32_t seq_capacity,
                                                           uint32_t* status,
                                                           GpuZstdParams params) {
    uint32_t gid = (uint32_t)blockIdx.x * blockDim.x + threadIdx.x;
    uint32_t start = gid * params.chunk_size;
    if (start >= params.len) {
        return;
    }
    uint32_t end = start + params.chunk_size;
    if (end > params.len) {
        end = params.len;
    }

    int hash_table[HASH_SIZE];
    init_hash_table(hash_table);

    uint32_t seq_idx = offsets[gid];
    uint32_t lit_start = start;
    uint32_t pos = start;
    while (pos + params.min_match <= end) {
        uint32_t match_len = 0;
        uint32_t offset = 0;
        if (pos + 4 <= end) {
            uint32_t h = hash4(input, pos) & (HASH_SIZE - 1U);
            int match_pos = hash_table[h];
            hash_table[h] = (int)pos;
            if (match_pos >= (int)start && pos > (uint32_t)match_pos) {
                uint32_t max_len = params.max_match;
                uint32_t max_from_pos = end - pos;
                uint32_t max_from_match = end - (uint32_t)match_pos;
                if (max_len > max_from_pos) {
                    max_len = max_from_pos;
                }
                if (max_len > max_from_match) {
                    max_len = max_from_match;
                }
                while (match_len < max_len
                    && input[(uint32_t)match_pos + match_len] == input[pos + match_len]) {
                    ++match_len;
                }
                if (match_len >= params.min_match) {
                    offset = pos - (uint32_t)match_pos;
                } else {
                    match_len = 0;
                }
            }
        }

        if (match_len >= params.min_match) {
            if (seq_idx >= seq_capacity) {
                status[gid] = RC_NO_SPACE;
                return;
            }
            GpuZstdSequence seq;
            seq.lit_len = pos - lit_start;
            seq.match_len = match_len;
            seq.offset = offset;
            seq.reserved = 0;
            out_seqs[seq_idx++] = seq;
            pos += match_len;
            lit_start = pos;
        } else {
            ++pos;
        }
    }

    if (seq_idx >= seq_capacity) {
        status[gid] = RC_NO_SPACE;
        return;
    }
    GpuZstdSequence tail;
    tail.lit_len = end - lit_start;
    tail.match_len = 0;
    tail.offset = 0;
    tail.reserved = 0;
    out_seqs[seq_idx] = tail;
}

static int ensure_cuda_device() {
    int device_count = 0;
    cudaError_t err = cudaGetDeviceCount(&device_count);
    if (err != cudaSuccess || device_count == 0) {
        return RC_GPU_UNAVAILABLE;
    }
    return RC_OK;
}

template <typename QueryFn>
static cudaError_t wait_until_cuda_ready(QueryFn query) {
    const auto start = std::chrono::steady_clock::now();
    for (;;) {
        cudaError_t status = query();
        if (status == cudaSuccess) {
            return cudaSuccess;
        }
        if (status != cudaErrorNotReady) {
            return status;
        }
        if (std::chrono::steady_clock::now() - start >= CUDA_COMMAND_TIMEOUT) {
            return cudaErrorLaunchTimeout;
        }
        std::this_thread::sleep_for(std::chrono::milliseconds(1));
    }
}

static cudaError_t wait_for_default_stream() {
    return wait_until_cuda_ready([]() { return cudaStreamQuery(nullptr); });
}

extern "C" int gpuzstd_cuda_count_sequences(const uint8_t* input,
                                             size_t len,
                                             uint32_t chunk_size,
                                             uint32_t min_match,
                                             uint32_t max_match,
                                             uint32_t* out_counts,
                                             uint32_t counts_len) {
    if (input == nullptr || out_counts == nullptr || chunk_size == 0
        || min_match == 0 || max_match < min_match) {
        return RC_INVALID;
    }
    if (len > UINT32_MAX) {
        return RC_INVALID;
    }
    if (len == 0) {
        if (counts_len > 0) {
            out_counts[0] = 0;
        }
        return RC_OK;
    }

    int device_rc = ensure_cuda_device();
    if (device_rc != RC_OK) {
        return device_rc;
    }

    uint32_t input_len = (uint32_t)len;
    uint32_t chunk_count = (input_len + chunk_size - 1U) / chunk_size;
    if (chunk_count > counts_len) {
        return RC_NO_SPACE;
    }

    uint8_t* d_input = nullptr;
    uint32_t* d_counts = nullptr;
    int ret = RC_OK;
    bool free_device_buffers = true;
    cudaError_t wait_status = cudaSuccess;

    if (cudaMalloc((void**)&d_input, len) != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }
    if (cudaMalloc((void**)&d_counts, chunk_count * sizeof(uint32_t)) != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }
    if (cudaMemcpy(d_input, input, len, cudaMemcpyHostToDevice) != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }

    GpuZstdParams params;
    params.len = input_len;
    params.chunk_size = chunk_size;
    params.min_match = min_match;
    params.max_match = max_match;
    gpuzstd_count_sequences_kernel<<<chunk_count, 1>>>(d_input, d_counts, params);
    if (cudaGetLastError() != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }
    wait_status = wait_for_default_stream();
    if (wait_status != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        free_device_buffers = wait_status != cudaErrorLaunchTimeout;
        goto cleanup;
    }
    if (cudaMemcpy(out_counts,
                   d_counts,
                   chunk_count * sizeof(uint32_t),
                   cudaMemcpyDeviceToHost) != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }

cleanup:
    if (free_device_buffers && d_input != nullptr) {
        cudaFree(d_input);
    }
    if (free_device_buffers && d_counts != nullptr) {
        cudaFree(d_counts);
    }
    return ret;
}

extern "C" int gpuzstd_cuda_write_sequences(const uint8_t* input,
                                             size_t len,
                                             uint32_t chunk_size,
                                             uint32_t min_match,
                                             uint32_t max_match,
                                             const uint32_t* offsets,
                                             uint32_t offsets_len,
                                             GpuZstdSequence* out_seqs,
                                             uint32_t seq_capacity) {
    if (input == nullptr || offsets == nullptr || out_seqs == nullptr || chunk_size == 0
        || min_match == 0 || max_match < min_match) {
        return RC_INVALID;
    }
    if (len > UINT32_MAX) {
        return RC_INVALID;
    }
    if (len == 0) {
        return RC_OK;
    }

    int device_rc = ensure_cuda_device();
    if (device_rc != RC_OK) {
        return device_rc;
    }

    uint32_t input_len = (uint32_t)len;
    uint32_t chunk_count = (input_len + chunk_size - 1U) / chunk_size;
    if (chunk_count > offsets_len) {
        return RC_INVALID;
    }

    uint8_t* d_input = nullptr;
    uint32_t* d_offsets = nullptr;
    GpuZstdSequence* d_seqs = nullptr;
    uint32_t* d_status = nullptr;
    uint32_t* host_status = nullptr;
    int ret = RC_OK;
    bool free_device_buffers = true;
    cudaError_t wait_status = cudaSuccess;

    host_status = (uint32_t*)malloc(chunk_count * sizeof(uint32_t));
    if (host_status == nullptr) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }

    if (cudaMalloc((void**)&d_input, len) != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }
    if (cudaMalloc((void**)&d_offsets, offsets_len * sizeof(uint32_t)) != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }
    if (cudaMalloc((void**)&d_seqs, seq_capacity * sizeof(GpuZstdSequence)) != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }
    if (cudaMalloc((void**)&d_status, chunk_count * sizeof(uint32_t)) != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }
    if (cudaMemset(d_status, 0, chunk_count * sizeof(uint32_t)) != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }
    if (cudaMemcpy(d_input, input, len, cudaMemcpyHostToDevice) != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }
    if (cudaMemcpy(d_offsets,
                   offsets,
                   offsets_len * sizeof(uint32_t),
                   cudaMemcpyHostToDevice) != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }

    GpuZstdParams params;
    params.len = input_len;
    params.chunk_size = chunk_size;
    params.min_match = min_match;
    params.max_match = max_match;
    gpuzstd_write_sequences_kernel<<<chunk_count, 1>>>(
        d_input, d_offsets, d_seqs, seq_capacity, d_status, params);
    if (cudaGetLastError() != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }
    wait_status = wait_for_default_stream();
    if (wait_status != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        free_device_buffers = wait_status != cudaErrorLaunchTimeout;
        goto cleanup;
    }
    if (cudaMemcpy(host_status,
                   d_status,
                   chunk_count * sizeof(uint32_t),
                   cudaMemcpyDeviceToHost) != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }
    for (uint32_t idx = 0; idx < chunk_count; ++idx) {
        if (host_status[idx] != RC_OK) {
            ret = (int)host_status[idx];
            goto cleanup;
        }
    }
    if (cudaMemcpy(out_seqs,
                   d_seqs,
                   seq_capacity * sizeof(GpuZstdSequence),
                   cudaMemcpyDeviceToHost) != cudaSuccess) {
        ret = RC_GPU_UNAVAILABLE;
        goto cleanup;
    }

cleanup:
    if (free_device_buffers && d_input != nullptr) {
        cudaFree(d_input);
    }
    if (free_device_buffers && d_offsets != nullptr) {
        cudaFree(d_offsets);
    }
    if (free_device_buffers && d_seqs != nullptr) {
        cudaFree(d_seqs);
    }
    if (free_device_buffers && d_status != nullptr) {
        cudaFree(d_status);
    }
    free(host_status);
    return ret;
}
