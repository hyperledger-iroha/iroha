#include <cuda.h>
#include <cuda_runtime.h>
#include <stdint.h>
#include <stddef.h>
#include <stdlib.h>

// CRC64-XZ polynomial (reflected ECMA) and helper constants.
static constexpr uint64_t CRC64_POLY = 0xC96C5795D7870F42ULL;
static constexpr uint64_t CRC64_INIT = 0xFFFFFFFFFFFFFFFFULL;
static constexpr uint64_t CRC64_XOR_OUT = 0xFFFFFFFFFFFFFFFFULL;
static constexpr uint64_t CRC64_CHUNK_SIZE = 16ULL * 1024ULL;

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

    uint64_t crc = CRC64_INIT;
    for (size_t idx = 0; idx < chunk_count; ++idx) {
        size_t offset = idx * CRC64_CHUNK_SIZE;
        size_t remaining = input_len > offset ? input_len - offset : 0;
        size_t seg_len = (remaining < CRC64_CHUNK_SIZE) ? remaining : CRC64_CHUNK_SIZE;
        crc = crc64_combine_host(crc, host_chunks[idx], seg_len);
    }

    *out_crc = crc ^ CRC64_XOR_OUT;

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
