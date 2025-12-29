#include <stdint.h>
#include <cuda_runtime.h>

#define FIELD_LIMBS 4
#define MAX_STATE_WIDTH 6

enum StatusCode : uint32_t {
    STATUS_OK = 0,
    STATUS_ERR_WIDTH = 1,
    STATUS_ERR_STRIDE = 2,
    STATUS_ERR_ROUNDS = 3,
};

struct FieldElem {
    uint64_t limbs[FIELD_LIMBS];
};

struct KernelStatus {
    uint32_t code;
    uint32_t detail;
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

__device__ __forceinline__ void report_error(
    KernelStatus* status_ptr,
    uint32_t code,
    uint32_t detail
) {
    if (status_ptr == nullptr || code == STATUS_OK) {
        return;
    }
    unsigned int* code_slot = reinterpret_cast<unsigned int*>(&status_ptr->code);
    unsigned int prev = atomicCAS(code_slot, 0u, code);
    if (prev == 0u) {
        status_ptr->detail = detail;
    }
}

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

__device__ __forceinline__ FieldElem field_from_u64(uint64_t v) {
    FieldElem out = field_zero();
    out.limbs[0] = v;
    return out;
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

__device__ __forceinline__ FieldElem field_pow5(const FieldElem& x) {
    FieldElem x2 = field_mul(x, x);
    FieldElem x4 = field_mul(x2, x2);
    return field_mul(x4, x);
}

__device__ __forceinline__ void apply_mds(
    const uint64_t* mds,
    FieldElem state[MAX_STATE_WIDTH],
    uint32_t width
) {
    FieldElem acc[MAX_STATE_WIDTH];
    for (uint32_t row = 0; row < width; ++row) {
        FieldElem sum = field_zero();
        for (uint32_t col = 0; col < width; ++col) {
            const uint64_t* coeff_ptr =
                mds + (row * width + col) * FIELD_LIMBS;
            FieldElem coeff = load_field(coeff_ptr);
            FieldElem prod = field_mul(coeff, state[col]);
            sum = field_add(sum, prod);
        }
        acc[row] = sum;
    }
    for (uint32_t i = 0; i < width; ++i) {
        state[i] = acc[i];
    }
}

__device__ bool poseidon_permute(
    uint64_t* state_ptr,
    uint32_t state_stride_words,
    uint32_t width,
    uint32_t domain_tag,
    const uint64_t* round_constants,
    const uint64_t* mds_matrix,
    uint32_t full_rounds,
    uint32_t partial_rounds,
    KernelStatus* status_ptr
) {
    if (width == 0 || width > MAX_STATE_WIDTH) {
        report_error(status_ptr, STATUS_ERR_WIDTH, width);
        return false;
    }
    uint32_t required_stride = width * FIELD_LIMBS;
    if (state_stride_words < required_stride) {
        report_error(status_ptr, STATUS_ERR_STRIDE, state_stride_words);
        return false;
    }
    if (full_rounds == 0 || (full_rounds & 1U)) {
        report_error(status_ptr, STATUS_ERR_ROUNDS, full_rounds);
        return false;
    }
    uint32_t total_rounds = full_rounds + partial_rounds;
    if (total_rounds == 0) {
        report_error(status_ptr, STATUS_ERR_ROUNDS, total_rounds);
        return false;
    }

    FieldElem state[MAX_STATE_WIDTH];
    for (uint32_t lane = 0; lane < width; ++lane) {
        uint64_t* lane_ptr = state_ptr + lane * FIELD_LIMBS;
        state[lane] = load_field(lane_ptr);
    }
    for (uint32_t lane = width; lane < MAX_STATE_WIDTH; ++lane) {
        state[lane] = field_zero();
    }

    if (domain_tag != 0) {
        FieldElem tag = field_from_u64(domain_tag);
        state[width - 1] = field_add(state[width - 1], tag);
    }

    uint32_t rf_half = full_rounds / 2;

    for (uint32_t round = 0; round < rf_half; ++round) {
        for (uint32_t lane = 0; lane < width; ++lane) {
            const uint64_t* rc_ptr =
                round_constants + (round * width + lane) * FIELD_LIMBS;
            FieldElem rc = load_field(rc_ptr);
            FieldElem sum = field_add(state[lane], rc);
            state[lane] = field_pow5(sum);
        }
        apply_mds(mds_matrix, state, width);
    }

    for (uint32_t round = 0; round < partial_rounds; ++round) {
        uint32_t idx_round = rf_half + round;
        for (uint32_t lane = 0; lane < width; ++lane) {
            const uint64_t* rc_ptr =
                round_constants + (idx_round * width + lane) * FIELD_LIMBS;
            FieldElem rc = load_field(rc_ptr);
            state[lane] = field_add(state[lane], rc);
        }
        state[0] = field_pow5(state[0]);
        apply_mds(mds_matrix, state, width);
    }

    for (uint32_t round = 0; round < rf_half; ++round) {
        uint32_t idx_round = rf_half + partial_rounds + round;
        for (uint32_t lane = 0; lane < width; ++lane) {
            const uint64_t* rc_ptr =
                round_constants + (idx_round * width + lane) * FIELD_LIMBS;
            FieldElem rc = load_field(rc_ptr);
            FieldElem sum = field_add(state[lane], rc);
            state[lane] = field_pow5(sum);
        }
        apply_mds(mds_matrix, state, width);
    }

    for (uint32_t lane = 0; lane < width; ++lane) {
        uint64_t* lane_ptr = state_ptr + lane * FIELD_LIMBS;
        for (int limb = 0; limb < FIELD_LIMBS; ++limb) {
            lane_ptr[limb] = state[lane].limbs[limb];
        }
    }
    return true;
}

extern "C" __global__ void poseidon2_permute_kernel(
    uint64_t* states,
    uint32_t state_stride_words,
    uint32_t batch_len,
    uint32_t domain_tag,
    const uint64_t* round_constants,
    const uint64_t* mds_matrix,
    uint32_t full_rounds,
    uint32_t partial_rounds,
    KernelStatus* status_out
) {
    uint32_t idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx == 0 && status_out != nullptr) {
        status_out[0].code = STATUS_OK;
        status_out[0].detail = 0;
    }
    if (idx >= batch_len) {
        return;
    }
    uint64_t* state_ptr = states + (size_t)idx * state_stride_words;
    poseidon_permute(
        state_ptr,
        state_stride_words,
        3,
        domain_tag,
        round_constants,
        mds_matrix,
        full_rounds,
        partial_rounds,
        status_out
    );
}

extern "C" __global__ void poseidon6_permute_kernel(
    uint64_t* states,
    uint32_t state_stride_words,
    uint32_t batch_len,
    uint32_t domain_tag,
    const uint64_t* round_constants,
    const uint64_t* mds_matrix,
    uint32_t full_rounds,
    uint32_t partial_rounds,
    KernelStatus* status_out
) {
    uint32_t idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx == 0 && status_out != nullptr) {
        status_out[0].code = STATUS_OK;
        status_out[0].detail = 0;
    }
    if (idx >= batch_len) {
        return;
    }
    uint64_t* state_ptr = states + (size_t)idx * state_stride_words;
    poseidon_permute(
        state_ptr,
        state_stride_words,
        6,
        domain_tag,
        round_constants,
        mds_matrix,
        full_rounds,
        partial_rounds,
        status_out
    );
}
