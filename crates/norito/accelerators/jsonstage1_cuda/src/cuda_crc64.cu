#include <cuda.h>
#include <cuda_runtime.h>
#include <chrono>
#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>
#include <thread>

enum {
    RC_OK = 0,
    RC_INVALID = 1,
    RC_NO_SPACE = 2,
    RC_GPU_UNAVAILABLE = 3,
    RC_CUDA = 4,
};

// CRC64-XZ polynomial (reflected ECMA) and helper constants.
static constexpr uint64_t CRC64_POLY = 0xC96C5795D7870F42ULL;
static constexpr uint64_t CRC64_INIT = 0xFFFFFFFFFFFFFFFFULL;
static constexpr uint64_t CRC64_XOR_OUT = 0xFFFFFFFFFFFFFFFFULL;
static constexpr uint64_t CRC64_CHUNK_SIZE = 16ULL * 1024ULL;
static constexpr auto CUDA_COMMAND_TIMEOUT = std::chrono::seconds(120);

extern "C" __global__ void classify_json_kernel(const uint8_t* input,
                                                 uint32_t* out_structural,
                                                 uint32_t* out_quote,
                                                 uint32_t* out_backslash,
                                                 uint32_t len) {
    uint32_t gid = (uint32_t)blockIdx.x * blockDim.x + threadIdx.x;
    uint32_t base = gid * 32U;
    if (base >= len) {
        return;
    }

    uint32_t structural = 0;
    uint32_t quote = 0;
    uint32_t backslash = 0;
    #pragma unroll
    for (uint32_t i = 0; i < 32U; ++i) {
        uint32_t pos = base + i;
        if (pos >= len) {
            break;
        }
        uint8_t c = input[pos];
        quote |= (uint32_t)(c == 34U) << i;
        backslash |= (uint32_t)(c == 92U) << i;
        bool is_structural = c == (uint8_t)'{' || c == (uint8_t)'}'
                          || c == (uint8_t)'[' || c == (uint8_t)']'
                          || c == (uint8_t)':' || c == (uint8_t)',';
        structural |= (uint32_t)is_structural << i;
    }
    out_structural[gid] = structural;
    out_quote[gid] = quote;
    out_backslash[gid] = backslash;
}

__device__ __forceinline__ uint64_t crc64_update(uint64_t crc, uint8_t byte) {
    crc ^= (uint64_t)byte;
    #pragma unroll
    for (int i = 0; i < 8; ++i) {
        if (crc & 1ULL) {
            crc = (crc >> 1) ^ CRC64_POLY;
        } else {
            crc >>= 1;
        }
    }
    return crc;
}

extern "C" __global__ void crc64_chunks_kernel(const uint8_t* input,
                                               size_t len,
                                               uint64_t* out_chunks) {
    size_t gid = (size_t)blockIdx.x * blockDim.x + threadIdx.x;
    size_t start = gid * CRC64_CHUNK_SIZE;
    if (start >= len) {
        return;
    }
    size_t end = (len - start < CRC64_CHUNK_SIZE) ? len : (start + CRC64_CHUNK_SIZE);
    uint64_t crc = 0;
    for (size_t idx = start; idx < end; ++idx) {
        crc = crc64_update(crc, input[idx]);
    }
    out_chunks[gid] = crc;
}

static uint64_t gf2_matrix_times(const uint64_t* mat, uint64_t vec) {
    uint64_t sum = 0;
    int idx = 0;
    while (vec != 0) {
        if (vec & 1U) {
            sum ^= mat[idx];
        }
        vec >>= 1;
        idx += 1;
    }
    return sum;
}

static void gf2_matrix_square(uint64_t* square, const uint64_t* mat) {
    for (int n = 0; n < 64; ++n) {
        square[n] = gf2_matrix_times(mat, mat[n]);
    }
}

static uint64_t crc64_shift(uint64_t crc, size_t len2) {
    if (len2 == 0) {
        return crc;
    }

    uint64_t mat[64];
    uint64_t square[64];
    uint64_t row = 1ULL;
    mat[0] = CRC64_POLY;
    for (int n = 1; n < 64; ++n) {
        mat[n] = row;
        row <<= 1;
    }

    size_t len = len2 * 8ULL;
    while (len != 0) {
        if (len & 1ULL) {
            crc = gf2_matrix_times(mat, crc);
        }
        gf2_matrix_square(square, mat);
        for (int n = 0; n < 64; ++n) {
            mat[n] = square[n];
        }
        len >>= 1;
    }

    return crc;
}

static uint64_t crc64_combine_host(uint64_t crc1, uint64_t crc2, size_t len2) {
    if (len2 == 0) {
        return crc1;
    }
    crc1 = crc64_shift(crc1, len2);
    return crc1 ^ crc2;
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

static int finalize_tape_from_masks(const uint32_t* structural,
                                    const uint32_t* quote,
                                    const uint32_t* backslash,
                                    size_t blocks,
                                    size_t input_len,
                                    uint32_t* out_offsets,
                                    size_t out_capacity,
                                    size_t* out_len) {
    size_t need = 0;
    size_t backslash_run = 0;
    bool in_string = false;
    for (size_t block = 0; block < blocks; ++block) {
        uint32_t sm = structural[block];
        uint32_t qm = quote[block];
        uint32_t bm = backslash[block];
        size_t base = block * 32ULL;
        for (uint32_t bit = 0; bit < 32U; ++bit) {
            size_t pos = base + bit;
            if (pos >= input_len) {
                break;
            }
            bool is_backslash = ((bm >> bit) & 1U) != 0U;
            bool is_quote = ((qm >> bit) & 1U) != 0U;
            bool is_structural = ((sm >> bit) & 1U) != 0U;
            if (in_string) {
                if (is_backslash) {
                    ++backslash_run;
                    continue;
                }
                if (is_quote) {
                    bool escaped = (backslash_run & 1U) != 0U;
                    backslash_run = 0;
                    if (!escaped) {
                        in_string = false;
                        if (need < out_capacity) {
                            out_offsets[need] = (uint32_t)pos;
                        }
                        ++need;
                    }
                    continue;
                }
                backslash_run = 0;
                continue;
            }

            backslash_run = 0;
            if (is_quote) {
                in_string = true;
                if (need < out_capacity) {
                    out_offsets[need] = (uint32_t)pos;
                }
                ++need;
            } else if (is_structural) {
                if (need < out_capacity) {
                    out_offsets[need] = (uint32_t)pos;
                }
                ++need;
            }
        }
    }
    *out_len = need;
    return need > out_capacity ? RC_NO_SPACE : RC_OK;
}

extern "C" int json_stage1_build_tape_cuda_impl(const uint8_t* input_ptr,
                                                size_t input_len,
                                                uint32_t* out_offsets,
                                                size_t out_capacity,
                                                size_t* out_len) {
    if (input_ptr == nullptr || out_offsets == nullptr || out_len == nullptr) {
        return RC_INVALID;
    }
    if (input_len > UINT32_MAX) {
        return RC_INVALID;
    }
    if (input_len == 0) {
        *out_len = 0;
        return RC_OK;
    }

    int device_rc = ensure_cuda_device();
    if (device_rc != RC_OK) {
        return device_rc;
    }

    size_t blocks = (input_len + 31ULL) / 32ULL;
    uint8_t* d_input = nullptr;
    uint32_t* d_structural = nullptr;
    uint32_t* d_quote = nullptr;
    uint32_t* d_backslash = nullptr;
    uint32_t* h_structural = nullptr;
    uint32_t* h_quote = nullptr;
    uint32_t* h_backslash = nullptr;
    int ret = RC_OK;
    bool free_device_buffers = true;
    cudaError_t wait_status = cudaSuccess;

    h_structural = static_cast<uint32_t*>(malloc(blocks * sizeof(uint32_t)));
    h_quote = static_cast<uint32_t*>(malloc(blocks * sizeof(uint32_t)));
    h_backslash = static_cast<uint32_t*>(malloc(blocks * sizeof(uint32_t)));
    if (h_structural == nullptr || h_quote == nullptr || h_backslash == nullptr) {
        ret = RC_CUDA;
        goto cleanup;
    }

    if (cudaMalloc(reinterpret_cast<void**>(&d_input), input_len) != cudaSuccess) {
        ret = RC_CUDA;
        goto cleanup;
    }
    if (cudaMalloc(reinterpret_cast<void**>(&d_structural), blocks * sizeof(uint32_t))
        != cudaSuccess) {
        ret = RC_CUDA;
        goto cleanup;
    }
    if (cudaMalloc(reinterpret_cast<void**>(&d_quote), blocks * sizeof(uint32_t))
        != cudaSuccess) {
        ret = RC_CUDA;
        goto cleanup;
    }
    if (cudaMalloc(reinterpret_cast<void**>(&d_backslash), blocks * sizeof(uint32_t))
        != cudaSuccess) {
        ret = RC_CUDA;
        goto cleanup;
    }
    if (cudaMemcpy(d_input, input_ptr, input_len, cudaMemcpyHostToDevice) != cudaSuccess) {
        ret = RC_CUDA;
        goto cleanup;
    }

    dim3 block(256);
    dim3 grid(static_cast<unsigned int>((blocks + block.x - 1ULL) / block.x));
    classify_json_kernel<<<grid, block>>>(d_input,
                                          d_structural,
                                          d_quote,
                                          d_backslash,
                                          static_cast<uint32_t>(input_len));
    if (cudaGetLastError() != cudaSuccess) {
        ret = RC_CUDA;
        goto cleanup;
    }
    wait_status = wait_for_default_stream();
    if (wait_status != cudaSuccess) {
        ret = RC_CUDA;
        free_device_buffers = wait_status != cudaErrorLaunchTimeout;
        goto cleanup;
    }
    if (cudaMemcpy(h_structural,
                   d_structural,
                   blocks * sizeof(uint32_t),
                   cudaMemcpyDeviceToHost) != cudaSuccess) {
        ret = RC_CUDA;
        goto cleanup;
    }
    if (cudaMemcpy(h_quote,
                   d_quote,
                   blocks * sizeof(uint32_t),
                   cudaMemcpyDeviceToHost) != cudaSuccess) {
        ret = RC_CUDA;
        goto cleanup;
    }
    if (cudaMemcpy(h_backslash,
                   d_backslash,
                   blocks * sizeof(uint32_t),
                   cudaMemcpyDeviceToHost) != cudaSuccess) {
        ret = RC_CUDA;
        goto cleanup;
    }

    ret = finalize_tape_from_masks(h_structural,
                                   h_quote,
                                   h_backslash,
                                   blocks,
                                   input_len,
                                   out_offsets,
                                   out_capacity,
                                   out_len);

cleanup:
    if (free_device_buffers && d_input) {
        cudaFree(d_input);
    }
    if (free_device_buffers && d_structural) {
        cudaFree(d_structural);
    }
    if (free_device_buffers && d_quote) {
        cudaFree(d_quote);
    }
    if (free_device_buffers && d_backslash) {
        cudaFree(d_backslash);
    }
    free(h_structural);
    free(h_quote);
    free(h_backslash);
    return ret;
}

extern "C" int norito_crc64_cuda_impl(const uint8_t* input_ptr,
                                      size_t input_len,
                                      uint64_t* out_crc) {
    if (input_ptr == nullptr || out_crc == nullptr) {
        return RC_INVALID;
    }
    if (input_len == 0) {
        *out_crc = 0;
        return RC_OK;
    }

    int device_rc = ensure_cuda_device();
    if (device_rc != RC_OK) {
        return device_rc;
    }

    size_t chunk_count = (input_len + CRC64_CHUNK_SIZE - 1ULL) / CRC64_CHUNK_SIZE;
    uint64_t* host_chunks = static_cast<uint64_t*>(
        malloc(chunk_count * sizeof(uint64_t)));
    if (host_chunks == nullptr) {
        return RC_CUDA;
    }

    uint8_t* d_input = nullptr;
    uint64_t* d_chunks = nullptr;
    cudaError_t err;
    int ret = 0;
    bool free_device_buffers = true;
    dim3 block(256);
    dim3 grid(1);
    uint64_t crc = CRC64_INIT;

    err = cudaMalloc(reinterpret_cast<void**>(&d_input), input_len);
    if (err != cudaSuccess) {
        ret = RC_CUDA;
        goto cleanup;
    }
    err = cudaMalloc(reinterpret_cast<void**>(&d_chunks),
                     chunk_count * sizeof(uint64_t));
    if (err != cudaSuccess) {
        ret = RC_CUDA;
        goto cleanup;
    }

    err = cudaMemcpy(d_input, input_ptr, input_len, cudaMemcpyHostToDevice);
    if (err != cudaSuccess) {
        ret = RC_CUDA;
        goto cleanup;
    }

    grid = dim3(static_cast<unsigned int>((chunk_count + block.x - 1) / block.x));
    crc64_chunks_kernel<<<grid, block>>>(d_input, input_len, d_chunks);
    err = cudaGetLastError();
    if (err != cudaSuccess) {
        ret = RC_CUDA;
        goto cleanup;
    }
    err = wait_for_default_stream();
    if (err != cudaSuccess) {
        ret = RC_CUDA;
        free_device_buffers = err != cudaErrorLaunchTimeout;
        goto cleanup;
    }

    err = cudaMemcpy(host_chunks,
                     d_chunks,
                     chunk_count * sizeof(uint64_t),
                     cudaMemcpyDeviceToHost);
    if (err != cudaSuccess) {
        ret = RC_CUDA;
        goto cleanup;
    }

    for (size_t idx = 0; idx < chunk_count; ++idx) {
        size_t offset = idx * CRC64_CHUNK_SIZE;
        size_t remaining = input_len > offset ? input_len - offset : 0;
        size_t seg_len = (remaining < CRC64_CHUNK_SIZE) ? remaining : CRC64_CHUNK_SIZE;
        crc = crc64_combine_host(crc, host_chunks[idx], seg_len);
    }

    *out_crc = crc ^ CRC64_XOR_OUT;

cleanup:
    if (free_device_buffers && d_input) {
        cudaFree(d_input);
    }
    if (free_device_buffers && d_chunks) {
        cudaFree(d_chunks);
    }
    free(host_chunks);
    return ret;
}
