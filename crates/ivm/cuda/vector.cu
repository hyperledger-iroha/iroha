#include <stdint.h>

__device__ __forceinline__ unsigned int global_idx() {
    return blockIdx.x * blockDim.x + threadIdx.x;
}

extern "C" __global__ void vadd32(const uint32_t* a, const uint32_t* b, uint32_t* out, unsigned int n) {
    unsigned int idx = global_idx();
    if (idx < n) {
        out[idx] = a[idx] + b[idx];
    }
}

extern "C" __global__ void vadd64(const uint64_t* a, const uint64_t* b, uint64_t* out, unsigned int n) {
    unsigned int idx = global_idx();
    if (idx < n) {
        out[idx] = a[idx] + b[idx];
    }
}

extern "C" __global__ void vand(const uint32_t* a, const uint32_t* b, uint32_t* out, unsigned int n) {
    unsigned int idx = global_idx();
    if (idx < n) {
        out[idx] = a[idx] & b[idx];
    }
}

extern "C" __global__ void vxor(const uint32_t* a, const uint32_t* b, uint32_t* out, unsigned int n) {
    unsigned int idx = global_idx();
    if (idx < n) {
        out[idx] = a[idx] ^ b[idx];
    }
}

extern "C" __global__ void vor(const uint32_t* a, const uint32_t* b, uint32_t* out, unsigned int n) {
    unsigned int idx = global_idx();
    if (idx < n) {
        out[idx] = a[idx] | b[idx];
    }
}
