// SPDX-License-Identifier: Apache-2.0
// Minimal, deterministic CUDA backend for Goldilocks FFT / IFFT / LDE.
// Architecture requirement: SM80+ (Ampere or newer).

#include <cuda_runtime.h>
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifndef FASTPQ_CUDA_ARCH_MIN
#define FASTPQ_CUDA_ARCH_MIN 800
#endif

#if defined(__CUDA_ARCH__) && (__CUDA_ARCH__ < FASTPQ_CUDA_ARCH_MIN)
#error "fastpq_cuda requires SM80+ (compute capability 8.0 or newer)."
#endif

// -----------------------------------------------------------------------------.
// Field constants (Goldilocks prime: 2^64 - 2^32 + 1).
// -----------------------------------------------------------------------------.
static __device__ __constant__ uint64_t FIELD_MODULUS = 18446744069414584321ull;
static __device__ __constant__ uint64_t FIELD_GENERATOR = 7ull;           // primitive generator
static __device__ __constant__ uint64_t FOLD_CONST = 0x00000000FFFFFFFFull; // 2^32 - 1

#define POSEIDON_STATE_WIDTH 3u
#define POSEIDON_RATE 2u
#define POSEIDON_FULL_ROUNDS_HALF 4u
#define POSEIDON_PARTIAL_ROUNDS 57u

struct PoseidonSlice {
    uint32_t offset;
    uint32_t len;
};

static __device__ __constant__ uint64_t POSEIDON_ROUND_CONSTANTS[POSEIDON_FULL_ROUNDS_HALF * 2u + POSEIDON_PARTIAL_ROUNDS][POSEIDON_STATE_WIDTH] = {
    {0x0355defb099bed96ull, 0xfcc55fb9681355e6ull, 0x931368d725f8720aull},
    {0x3b8752f81e4e100aull, 0x53b864ee53de1e7cull, 0xfccb110a3a839800ull},
    {0xfff0e99012e32773ull, 0xfca28ab2898eff49ull, 0xb02b1c854e99b411ull},
    {0x9c45a0b2adeddcddull, 0xab6bd8d6b4959cfaull, 0x32eb07eb92becca5ull},
    {0x02698cda7aa8674aull, 0xa6914454ffbbc85aull, 0x330402af7562de69ull},
    {0x6b9deb689731670aull, 0xb6acba1824052a6eull, 0x3ad181a6aaed99ecull},
    {0x871f25345b28b8e9ull, 0xcd88d358315eeec1ull, 0xe051da577c86be1dull},
    {0x153ad1817c0c125aull, 0x97461b442e8abe8cull, 0x86591af51639f5a9ull},
    {0xb5bbc2a445ce1d3cull, 0x3cf87df379503557ull, 0x69c87ef4b926f914ull},
    {0x941bba1e4ef7e9f5ull, 0x2f9b29b58901fe31ull, 0x49c95be066d4e00bull},
    {0xcdb4fa571ed16b7eull, 0x81526ee1686c1130ull, 0x9afe7622fd187b9dull},
    {0x1c6d3d32f5666a4aull, 0xbc714a509ed0c9e2ull, 0xdbd44f433d9bb989ull},
    {0x07f9a90b7e43d95cull, 0xbf0830bc8e11be74ull, 0x97f82d7658d424afull},
    {0x232d735bd912e717ull, 0x24c5847ca7491e7aull, 0x050e45f9ad018df7ull},
    {0x3d875e313e445623ull, 0xf0e765b8e6350e3aull, 0x24e1f73aacb2a5d8ull},
    {0xa2608cac4eddcbf6ull, 0xbb84373a14012579ull, 0x153ff026357ddb2eull},
    {0x31c51990051b2fceull, 0xc4326a6df1342061ull, 0x110ab731271e081dull},
    {0xd7d4ed983f738f7dull, 0x04c2243a9d2d2dd8ull, 0xec84bd9f09f4a4eeull},
    {0x459c6a91e8310f7dull, 0xbd5448db7b312560ull, 0x55405bc1d4841791ull},
    {0x9b29aa29d32ff2efull, 0x19b4882d7d91817dull, 0xff96eeff0d35d395ull},
    {0x725bee8d2e752ebaull, 0xb778a21c64310114ull, 0x9991cf6141e2878aull},
    {0x5baf8b23f6d30359ull, 0x17d0b954a0c24e6dull, 0x3050c4078053b0d5ull},
    {0xf9f52a3090efa474ull, 0xa598d52915f99806ull, 0xbf48572b618fbd61ull},
    {0xa34bfa5c0b92ec3eull, 0xab1115736057c63cull, 0x2f7d416846014aaaull},
    {0xe19f214292d3a546ull, 0xb48ced4361042203ull, 0xb64f6bfc1cfcbd92ull},
    {0xe0f48eecd39b7cbfull, 0xc4384583c0d6450dull, 0x7460cad33f436acaull},
    {0xa69f9eff5caf7223ull, 0x82acd34e811e340aull, 0xecb3d78f9881bedfull},
    {0x976760db1c85079aull, 0xa0d527128f5231caull, 0xa8823b80503c432bull},
    {0xc41afca19eb78258ull, 0x33b30793338e26eeull, 0x3369d80a26f26384ull},
    {0x4f5237b43fe7c081ull, 0x3e2e28d3737926d1ull, 0x9e9fd9449eafc854ull},
    {0xf6dbf8e50187050full, 0x0883a7372f01a7d2ull, 0xe85c8cbe6053fa56ull},
    {0x43e66dea535dce9dull, 0xff7445db745c243bull, 0xde5d71d4d7709458ull},
    {0xb14f0f45451d99f9ull, 0xfcdf81f971206fc3ull, 0x2508429ed0bbabfaull},
    {0x408115a6f42a57e8ull, 0xfcde33569c03693dull, 0x83926cc606241abdull},
    {0xfe6191419d8e0a84ull, 0x358d71362d234258ull, 0x3bfb29c64195c104ull},
    {0x0ed6b527fe034ad7ull, 0xcd1998fe841a698aull, 0x6ab890ebbfe772a9ull},
    {0x099a748ed5415721ull, 0x254b906a5a91bc8aull, 0x4d5130a30f0cbb72ull},
    {0xd0932ea3fa64d467ull, 0xbe72249d94aea218ull, 0x06215750fd6a29c5ull},
    {0xd79031484db89846ull, 0xdd89cad959ba24a4ull, 0xdc046dfa44871254ull},
    {0xb36f522bf4af2ef6ull, 0x92796eae38632589ull, 0x9796638178a01e39ull},
    {0xbe2c6a4f43121effull, 0x0c20942ea145116bull, 0xb6d21c3af9d7af98ull},
    {0x953af8c79c3e074eull, 0xc039153065b4a5c5ull, 0x9f51afaf415b8a55ull},
    {0x6198a82927569707ull, 0xa09366e5bb6a66c7ull, 0x68e710be9451ff59ull},
    {0x9226cca1e4988dddull, 0x27e15510153c0224ull, 0x8d6a3e1fc7c197f2ull},
    {0xfaa929e25257bc60ull, 0x6b37c307ac47aa83ull, 0x907b20a35cfe4fc2ull},
    {0x50350676383d204eull, 0x446c05dd253b59e8ull, 0x0f31889e23c5d5eaull},
    {0x38ec3e1c2e667720ull, 0x026fdf27b554ff10ull, 0x1ca82d95c6fafd39ull},
    {0xddd0bc056342e191ull, 0x2d315fb44b04ac30ull, 0xfd4b193bae11b94bull},
    {0x086df800b3489093ull, 0xf30fae7604b987e2ull, 0x10a19793fe69ebe8ull},
    {0x0c77141b7edf0449ull, 0x5ea875779328deb1ull, 0x0b674c79767e421full},
    {0x4a5a726d82f3258eull, 0x09e8c7a8be47b1deull, 0xfffa2ac085276013ull},
    {0xc09139324aa9d4f9ull, 0xb51a23205924fd17ull, 0xdcc8894949e02da4ull},
    {0x227483529222ad6eull, 0x65337b2101dfc3d8ull, 0xe19dfd7c78aaacc7ull},
    {0xf1b98df340a16ca6ull, 0x18d543123d27950dull, 0xc0c16b9b6007e396ull},
    {0xb90bc3c0d0605c8cull, 0xde108ac1d6c0bdcaull, 0x5ee8196b61146783ull},
    {0x2aaba85c6bbcf12full, 0x02ff605b73843154ull, 0x2c37fc5e4dfebb1eull},
    {0xcb1ca352a46e4d33ull, 0x187d0f76fc0fb4a3ull, 0xaeadcdfe0a66b1f1ull},
    {0xc12532bfb67e3436ull, 0x4b403b0bf220033eull, 0xe0dec1015d69cd0bull},
    {0x5d3eb71f9ef30c4aull, 0xf82d9fbcdf532aebull, 0xa362551d86bebd87ull},
    {0x6823e25802851126ull, 0x189ce62b77650805ull, 0xefc261ffa36bf041ull},
    {0x478863511cbaf173ull, 0x4f4bd042c56b7936ull, 0x5b4cf8cc8584ca9aull},
    {0xcc944e5f76063e0cull, 0xd29a0b002c783ca7ull, 0x2f59efce5cde182bull},
    {0x93a0767d9186685cull, 0xee2501a761ecf4e5ull, 0xe7514fa48b145686ull},
    {0x5e0f189e8c61d97cull, 0xebe9bd9bef0f5443ull, 0xd6cfe2a76189672aull},
    {0x38ff8f40252c1ab7ull, 0x3cf9c4c3804d8166ull, 0x512f1e3ac8d0ffe5ull},
};

static __device__ __constant__ uint64_t POSEIDON_MDS[POSEIDON_STATE_WIDTH][POSEIDON_STATE_WIDTH] = {
    {0x982513a23d22b592ull, 0xa3115db8cf1d9c90ull, 0x46ba684b9eee84b7ull},
    {0xbe3dce25491db768ull, 0xfb0a6f731943519full, 0xfce5bd953cde1896ull},
    {0xe624719c41eb1a09ull, 0xd2221b0f1aa2ebc4ull, 0x1ab5e60d03ad44bcull},
};

// -----------------------------------------------------------------------------.
// Utility helpers.
// -----------------------------------------------------------------------------
__device__ __forceinline__ uint64_t d_load(const uint64_t* ptr) {
    return __ldg(ptr);
}

__device__ __forceinline__ void d_store(uint64_t* ptr, uint64_t value) {
    *ptr = value;
}

__device__ __forceinline__ uint64_t field_reduce_u128(unsigned __int128 value) {
    // Repeatedly fold high limbs using 2^64 ≡ (2^32 - 1) (mod p) until the value fits in u64.
    const unsigned __int128 fold = (unsigned __int128)FOLD_CONST;
    while ((value >> 64) != 0) {
        uint64_t lo = (uint64_t)value;
        uint64_t hi = (uint64_t)(value >> 64);
        value = (unsigned __int128)lo + (unsigned __int128)hi * fold;
    }
    uint64_t reduced = (uint64_t)value;
    uint64_t p = FIELD_MODULUS;
    if (reduced >= p) reduced -= p;
    if (reduced >= p) reduced -= p;
    return reduced;
}

__device__ __forceinline__ uint64_t f_add(uint64_t a, uint64_t b) {
    return field_reduce_u128((unsigned __int128)a + (unsigned __int128)b);
}

__device__ __forceinline__ uint64_t f_sub(uint64_t a, uint64_t b) {
    return (a >= b) ? (a - b) : (a + FIELD_MODULUS - b);
}

__device__ __forceinline__ uint64_t f_mul(uint64_t a, uint64_t b) {
    return field_reduce_u128((unsigned __int128)a * (unsigned __int128)b);
}

__device__ uint64_t f_pow(uint64_t base, uint64_t exp) {
    uint64_t acc = 1ull;
    while (exp) {
        if (exp & 1ull) {
            acc = f_mul(acc, base);
        }
        base = f_mul(base, base);
        exp >>= 1ull;
    }
    return acc;
}

__device__ __forceinline__ uint64_t f_inv(uint64_t value) {
    return f_pow(value, FIELD_MODULUS - 2ull);
}

__device__ __forceinline__ uint64_t poseidon_pow5(uint64_t x) {
    uint64_t x2 = f_mul(x, x);
    uint64_t x4 = f_mul(x2, x2);
    return f_mul(x4, x);
}

__device__ void poseidon_apply_mds(uint64_t state[POSEIDON_STATE_WIDTH]) {
    uint64_t new_state[POSEIDON_STATE_WIDTH] = {0ull, 0ull, 0ull};
    for (unsigned int row = 0; row < POSEIDON_STATE_WIDTH; ++row) {
        uint64_t acc = 0ull;
        for (unsigned int col = 0; col < POSEIDON_STATE_WIDTH; ++col) {
            acc = f_add(acc, f_mul(POSEIDON_MDS[row][col], state[col]));
        }
        new_state[row] = acc;
    }
    for (unsigned int idx = 0; idx < POSEIDON_STATE_WIDTH; ++idx) {
        state[idx] = new_state[idx];
    }
}

__device__ void poseidon_full_round(
    uint64_t state[POSEIDON_STATE_WIDTH],
    const uint64_t rc[POSEIDON_STATE_WIDTH]
) {
    for (unsigned int idx = 0; idx < POSEIDON_STATE_WIDTH; ++idx) {
        state[idx] = poseidon_pow5(f_add(state[idx], rc[idx]));
    }
    poseidon_apply_mds(state);
}

__device__ void poseidon_partial_round(
    uint64_t state[POSEIDON_STATE_WIDTH],
    const uint64_t rc[POSEIDON_STATE_WIDTH]
) {
    for (unsigned int idx = 0; idx < POSEIDON_STATE_WIDTH; ++idx) {
        state[idx] = f_add(state[idx], rc[idx]);
    }
    state[0] = poseidon_pow5(state[0]);
    poseidon_apply_mds(state);
}

__device__ void poseidon_permute(uint64_t state[POSEIDON_STATE_WIDTH]) {
    unsigned int round = 0;
    for (unsigned int i = 0; i < POSEIDON_FULL_ROUNDS_HALF; ++i, ++round) {
        poseidon_full_round(state, POSEIDON_ROUND_CONSTANTS[round]);
    }
    for (unsigned int i = 0; i < POSEIDON_PARTIAL_ROUNDS; ++i, ++round) {
        poseidon_partial_round(state, POSEIDON_ROUND_CONSTANTS[round]);
    }
    for (unsigned int i = 0; i < POSEIDON_FULL_ROUNDS_HALF; ++i, ++round) {
        poseidon_full_round(state, POSEIDON_ROUND_CONSTANTS[round]);
    }
}

__device__ __forceinline__ uint64_t bit_reverse_index(uint64_t index, uint32_t log_n) {
    return __brevll(index) >> (64 - log_n);
}

// -----------------------------------------------------------------------------.
// In-place radix-2 FFT (and inverse FFT) over a single column.
// -----------------------------------------------------------------------------
__device__ void fft_inplace_device(uint64_t* data, uint64_t len, uint32_t log_len, bool inverse) {
    for (uint64_t i = 1; i < len - 1; ++i) {
        uint64_t j = bit_reverse_index(i, log_len);
        if (i < j) {
            uint64_t tmp = data[i];
            data[i] = data[j];
            data[j] = tmp;
        }
    }

    uint64_t omega = f_pow(FIELD_GENERATOR, (FIELD_MODULUS - 1ull) >> log_len);
    if (inverse) {
        omega = f_inv(omega);
    }

    for (uint64_t size = 2; size <= len; size <<= 1) {
        uint64_t step = len / size;
        uint64_t wlen = f_pow(omega, step);
        uint64_t half = size >> 1;

        for (uint64_t start = 0; start < len; start += size) {
            uint64_t w = 1ull;
            for (uint64_t j = 0; j < half; ++j) {
                uint64_t u = data[start + j];
                uint64_t v = f_mul(data[start + j + half], w);
                data[start + j] = f_add(u, v);
                data[start + j + half] = f_sub(u, v);
                w = f_mul(w, wlen);
            }
        }
    }

    if (inverse) {
        uint64_t inv_len = f_inv((uint64_t)len);
        for (uint64_t i = 0; i < len; ++i) {
            data[i] = f_mul(data[i], inv_len);
        }
    }
}

// -----------------------------------------------------------------------------.
// Coset low-degree extension (coefficient → evaluation domain).
// -----------------------------------------------------------------------------
__device__ void lde_eval_device(
    const uint64_t* coeffs,
    uint64_t* out,
    uint64_t trace_len,
    uint64_t eval_len,
    uint32_t trace_log,
    uint32_t blowup_log,
    uint64_t coset
) {
    uint64_t omega_trace = f_pow(FIELD_GENERATOR, (FIELD_MODULUS - 1ull) >> trace_log);
    uint64_t omega_eval = f_pow(FIELD_GENERATOR, (FIELD_MODULUS - 1ull) >> (trace_log + blowup_log));

    uint64_t omega_inv = f_inv(omega_trace);

    extern __shared__ uint64_t shared[];

    for (uint64_t i = threadIdx.x; i < trace_len; i += blockDim.x) {
        shared[i] = coeffs[i];
    }
    __syncthreads();

    for (uint64_t i = trace_len; i < eval_len; ++i) {
        shared[i] = 0ull;
    }

    uint64_t* ptr = shared;
    fft_inplace_device(ptr, eval_len, trace_log + blowup_log, false);

    uint64_t coset_pow = 1ull;
    for (uint64_t i = 0; i < eval_len; ++i) {
        out[i] = f_mul(ptr[i], coset_pow);
        coset_pow = f_mul(coset_pow, coset);
    }
}

// -----------------------------------------------------------------------------.
// CUDA kernels - each column handled by a single thread for determinism.
// -----------------------------------------------------------------------------
__global__ void fastpq_fft_kernel(uint64_t* elements, uint64_t column_len, uint32_t log_len) {
    uint64_t col = blockIdx.x * blockDim.x + threadIdx.x;
    if (col >= gridDim.x * blockDim.x) {
        return;
    }
    uint64_t* column = elements + col * column_len;
    fft_inplace_device(column, column_len, log_len, false);
}

__global__ void fastpq_ifft_kernel(uint64_t* elements, uint64_t column_len, uint32_t log_len) {
    uint64_t col = blockIdx.x * blockDim.x + threadIdx.x;
    if (col >= gridDim.x * blockDim.x) {
        return;
    }
    uint64_t* column = elements + col * column_len;
    fft_inplace_device(column, column_len, log_len, true);
}

__global__ void fastpq_lde_kernel(
    const uint64_t* coeffs,
    uint64_t* out,
    uint64_t trace_len,
    uint64_t eval_len,
    uint32_t trace_log,
    uint32_t blowup_log,
    uint64_t coset
) {
    uint64_t col = blockIdx.x;
    const uint64_t* column_coeffs = coeffs + col * trace_len;
    uint64_t* out_column = out + col * eval_len;
    lde_eval_device(column_coeffs, out_column, trace_len, eval_len, trace_log, blowup_log, coset);
}

__global__ void fastpq_poseidon_permute_kernel(uint64_t* states, size_t state_count) {
    size_t idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= state_count) {
        return;
    }
    uint64_t local_state[POSEIDON_STATE_WIDTH];
    uint64_t* base = states + idx * POSEIDON_STATE_WIDTH;
    for (unsigned int i = 0; i < POSEIDON_STATE_WIDTH; ++i) {
        local_state[i] = base[i];
    }
    poseidon_permute(local_state);
    for (unsigned int i = 0; i < POSEIDON_STATE_WIDTH; ++i) {
        base[i] = local_state[i];
    }
}

__global__ void fastpq_poseidon_hash_columns_kernel(
    const uint64_t* payloads,
    const PoseidonSlice* slices,
    size_t block_count,
    size_t column_count,
    uint64_t* out_states
) {
    size_t col = blockIdx.x * blockDim.x + threadIdx.x;
    if (col >= column_count) {
        return;
    }
    PoseidonSlice slice = slices[col];
    const uint64_t* column_payload = payloads + (size_t)slice.offset;
    uint64_t local_state[POSEIDON_STATE_WIDTH] = {0ull, 0ull, 0ull};
    for (size_t block = 0; block < block_count; ++block) {
        const uint64_t* block_ptr = column_payload + block * POSEIDON_RATE;
        for (unsigned int lane = 0; lane < POSEIDON_RATE; ++lane) {
            local_state[lane] = f_add(local_state[lane], block_ptr[lane]);
        }
        poseidon_permute(local_state);
    }
    uint64_t* out = out_states + col * POSEIDON_STATE_WIDTH;
    for (unsigned int lane = 0; lane < POSEIDON_STATE_WIDTH; ++lane) {
        out[lane] = local_state[lane];
    }
}

__global__ void fastpq_poseidon_leaf_extract_kernel(
    const uint64_t* states,
    size_t column_count,
    uint64_t* out_hashes
) {
    size_t idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= column_count) {
        return;
    }
    out_hashes[idx] = states[idx * POSEIDON_STATE_WIDTH];
}

__global__ void fastpq_poseidon_parent_kernel(
    const uint64_t* leaf_hashes,
    size_t column_count,
    uint64_t* parent_hashes
) {
    size_t parent_idx = blockIdx.x * blockDim.x + threadIdx.x;
    size_t parent_count = (column_count + 1) / 2;
    if (parent_idx >= parent_count) {
        return;
    }
    size_t left = parent_idx * 2;
    size_t right = left + 1;
    if (right >= column_count) {
        right = column_count - 1;
    }
    uint64_t state[POSEIDON_STATE_WIDTH] = {
        leaf_hashes[left],
        leaf_hashes[right],
        0ull,
    };
    poseidon_permute(state);
    parent_hashes[parent_idx] = state[0];
}

// -----------------------------------------------------------------------------.
// Public C interface.
// -----------------------------------------------------------------------------
extern "C" cudaError_t fastpq_fft_cuda(
    uint64_t* elements,
    size_t column_count,
    uint32_t log_size
) {
    if (elements == nullptr || column_count == 0) {
        return cudaErrorInvalidValue;
    }
    uint64_t n = 1ull << log_size;
    dim3 block_dim(1);
    dim3 grid_dim((unsigned int)column_count);
    fastpq_fft_kernel<<<grid_dim, block_dim>>>(elements, n, log_size);
    return cudaGetLastError();
}

extern "C" cudaError_t fastpq_ifft_cuda(
    uint64_t* elements,
    size_t column_count,
    uint32_t log_size
) {
    if (elements == nullptr || column_count == 0) {
        return cudaErrorInvalidValue;
    }
    uint64_t n = 1ull << log_size;
    dim3 block_dim(1);
    dim3 grid_dim((unsigned int)column_count);
    fastpq_ifft_kernel<<<grid_dim, block_dim>>>(elements, n, log_size);
    return cudaGetLastError();
}

extern "C" cudaError_t fastpq_lde_cuda(
    const uint64_t* coeffs,
    size_t column_count,
    uint32_t trace_log,
    uint32_t blowup_log,
    uint64_t coset,
    uint64_t* out
) {
    if (coeffs == nullptr || out == nullptr || column_count == 0) {
        return cudaErrorInvalidValue;
    }
    uint64_t trace_len = 1ull << trace_log;
    uint64_t eval_len = 1ull << (trace_log + blowup_log);
    dim3 grid_dim((unsigned int)column_count);
    dim3 block_dim(1);
    size_t shared_bytes = eval_len * sizeof(uint64_t);
    fastpq_lde_kernel<<<grid_dim, block_dim, shared_bytes>>>(
        coeffs,
        out,
        trace_len,
        eval_len,
        trace_log,
        blowup_log,
        coset
    );
    return cudaGetLastError();
}

extern "C" cudaError_t fastpq_poseidon_permute_cuda(uint64_t* states, size_t state_count) {
    if (states == nullptr || state_count == 0) {
        return cudaErrorInvalidValue;
    }

    const size_t total_elements = state_count * (size_t)POSEIDON_STATE_WIDTH;
    const size_t byte_len = total_elements * sizeof(uint64_t);

    uint64_t* device_states = nullptr;
    cudaError_t status = cudaMalloc(&device_states, byte_len);
    if (status != cudaSuccess) {
        return status;
    }

    status = cudaMemcpy(device_states, states, byte_len, cudaMemcpyHostToDevice);
    if (status != cudaSuccess) {
        cudaFree(device_states);
        return status;
    }

    const unsigned int threads_per_block = 128u;
    size_t grid_size_host = (state_count + threads_per_block - 1u) / threads_per_block;
    if (grid_size_host == 0) {
        grid_size_host = 1;
    }
    if (grid_size_host > (size_t)UINT32_MAX) {
        cudaFree(device_states);
        return cudaErrorInvalidValue;
    }

    fastpq_poseidon_permute_kernel<<<(unsigned int)grid_size_host, threads_per_block>>>(
        device_states,
        state_count
    );
    status = cudaGetLastError();
    if (status == cudaSuccess) {
        status = cudaDeviceSynchronize();
    }
    if (status == cudaSuccess) {
        status = cudaMemcpy(states, device_states, byte_len, cudaMemcpyDeviceToHost);
    }

    cudaError_t free_status = cudaFree(device_states);
    if (status == cudaSuccess && free_status != cudaSuccess) {
        status = free_status;
    }
    return status;
}

extern "C" cudaError_t fastpq_poseidon_hash_columns_cuda(
    const uint64_t* payloads,
    const PoseidonSlice* slices,
    size_t column_count,
    size_t block_count,
    uint64_t* out_states
) {
    if (
        payloads == nullptr || slices == nullptr || out_states == nullptr || column_count == 0
        || block_count == 0
    ) {
        return cudaErrorInvalidValue;
    }
    size_t state_len = column_count * (size_t)POSEIDON_STATE_WIDTH;
    size_t state_bytes = state_len * sizeof(uint64_t);
    size_t payload_bytes = 0;
    if (column_count > 0) {
        PoseidonSlice last = slices[column_count - 1];
        payload_bytes = ((size_t)last.offset + (size_t)last.len) * sizeof(uint64_t);
    }

    uint64_t* device_payloads = nullptr;
    PoseidonSlice* device_slices = nullptr;
    uint64_t* device_states = nullptr;
    cudaError_t status = cudaMalloc(&device_payloads, payload_bytes);
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMalloc(&device_slices, column_count * sizeof(PoseidonSlice));
    if (status != cudaSuccess) {
        cudaFree(device_payloads);
        return status;
    }
    status = cudaMalloc(&device_states, state_bytes);
    if (status != cudaSuccess) {
        cudaFree(device_slices);
        cudaFree(device_payloads);
        return status;
    }

    status = cudaMemcpy(device_payloads, payloads, payload_bytes, cudaMemcpyHostToDevice);
    if (status != cudaSuccess) {
        cudaFree(device_states);
        cudaFree(device_slices);
        cudaFree(device_payloads);
        return status;
    }
    status = cudaMemcpy(
        device_slices,
        slices,
        column_count * sizeof(PoseidonSlice),
        cudaMemcpyHostToDevice
    );
    if (status != cudaSuccess) {
        cudaFree(device_states);
        cudaFree(device_slices);
        cudaFree(device_payloads);
        return status;
    }

    const unsigned int threads_per_block = 128u;
    size_t grid_size_host = (column_count + threads_per_block - 1u) / threads_per_block;
    if (grid_size_host == 0) {
        grid_size_host = 1;
    }
    if (grid_size_host > (size_t)UINT32_MAX) {
        cudaFree(device_states);
        cudaFree(device_slices);
        cudaFree(device_payloads);
        return cudaErrorInvalidValue;
    }

    fastpq_poseidon_hash_columns_kernel<<<(unsigned int)grid_size_host, threads_per_block>>>(
        device_payloads,
        device_slices,
        block_count,
        column_count,
        device_states
    );
    status = cudaGetLastError();
    if (status == cudaSuccess) {
        status = cudaDeviceSynchronize();
    }
    if (status == cudaSuccess) {
        status = cudaMemcpy(out_states, device_states, state_bytes, cudaMemcpyDeviceToHost);
    }

    cudaError_t payload_status = cudaFree(device_payloads);
    cudaError_t slice_status = cudaFree(device_slices);
    cudaError_t state_status = cudaFree(device_states);
    if (status == cudaSuccess && payload_status != cudaSuccess) {
        status = payload_status;
    }
    if (status == cudaSuccess && slice_status != cudaSuccess) {
        status = slice_status;
    }
    if (status == cudaSuccess && state_status != cudaSuccess) {
        status = state_status;
    }
    return status;
}

extern "C" cudaError_t fastpq_poseidon_hash_columns_fused_cuda(
    const uint64_t* payloads,
    const PoseidonSlice* slices,
    size_t column_count,
    size_t block_count,
    uint64_t* out_hashes
) {
    if (
        payloads == nullptr || slices == nullptr || out_hashes == nullptr || column_count == 0
        || block_count == 0
    ) {
        return cudaErrorInvalidValue;
    }
    size_t parents = (column_count + 1) / 2;
    size_t payload_bytes = 0;
    if (column_count > 0) {
        PoseidonSlice last = slices[column_count - 1];
        payload_bytes = ((size_t)last.offset + (size_t)last.len) * sizeof(uint64_t);
    }
    size_t state_bytes = column_count * (size_t)POSEIDON_STATE_WIDTH * sizeof(uint64_t);
    size_t slice_bytes = column_count * sizeof(PoseidonSlice);
    size_t hash_bytes = (column_count + parents) * sizeof(uint64_t);

    uint64_t* device_payloads = nullptr;
    PoseidonSlice* device_slices = nullptr;
    uint64_t* device_states = nullptr;
    uint64_t* device_hashes = nullptr;

    cudaError_t status = cudaMalloc(&device_payloads, payload_bytes);
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMalloc(&device_slices, slice_bytes);
    if (status != cudaSuccess) {
        cudaFree(device_payloads);
        return status;
    }
    status = cudaMalloc(&device_states, state_bytes);
    if (status != cudaSuccess) {
        cudaFree(device_slices);
        cudaFree(device_payloads);
        return status;
    }
    status = cudaMalloc(&device_hashes, hash_bytes);
    if (status != cudaSuccess) {
        cudaFree(device_states);
        cudaFree(device_slices);
        cudaFree(device_payloads);
        return status;
    }

    status = cudaMemcpy(device_payloads, payloads, payload_bytes, cudaMemcpyHostToDevice);
    if (status != cudaSuccess) {
        goto fused_cleanup;
    }
    status = cudaMemcpy(
        device_slices,
        slices,
        slice_bytes,
        cudaMemcpyHostToDevice
    );
    if (status != cudaSuccess) {
        goto fused_cleanup;
    }

    const unsigned int threads_per_block = 128u;
    size_t grid_size_host = (column_count + threads_per_block - 1u) / threads_per_block;
    if (grid_size_host == 0) {
        grid_size_host = 1;
    }
    fastpq_poseidon_hash_columns_kernel<<<(unsigned int)grid_size_host, threads_per_block>>>(
        device_payloads,
        device_slices,
        block_count,
        column_count,
        device_states
    );
    status = cudaGetLastError();
    if (status == cudaSuccess) {
        status = cudaDeviceSynchronize();
    }
    if (status != cudaSuccess) {
        goto fused_cleanup;
    }

    fastpq_poseidon_leaf_extract_kernel<<<(unsigned int)grid_size_host, threads_per_block>>>(
        device_states,
        column_count,
        device_hashes
    );
    status = cudaGetLastError();
    if (status == cudaSuccess) {
        status = cudaDeviceSynchronize();
    }
    if (status != cudaSuccess) {
        goto fused_cleanup;
    }

    size_t parent_grid = (parents + threads_per_block - 1u) / threads_per_block;
    if (parent_grid == 0) {
        parent_grid = 1;
    }
    fastpq_poseidon_parent_kernel<<<(unsigned int)parent_grid, threads_per_block>>>(
        device_hashes,
        column_count,
        device_hashes + column_count
    );
    status = cudaGetLastError();
    if (status == cudaSuccess) {
        status = cudaDeviceSynchronize();
    }
    if (status != cudaSuccess) {
        goto fused_cleanup;
    }

    status = cudaMemcpy(out_hashes, device_hashes, hash_bytes, cudaMemcpyDeviceToHost);

fused_cleanup:
    cudaError_t free_hashes = cudaFree(device_hashes);
    cudaError_t free_states = cudaFree(device_states);
    cudaError_t free_slices = cudaFree(device_slices);
    cudaError_t free_payloads = cudaFree(device_payloads);
    if (status == cudaSuccess) {
        if (free_hashes != cudaSuccess) {
            status = free_hashes;
        } else if (free_states != cudaSuccess) {
            status = free_states;
        } else if (free_slices != cudaSuccess) {
            status = free_slices;
        } else if (free_payloads != cudaSuccess) {
            status = free_payloads;
        }
    } else {
        if (free_hashes != cudaSuccess) {
            status = free_hashes;
        }
        if (free_states != cudaSuccess) {
            status = free_states;
        }
        if (free_slices != cudaSuccess) {
            status = free_slices;
        }
        if (free_payloads != cudaSuccess) {
            status = free_payloads;
        }
    }
    return status;
}
