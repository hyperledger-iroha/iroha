#include <cuda.h>
#include <cuda_runtime.h>
#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>

// CRC64-ECMA polynomial and helper constants.
static constexpr uint64_t CRC64_POLY = 0x42F0E1EBA9EA3693ULL;
static constexpr uint64_t CRC64_TOPBIT = 0x8000000000000000ULL;
static constexpr uint64_t CRC64_CHUNK_SIZE = 16ULL * 1024ULL;

__device__ __forceinline__ uint64_t crc64_update(uint64_t crc, uint8_t byte) {
    crc ^= (uint64_t)byte << 56;
    #pragma unroll
    for (int i = 0; i < 8; ++i) {
        uint64_t carry = crc & CRC64_TOPBIT;
        crc <<= 1;
        if (carry != 0) {
            crc ^= CRC64_POLY;
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

static uint64_t crc64_combine_host(uint64_t crc1, uint64_t crc2, size_t len2) {
    if (len2 == 0) {
        return crc1;
    }

    uint64_t odd[64];
    uint64_t even[64];

    odd[0] = CRC64_POLY;
    for (int n = 1; n < 64; ++n) {
        odd[n] = 1ULL << (63 - n);
    }

    gf2_matrix_square(even, odd);
    size_t len = len2 * 8ULL;
    while (len != 0) {
        gf2_matrix_square(odd, even);
        if (len & 1ULL) {
            crc1 = gf2_matrix_times(odd, crc1);
        }
        len >>= 1;
        if (len == 0) {
            break;
        }
        gf2_matrix_square(even, odd);
        if (len & 1ULL) {
            crc1 = gf2_matrix_times(even, crc1);
        }
        len >>= 1;
    }

    return crc1 ^ crc2;
}

extern "C" int norito_crc64_cuda_impl(const uint8_t* input_ptr,
                                      size_t input_len,
                                      uint64_t* out_crc) {
    if (input_ptr == nullptr || out_crc == nullptr) {
        return 1;
    }
    if (input_len == 0) {
        *out_crc = 0;
        return 0;
    }

    int device_count = 0;
    if (cudaGetDeviceCount(&device_count) != cudaSuccess || device_count == 0) {
        return 2;
    }

    size_t chunk_count = (input_len + CRC64_CHUNK_SIZE - 1ULL) / CRC64_CHUNK_SIZE;
    uint64_t* host_chunks = static_cast<uint64_t*>(
        malloc(chunk_count * sizeof(uint64_t)));
    if (host_chunks == nullptr) {
        return 3;
    }

    uint8_t* d_input = nullptr;
    uint64_t* d_chunks = nullptr;
    cudaError_t err;
    int ret = 0;

    err = cudaMalloc(reinterpret_cast<void**>(&d_input), input_len);
    if (err != cudaSuccess) {
        ret = 4;
        goto cleanup;
    }
    err = cudaMalloc(reinterpret_cast<void**>(&d_chunks),
                     chunk_count * sizeof(uint64_t));
    if (err != cudaSuccess) {
        ret = 4;
        goto cleanup;
    }

    err = cudaMemcpy(d_input, input_ptr, input_len, cudaMemcpyHostToDevice);
    if (err != cudaSuccess) {
        ret = 4;
        goto cleanup;
    }

    dim3 block(256);
    dim3 grid(static_cast<unsigned int>((chunk_count + block.x - 1) / block.x));
    crc64_chunks_kernel<<<grid, block>>>(d_input, input_len, d_chunks);
    err = cudaDeviceSynchronize();
    if (err != cudaSuccess) {
        ret = 4;
        goto cleanup;
    }

    err = cudaMemcpy(host_chunks,
                     d_chunks,
                     chunk_count * sizeof(uint64_t),
                     cudaMemcpyDeviceToHost);
    if (err != cudaSuccess) {
        ret = 4;
        goto cleanup;
    }

    uint64_t crc = 0;
    for (size_t idx = 0; idx < chunk_count; ++idx) {
        size_t offset = idx * CRC64_CHUNK_SIZE;
        size_t remaining = input_len > offset ? input_len - offset : 0;
        size_t seg_len = (remaining < CRC64_CHUNK_SIZE) ? remaining : CRC64_CHUNK_SIZE;
        crc = crc64_combine_host(crc, host_chunks[idx], seg_len);
    }

    *out_crc = crc;

cleanup:
    if (d_input) {
        cudaFree(d_input);
    }
    if (d_chunks) {
        cudaFree(d_chunks);
    }
    free(host_chunks);
    return ret;
}
