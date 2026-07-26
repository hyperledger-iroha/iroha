#include <stdint.h>

#define FIELD_LIMBS 4

struct FieldElem {
    uint64_t limbs[FIELD_LIMBS];
};

__device__ __constant__ uint64_t BN254_MOD[FIELD_LIMBS] = {
    0x43e1f593f0000001ULL,
    0x2833e84879b97091ULL,
    0xb85045b68181585dULL,
    0x30644e72e131a029ULL,
};

__device__ __constant__ uint64_t TWO_TO_256_MOD[FIELD_LIMBS] = {
    0xac96341c4ffffffbULL,
    0x36fc76959f60cd29ULL,
    0x666ea36f7879462eULL,
    0x0e0a77c19a07df2fULL,
};

__device__ __forceinline__ FieldElem field_zero() {
    FieldElem out;
#pragma unroll
    for (int i = 0; i < FIELD_LIMBS; ++i) {
        out.limbs[i] = 0ULL;
    }
    return out;
}

__device__ __forceinline__ FieldElem load_field(const uint64_t* src) {
    FieldElem out;
#pragma unroll
    for (int i = 0; i < FIELD_LIMBS; ++i) {
        out.limbs[i] = src[i];
    }
    return out;
}

__device__ __forceinline__ void store_field(uint64_t* dst, const FieldElem& value) {
#pragma unroll
    for (int i = 0; i < FIELD_LIMBS; ++i) {
        dst[i] = value.limbs[i];
    }
}

__device__ __forceinline__ bool field_ge_mod(const FieldElem& a) {
    for (int i = FIELD_LIMBS - 1; i >= 0; --i) {
        if (a.limbs[i] > BN254_MOD[i]) {
            return true;
        }
        if (a.limbs[i] < BN254_MOD[i]) {
            return false;
        }
    }
    return true;
}

__device__ __forceinline__ FieldElem field_sub_const(const FieldElem& a, const uint64_t* b) {
    FieldElem out;
    unsigned long long borrow = 0ULL;
#pragma unroll
    for (int i = 0; i < FIELD_LIMBS; ++i) {
        unsigned __int128 lhs = (unsigned __int128)a.limbs[i];
        unsigned __int128 rhs = (unsigned __int128)b[i] + borrow;
        unsigned __int128 diff = lhs - rhs;
        out.limbs[i] = static_cast<uint64_t>(diff);
        borrow = lhs < rhs ? 1ULL : 0ULL;
    }
    if (borrow != 0ULL) {
        unsigned __int128 carry = 0;
#pragma unroll
        for (int i = 0; i < FIELD_LIMBS; ++i) {
            unsigned __int128 sum = (unsigned __int128)out.limbs[i] + BN254_MOD[i] + carry;
            out.limbs[i] = static_cast<uint64_t>(sum);
            carry = sum >> 64;
        }
    }
    return out;
}

__device__ __forceinline__ FieldElem field_add(const FieldElem& a, const FieldElem& b) {
    FieldElem out;
    unsigned __int128 carry = 0;
#pragma unroll
    for (int i = 0; i < FIELD_LIMBS; ++i) {
        unsigned __int128 sum = (unsigned __int128)a.limbs[i] + b.limbs[i] + carry;
        out.limbs[i] = static_cast<uint64_t>(sum);
        carry = sum >> 64;
    }
    if (carry != 0 || field_ge_mod(out)) {
        out = field_sub_const(out, BN254_MOD);
    }
    return out;
}

__device__ __forceinline__ FieldElem field_sub(const FieldElem& a, const FieldElem& b) {
    FieldElem out;
    unsigned long long borrow = 0ULL;
#pragma unroll
    for (int i = 0; i < FIELD_LIMBS; ++i) {
        unsigned __int128 lhs = (unsigned __int128)a.limbs[i];
        unsigned __int128 rhs = (unsigned __int128)b.limbs[i] + borrow;
        unsigned __int128 diff = lhs - rhs;
        out.limbs[i] = static_cast<uint64_t>(diff);
        borrow = lhs < rhs ? 1ULL : 0ULL;
    }
    if (borrow != 0ULL) {
        unsigned __int128 carry = 0;
#pragma unroll
        for (int i = 0; i < FIELD_LIMBS; ++i) {
            unsigned __int128 sum = (unsigned __int128)out.limbs[i] + BN254_MOD[i] + carry;
            out.limbs[i] = static_cast<uint64_t>(sum);
            carry = sum >> 64;
        }
    }
    return out;
}

__device__ __forceinline__ FieldElem field_reduce_256(FieldElem value) {
    FieldElem out = value;
    while (field_ge_mod(out)) {
        out = field_sub_const(out, BN254_MOD);
    }
    return out;
}

__device__ __forceinline__ FieldElem field_double(const FieldElem& a) {
    return field_add(a, a);
}

__device__ __forceinline__ void mul_wide(const FieldElem& a, const FieldElem& b, uint64_t out[8]) {
    uint64_t tmp[8] = {0ULL, 0ULL, 0ULL, 0ULL, 0ULL, 0ULL, 0ULL, 0ULL};
#pragma unroll
    for (int i = 0; i < FIELD_LIMBS; ++i) {
        unsigned __int128 carry = 0;
#pragma unroll
        for (int j = 0; j < FIELD_LIMBS; ++j) {
            unsigned __int128 prod = (unsigned __int128)a.limbs[i] * b.limbs[j];
            unsigned __int128 sum = prod + tmp[i + j] + carry;
            tmp[i + j] = static_cast<uint64_t>(sum);
            carry = sum >> 64;
        }
        int k = i + FIELD_LIMBS;
        while (carry != 0 && k < 8) {
            unsigned __int128 sum = (unsigned __int128)tmp[k] + carry;
            tmp[k] = static_cast<uint64_t>(sum);
            carry = sum >> 64;
            ++k;
        }
    }
#pragma unroll
    for (int i = 0; i < 8; ++i) {
        out[i] = tmp[i];
    }
}

__device__ __forceinline__ FieldElem reduce_limbs(const uint64_t limbs[8]) {
    FieldElem result = load_field(limbs);
    result = field_reduce_256(result);
    FieldElem factor = load_field(TWO_TO_256_MOD);
    for (int limb = 4; limb < 8; ++limb) {
        uint64_t bits = limbs[limb];
        for (int bit = 0; bit < 64; ++bit) {
            if (bits & 1ULL) {
                result = field_add(result, factor);
            }
            bits >>= 1U;
            factor = field_double(factor);
        }
    }
    return result;
}

__device__ __forceinline__ FieldElem field_mul(const FieldElem& a, const FieldElem& b) {
    uint64_t wide[8];
    mul_wide(a, b, wide);
    return reduce_limbs(wide);
}

extern "C" __global__ void bn254_add_kernel(
    const uint64_t* lhs,
    const uint64_t* rhs,
    uint64_t* out,
    uint32_t elem_count,
    uint32_t stride_words
) {
    uint32_t idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= elem_count) {
        return;
    }
    size_t stride = static_cast<size_t>(stride_words);
    const uint64_t* lhs_ptr = lhs + stride * idx;
    const uint64_t* rhs_ptr = rhs + stride * idx;
    uint64_t* out_ptr = out + stride * idx;
    FieldElem a = load_field(lhs_ptr);
    FieldElem b = load_field(rhs_ptr);
    FieldElem sum = field_add(a, b);
    store_field(out_ptr, sum);
}

extern "C" __global__ void bn254_sub_kernel(
    const uint64_t* lhs,
    const uint64_t* rhs,
    uint64_t* out,
    uint32_t elem_count,
    uint32_t stride_words
) {
    uint32_t idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= elem_count) {
        return;
    }
    size_t stride = static_cast<size_t>(stride_words);
    const uint64_t* lhs_ptr = lhs + stride * idx;
    const uint64_t* rhs_ptr = rhs + stride * idx;
    uint64_t* out_ptr = out + stride * idx;
    FieldElem a = load_field(lhs_ptr);
    FieldElem b = load_field(rhs_ptr);
    FieldElem diff = field_sub(a, b);
    store_field(out_ptr, diff);
}

extern "C" __global__ void bn254_mul_kernel(
    const uint64_t* lhs,
    const uint64_t* rhs,
    uint64_t* out,
    uint32_t elem_count,
    uint32_t stride_words
) {
    uint32_t idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= elem_count) {
        return;
    }
    size_t stride = static_cast<size_t>(stride_words);
    const uint64_t* lhs_ptr = lhs + stride * idx;
    const uint64_t* rhs_ptr = rhs + stride * idx;
    uint64_t* out_ptr = out + stride * idx;
    FieldElem a = load_field(lhs_ptr);
    FieldElem b = load_field(rhs_ptr);
    FieldElem prod = field_mul(a, b);
    store_field(out_ptr, prod);
}
