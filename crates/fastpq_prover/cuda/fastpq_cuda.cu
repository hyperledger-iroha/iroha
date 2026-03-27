// SPDX-License-Identifier: Apache-2.0
// Minimal, deterministic CUDA backend for Goldilocks FFT / IFFT / LDE.
// Architecture requirement: SM80+ (Ampere or newer).

#include <cuda_runtime.h>
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#include <mutex>
#include <new>
#include <string.h>
#include <unordered_map>
#include <vector>

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
static __device__ __constant__ uint64_t FOLD_CONST = 0x00000000FFFFFFFFull; // 2^32 - 1
static __device__ __constant__ uint64_t TRACE_NODE_DOMAIN_SEED = 6068040151459020611ull;

#define POSEIDON_STATE_WIDTH 3u
#define POSEIDON_RATE 2u
#define POSEIDON_FULL_ROUNDS_HALF 4u
#define POSEIDON_PARTIAL_ROUNDS 57u
#define BN254_LIMBS 4u

static __device__ __constant__ uint64_t BN254_MODULUS[BN254_LIMBS] = {
    0x43e1f593f0000001ull,
    0x2833e84879b97091ull,
    0xb85045b68181585dull,
    0x30644e72e131a029ull,
};
static __device__ __constant__ uint64_t BN254_MODULUS_INV = 0xc2e1f593efffffffull;
static __device__ __constant__ uint64_t BN254_R2[BN254_LIMBS] = {
    0x1bb8e645ae216da7ull,
    0x53fe3ab1e35c59e3ull,
    0x8c49833d53bb8085ull,
    0x0216d0b17f4e44a5ull,
};

struct PoseidonSlice {
    uint32_t offset;
    uint32_t len;
};

struct Bn254Value {
    uint64_t limbs[BN254_LIMBS];
};

struct TransformWorkspace {
    uint64_t* dense;
    uint64_t* coeffs;
    uint64_t* evals;
    uint64_t* bn254_twiddles;
    uint64_t* bn254_coset;
    size_t dense_capacity_bytes;
    size_t coeff_capacity_bytes;
    size_t eval_capacity_bytes;
    size_t bn254_twiddle_capacity_bytes;
    size_t bn254_coset_capacity_bytes;

    TransformWorkspace()
        : dense(nullptr),
          coeffs(nullptr),
          evals(nullptr),
          bn254_twiddles(nullptr),
          bn254_coset(nullptr),
          dense_capacity_bytes(0),
          coeff_capacity_bytes(0),
          eval_capacity_bytes(0),
          bn254_twiddle_capacity_bytes(0),
          bn254_coset_capacity_bytes(0) {}
};

struct AsyncDispatchBuffers {
    int device;
    uint64_t* dense;
    uint64_t* coeffs;
    uint64_t* evals;
    uint64_t* host_dense;
    uint64_t* host_coeffs;
    uint64_t* host_evals;
    size_t dense_capacity_bytes;
    size_t coeff_capacity_bytes;
    size_t eval_capacity_bytes;
    size_t host_dense_capacity_bytes;
    size_t host_coeff_capacity_bytes;
    size_t host_eval_capacity_bytes;
    cudaStream_t stream;
    bool stream_ready;

    explicit AsyncDispatchBuffers(int device_in)
        : device(device_in),
          dense(nullptr),
          coeffs(nullptr),
          evals(nullptr),
          host_dense(nullptr),
          host_coeffs(nullptr),
          host_evals(nullptr),
          dense_capacity_bytes(0),
          coeff_capacity_bytes(0),
          eval_capacity_bytes(0),
          host_dense_capacity_bytes(0),
          host_coeff_capacity_bytes(0),
          host_eval_capacity_bytes(0),
          stream(nullptr),
          stream_ready(false) {}
};

struct PendingTransform {
    int device;
    cudaEvent_t event;
    AsyncDispatchBuffers* buffers;
    void* host_output;
    void* staging_output;
    size_t output_byte_len;

    PendingTransform(
        int device_in,
        cudaEvent_t event_in,
        AsyncDispatchBuffers* buffers_in,
        void* host_output_in,
        void* staging_output_in,
        size_t output_byte_len_in
    )
        : device(device_in),
          event(event_in),
          buffers(buffers_in),
          host_output(host_output_in),
          staging_output(staging_output_in),
          output_byte_len(output_byte_len_in) {}
};

struct PoseidonWorkspace {
    uint64_t* payloads;
    PoseidonSlice* slices;
    uint64_t* states;
    uint64_t* hashes;
    size_t payload_capacity_bytes;
    size_t slice_capacity_bytes;
    size_t state_capacity_bytes;
    size_t hash_capacity_bytes;

    PoseidonWorkspace()
        : payloads(nullptr),
          slices(nullptr),
          states(nullptr),
          hashes(nullptr),
          payload_capacity_bytes(0),
          slice_capacity_bytes(0),
          state_capacity_bytes(0),
          hash_capacity_bytes(0) {}
};

static std::mutex& transform_workspace_mutex() {
    static std::mutex mutex;
    return mutex;
}

static std::unordered_map<int, TransformWorkspace>& transform_workspaces() {
    static auto* workspaces = new std::unordered_map<int, TransformWorkspace>();
    return *workspaces;
}

static std::mutex& async_dispatch_pool_mutex() {
    static std::mutex mutex;
    return mutex;
}

static std::unordered_map<int, std::vector<AsyncDispatchBuffers*>>& async_dispatch_pools() {
    static auto* pools = new std::unordered_map<int, std::vector<AsyncDispatchBuffers*>>();
    return *pools;
}

static std::mutex& poseidon_workspace_mutex() {
    static std::mutex mutex;
    return mutex;
}

static std::unordered_map<int, PoseidonWorkspace>& poseidon_workspaces() {
    static auto* workspaces = new std::unordered_map<int, PoseidonWorkspace>();
    return *workspaces;
}

template <typename T>
static cudaError_t ensure_workspace_buffer(
    T** buffer,
    size_t* capacity_bytes,
    size_t need_bytes
) {
    if (need_bytes == 0) {
        return cudaSuccess;
    }
    if (*buffer != nullptr && *capacity_bytes >= need_bytes) {
        return cudaSuccess;
    }
    if (*buffer != nullptr) {
        cudaError_t free_status = cudaFree(*buffer);
        if (free_status != cudaSuccess) {
            return free_status;
        }
        *buffer = nullptr;
        *capacity_bytes = 0;
    }
    cudaError_t status = cudaMalloc(reinterpret_cast<void**>(buffer), need_bytes);
    if (status == cudaSuccess) {
        *capacity_bytes = need_bytes;
    }
    return status;
}

template <typename T>
static cudaError_t ensure_pinned_workspace_buffer(
    T** buffer,
    size_t* capacity_bytes,
    size_t need_bytes
) {
    if (need_bytes == 0) {
        return cudaSuccess;
    }
    if (*buffer != nullptr && *capacity_bytes >= need_bytes) {
        return cudaSuccess;
    }
    if (*buffer != nullptr) {
        cudaError_t free_status = cudaFreeHost(*buffer);
        if (free_status != cudaSuccess) {
            return free_status;
        }
        *buffer = nullptr;
        *capacity_bytes = 0;
    }
    cudaError_t status = cudaHostAlloc(reinterpret_cast<void**>(buffer), need_bytes, cudaHostAllocDefault);
    if (status == cudaSuccess) {
        *capacity_bytes = need_bytes;
    }
    return status;
}

static cudaError_t ensure_async_dispatch_stream(AsyncDispatchBuffers* buffers) {
    if (buffers == nullptr) {
        return cudaErrorInvalidValue;
    }
    if (buffers->stream_ready) {
        return cudaSuccess;
    }
    cudaError_t status = cudaStreamCreateWithFlags(&buffers->stream, cudaStreamNonBlocking);
    if (status == cudaSuccess) {
        buffers->stream_ready = true;
    }
    return status;
}

static cudaError_t dense_buffer_bytes(size_t column_count, uint64_t column_len, size_t* byte_len) {
    if (byte_len == nullptr) {
        return cudaErrorInvalidValue;
    }
    size_t column_len_size = (size_t)column_len;
    if (column_len != (uint64_t)column_len_size) {
        return cudaErrorInvalidValue;
    }
    if (column_len_size != 0 && column_count > SIZE_MAX / column_len_size) {
        return cudaErrorInvalidValue;
    }
    size_t element_count = column_count * column_len_size;
    if (element_count > SIZE_MAX / sizeof(uint64_t)) {
        return cudaErrorInvalidValue;
    }
    *byte_len = element_count * sizeof(uint64_t);
    return cudaSuccess;
}

static cudaError_t limb_buffer_bytes(size_t limb_count, size_t* byte_len) {
    if (byte_len == nullptr) {
        return cudaErrorInvalidValue;
    }
    if (limb_count > SIZE_MAX / sizeof(uint64_t)) {
        return cudaErrorInvalidValue;
    }
    *byte_len = limb_count * sizeof(uint64_t);
    return cudaSuccess;
}

static cudaError_t bn254_dense_buffer_bytes(
    size_t column_count,
    uint64_t element_count,
    size_t* byte_len
) {
    if (element_count > UINT64_MAX / BN254_LIMBS) {
        return cudaErrorInvalidValue;
    }
    return dense_buffer_bytes(column_count, element_count * BN254_LIMBS, byte_len);
}

static TransformWorkspace* current_transform_workspace() {
    int device = 0;
    if (cudaGetDevice(&device) != cudaSuccess) {
        return nullptr;
    }
    return &transform_workspaces()[device];
}

static AsyncDispatchBuffers* acquire_async_dispatch_buffers(int device) {
    std::lock_guard<std::mutex> guard(async_dispatch_pool_mutex());
    std::vector<AsyncDispatchBuffers*>& pool = async_dispatch_pools()[device];
    if (!pool.empty()) {
        AsyncDispatchBuffers* buffers = pool.back();
        pool.pop_back();
        return buffers;
    }
    return new (std::nothrow) AsyncDispatchBuffers(device);
}

static void release_async_dispatch_buffers(AsyncDispatchBuffers* buffers) {
    if (buffers == nullptr) {
        return;
    }
    std::lock_guard<std::mutex> guard(async_dispatch_pool_mutex());
    async_dispatch_pools()[buffers->device].push_back(buffers);
}

static void destroy_pending_transform(PendingTransform* pending) {
    if (pending == nullptr) {
        return;
    }
    (void)cudaSetDevice(pending->device);
    if (pending->buffers != nullptr && pending->buffers->stream_ready) {
        (void)cudaStreamSynchronize(pending->buffers->stream);
    }
    if (pending->event != nullptr) {
        (void)cudaEventDestroy(pending->event);
    }
    release_async_dispatch_buffers(pending->buffers);
    delete pending;
}

static cudaError_t create_pending_transform(
    int device,
    AsyncDispatchBuffers* buffers,
    void* host_output,
    void* staging_output,
    size_t output_byte_len,
    PendingTransform** pending_out
) {
    if (pending_out == nullptr || buffers == nullptr) {
        return cudaErrorInvalidValue;
    }
    *pending_out = nullptr;
    cudaEvent_t event = nullptr;
    cudaError_t status = cudaEventCreateWithFlags(&event, cudaEventDisableTiming);
    if (status != cudaSuccess) {
        return status;
    }
    PendingTransform* pending = new (std::nothrow)
        PendingTransform(
            device,
            event,
            buffers,
            host_output,
            staging_output,
            output_byte_len
        );
    if (pending == nullptr) {
        (void)cudaEventDestroy(event);
        return cudaErrorMemoryAllocation;
    }
    *pending_out = pending;
    return cudaSuccess;
}

static PoseidonWorkspace* current_poseidon_workspace() {
    int device = 0;
    if (cudaGetDevice(&device) != cudaSuccess) {
        return nullptr;
    }
    return &poseidon_workspaces()[device];
}

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

__device__ __forceinline__ Bn254Value bn254_load(const uint64_t* ptr) {
    Bn254Value value;
    #pragma unroll
    for (unsigned int idx = 0; idx < BN254_LIMBS; ++idx) {
        value.limbs[idx] = ptr[idx];
    }
    return value;
}

__device__ __forceinline__ Bn254Value bn254_zero() {
    Bn254Value value = {{0ull, 0ull, 0ull, 0ull}};
    return value;
}

__device__ __forceinline__ void bn254_store(uint64_t* ptr, const Bn254Value& value) {
    #pragma unroll
    for (unsigned int idx = 0; idx < BN254_LIMBS; ++idx) {
        ptr[idx] = value.limbs[idx];
    }
}

__device__ __forceinline__ bool bn254_ge_modulus(const Bn254Value& value) {
    for (int idx = (int)BN254_LIMBS - 1; idx >= 0; --idx) {
        if (value.limbs[idx] != BN254_MODULUS[idx]) {
            return value.limbs[idx] > BN254_MODULUS[idx];
        }
    }
    return true;
}

__device__ __forceinline__ uint64_t add_with_carry(
    uint64_t lhs,
    uint64_t rhs,
    uint64_t* carry
) {
    uint64_t sum = lhs + rhs;
    bool carry0 = sum < lhs;
    uint64_t result = sum + *carry;
    bool carry1 = result < sum;
    *carry = (carry0 || carry1) ? 1ull : 0ull;
    return result;
}

__device__ __forceinline__ uint64_t sub_with_borrow(
    uint64_t lhs,
    uint64_t rhs,
    uint64_t* borrow
) {
    uint64_t diff = lhs - rhs;
    bool borrow0 = lhs < rhs;
    uint64_t result = diff - *borrow;
    bool borrow1 = diff < *borrow;
    *borrow = (borrow0 || borrow1) ? 1ull : 0ull;
    return result;
}

__device__ __forceinline__ Bn254Value bn254_sub_modulus(const Bn254Value& value) {
    Bn254Value out;
    uint64_t borrow = 0ull;
    #pragma unroll
    for (unsigned int idx = 0; idx < BN254_LIMBS; ++idx) {
        out.limbs[idx] = sub_with_borrow(value.limbs[idx], BN254_MODULUS[idx], &borrow);
    }
    return out;
}

__device__ __forceinline__ Bn254Value bn254_add(const Bn254Value& a, const Bn254Value& b) {
    Bn254Value sum;
    uint64_t carry = 0ull;
    #pragma unroll
    for (unsigned int idx = 0; idx < BN254_LIMBS; ++idx) {
        sum.limbs[idx] = add_with_carry(a.limbs[idx], b.limbs[idx], &carry);
    }
    if (carry != 0ull || bn254_ge_modulus(sum)) {
        sum = bn254_sub_modulus(sum);
    }
    return sum;
}

__device__ __forceinline__ Bn254Value bn254_sub(const Bn254Value& a, const Bn254Value& b) {
    Bn254Value out;
    uint64_t borrow = 0ull;
    #pragma unroll
    for (unsigned int idx = 0; idx < BN254_LIMBS; ++idx) {
        out.limbs[idx] = sub_with_borrow(a.limbs[idx], b.limbs[idx], &borrow);
    }
    if (borrow != 0ull) {
        uint64_t carry = 0ull;
        #pragma unroll
        for (unsigned int idx = 0; idx < BN254_LIMBS; ++idx) {
            out.limbs[idx] = add_with_carry(out.limbs[idx], BN254_MODULUS[idx], &carry);
        }
    }
    return out;
}

__device__ __forceinline__ void bn254_mul_add(
    uint64_t x,
    uint64_t y,
    uint64_t* acc,
    uint64_t* carry
) {
    uint64_t lo = x * y;
    uint64_t hi = __umul64hi(x, y);

    uint64_t sum = *acc + lo;
    uint64_t carry0 = sum < *acc ? 1ull : 0ull;
    sum += *carry;
    uint64_t carry1 = sum < *carry ? 1ull : 0ull;

    *acc = sum;
    *carry = hi + carry0 + carry1;
}

__device__ __forceinline__ void bn254_montgomery_reduce(
    uint64_t t[8],
    uint64_t out[BN254_LIMBS]
) {
    for (unsigned int idx = 0; idx < BN254_LIMBS; ++idx) {
        uint64_t m = t[idx] * BN254_MODULUS_INV;
        uint64_t carry = 0ull;
        #pragma unroll
        for (unsigned int limb = 0; limb < BN254_LIMBS; ++limb) {
            bn254_mul_add(m, BN254_MODULUS[limb], &t[idx + limb], &carry);
        }
        unsigned int carry_idx = idx + BN254_LIMBS;
        uint64_t sum = t[carry_idx] + carry;
        uint64_t carry_out = sum < t[carry_idx] ? 1ull : 0ull;
        t[carry_idx] = sum;
        while (carry_out != 0ull) {
            ++carry_idx;
            uint64_t prior = t[carry_idx];
            t[carry_idx] += carry_out;
            carry_out = t[carry_idx] < prior ? 1ull : 0ull;
        }
    }

    Bn254Value candidate;
    #pragma unroll
    for (unsigned int idx = 0; idx < BN254_LIMBS; ++idx) {
        out[idx] = t[idx + BN254_LIMBS];
        candidate.limbs[idx] = out[idx];
    }
    if (bn254_ge_modulus(candidate)) {
        candidate = bn254_sub_modulus(candidate);
        #pragma unroll
        for (unsigned int idx = 0; idx < BN254_LIMBS; ++idx) {
            out[idx] = candidate.limbs[idx];
        }
    }
}

__device__ __forceinline__ Bn254Value bn254_montgomery_mul(
    const Bn254Value& a,
    const Bn254Value& b
) {
    uint64_t t[8] = {0ull, 0ull, 0ull, 0ull, 0ull, 0ull, 0ull, 0ull};
    for (unsigned int i = 0; i < BN254_LIMBS; ++i) {
        uint64_t carry = 0ull;
        for (unsigned int j = 0; j < BN254_LIMBS; ++j) {
            bn254_mul_add(a.limbs[i], b.limbs[j], &t[i + j], &carry);
        }
        unsigned int idx = i + BN254_LIMBS;
        uint64_t sum = t[idx] + carry;
        uint64_t carry_out = sum < t[idx] ? 1ull : 0ull;
        t[idx] = sum;
        while (carry_out != 0ull) {
            ++idx;
            uint64_t prior = t[idx];
            t[idx] += carry_out;
            carry_out = t[idx] < prior ? 1ull : 0ull;
        }
    }
    uint64_t reduced[BN254_LIMBS];
    bn254_montgomery_reduce(t, reduced);
    Bn254Value out;
    #pragma unroll
    for (unsigned int idx = 0; idx < BN254_LIMBS; ++idx) {
        out.limbs[idx] = reduced[idx];
    }
    return out;
}

__device__ __forceinline__ Bn254Value bn254_r2() {
    Bn254Value out;
    #pragma unroll
    for (unsigned int idx = 0; idx < BN254_LIMBS; ++idx) {
        out.limbs[idx] = BN254_R2[idx];
    }
    return out;
}

__device__ __forceinline__ Bn254Value bn254_from_canonical(const Bn254Value& raw);

__device__ __forceinline__ Bn254Value bn254_one_mont() {
    Bn254Value one = {{1ull, 0ull, 0ull, 0ull}};
    return bn254_from_canonical(one);
}

__device__ __forceinline__ Bn254Value bn254_from_canonical(const Bn254Value& raw) {
    return bn254_montgomery_mul(raw, bn254_r2());
}

__device__ __forceinline__ Bn254Value bn254_pow_u64(Bn254Value base, uint64_t exp) {
    Bn254Value acc = bn254_one_mont();
    while (exp != 0ull) {
        if ((exp & 1ull) != 0ull) {
            acc = bn254_montgomery_mul(acc, base);
        }
        base = bn254_montgomery_mul(base, base);
        exp >>= 1ull;
    }
    return acc;
}

__device__ __forceinline__ Bn254Value bn254_to_canonical(const Bn254Value& value) {
    uint64_t t[8] = {
        value.limbs[0],
        value.limbs[1],
        value.limbs[2],
        value.limbs[3],
        0ull,
        0ull,
        0ull,
        0ull,
    };
    uint64_t reduced[BN254_LIMBS];
    bn254_montgomery_reduce(t, reduced);
    Bn254Value out;
    #pragma unroll
    for (unsigned int idx = 0; idx < BN254_LIMBS; ++idx) {
        out.limbs[idx] = reduced[idx];
    }
    return out;
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
__device__ void fft_inplace_device(
    uint64_t* data,
    uint64_t len,
    uint32_t log_len,
    uint64_t root,
    bool inverse
) {
    for (uint64_t i = 1; i < len - 1; ++i) {
        uint64_t j = bit_reverse_index(i, log_len);
        if (i < j) {
            uint64_t tmp = data[i];
            data[i] = data[j];
            data[j] = tmp;
        }
    }

    uint64_t omega = inverse ? f_inv(root) : root;

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
    uint32_t eval_log,
    uint64_t eval_root,
    uint64_t coset
) {
    uint64_t coset_pow = 1ull;
    for (uint64_t i = 0; i < trace_len; ++i) {
        out[i] = f_mul(coeffs[i], coset_pow);
        coset_pow = f_mul(coset_pow, coset);
    }
    for (uint64_t i = trace_len; i < eval_len; ++i) {
        out[i] = 0ull;
    }
    fft_inplace_device(out, eval_len, eval_log, eval_root, false);
}

// -----------------------------------------------------------------------------.
// CUDA kernels - each column handled by a single thread for determinism.
// -----------------------------------------------------------------------------
__global__ void fastpq_fft_kernel(
    uint64_t* elements,
    size_t column_count,
    uint64_t column_len,
    uint32_t log_len,
    uint64_t root
) {
    uint64_t col = blockIdx.x * blockDim.x + threadIdx.x;
    if ((size_t)col >= column_count) {
        return;
    }
    uint64_t* column = elements + col * column_len;
    fft_inplace_device(column, column_len, log_len, root, false);
}

__global__ void fastpq_ifft_kernel(
    uint64_t* elements,
    size_t column_count,
    uint64_t column_len,
    uint32_t log_len,
    uint64_t root
) {
    uint64_t col = blockIdx.x * blockDim.x + threadIdx.x;
    if ((size_t)col >= column_count) {
        return;
    }
    uint64_t* column = elements + col * column_len;
    fft_inplace_device(column, column_len, log_len, root, true);
}

__global__ void fastpq_lde_kernel(
    const uint64_t* coeffs,
    uint64_t* out,
    uint64_t trace_len,
    uint64_t eval_len,
    uint32_t eval_log,
    uint64_t eval_root,
    uint64_t coset
) {
    uint64_t col = blockIdx.x;
    const uint64_t* column_coeffs = coeffs + col * trace_len;
    uint64_t* out_column = out + col * eval_len;
    lde_eval_device(column_coeffs, out_column, trace_len, eval_len, eval_log, eval_root, coset);
}

__global__ void fastpq_bn254_fft_kernel(
    uint64_t* elements,
    size_t column_count,
    uint64_t column_len,
    uint32_t log_len,
    const uint64_t* stage_twiddles
) {
    size_t col = (size_t)blockIdx.x;
    if (col >= column_count || stage_twiddles == nullptr) {
        return;
    }
    uint64_t* column = elements + col * column_len * BN254_LIMBS;
    for (uint64_t idx = threadIdx.x; idx < column_len; idx += blockDim.x) {
        Bn254Value value = bn254_load(column + idx * BN254_LIMBS);
        bn254_store(column + idx * BN254_LIMBS, bn254_from_canonical(value));
    }
    __syncthreads();

    uint64_t stage_span = column_len >> 1;
    for (uint32_t stage = 0; stage < log_len; ++stage) {
        uint64_t len = 1ull << (stage + 1u);
        uint64_t half_len = len >> 1;
        uint64_t stage_offset = (uint64_t)stage * stage_span;
        for (uint64_t pair_idx = threadIdx.x; pair_idx < stage_span; pair_idx += blockDim.x) {
            uint64_t block = pair_idx / half_len;
            uint64_t pair = pair_idx % half_len;
            uint64_t idx = block * len + pair;
            if (idx + half_len >= column_len) {
                continue;
            }
            Bn254Value twiddle = bn254_from_canonical(
                bn254_load(stage_twiddles + (stage_offset + pair) * BN254_LIMBS)
            );
            Bn254Value u = bn254_load(column + idx * BN254_LIMBS);
            Bn254Value v = bn254_montgomery_mul(
                bn254_load(column + (idx + half_len) * BN254_LIMBS),
                twiddle
            );
            bn254_store(column + idx * BN254_LIMBS, bn254_add(u, v));
            bn254_store(column + (idx + half_len) * BN254_LIMBS, bn254_sub(u, v));
        }
        __syncthreads();
    }

    for (uint64_t idx = threadIdx.x; idx < column_len; idx += blockDim.x) {
        Bn254Value value = bn254_load(column + idx * BN254_LIMBS);
        bn254_store(column + idx * BN254_LIMBS, bn254_to_canonical(value));
    }
}

__global__ void fastpq_bn254_lde_kernel(
    const uint64_t* coeffs,
    uint64_t* out,
    uint64_t trace_len,
    uint64_t eval_len,
    uint32_t trace_log,
    uint32_t blowup_log,
    const uint64_t* stage_twiddles,
    const uint64_t* coset_limbs
) {
    size_t col = (size_t)blockIdx.x;
    if (stage_twiddles == nullptr || coset_limbs == nullptr) {
        return;
    }
    const uint64_t* coeff_column = coeffs + col * trace_len * BN254_LIMBS;
    uint64_t* out_column = out + col * eval_len * BN254_LIMBS;
    for (uint64_t idx = threadIdx.x; idx < trace_len; idx += blockDim.x) {
        Bn254Value value = bn254_load(coeff_column + idx * BN254_LIMBS);
        bn254_store(out_column + idx * BN254_LIMBS, bn254_from_canonical(value));
    }
    for (uint64_t idx = threadIdx.x + trace_len; idx < eval_len; idx += blockDim.x) {
        Bn254Value zero = {{0ull, 0ull, 0ull, 0ull}};
        bn254_store(out_column + idx * BN254_LIMBS, zero);
    }
    __syncthreads();

    const uint32_t log_len = trace_log + blowup_log;
    const uint64_t stage_span = eval_len >> 1;
    const Bn254Value coset_mont = bn254_from_canonical(bn254_load(coset_limbs));
    for (uint64_t idx = threadIdx.x; idx < trace_len; idx += blockDim.x) {
        Bn254Value value = bn254_load(out_column + idx * BN254_LIMBS);
        Bn254Value coset_power = bn254_pow_u64(coset_mont, idx);
        bn254_store(
            out_column + idx * BN254_LIMBS,
            bn254_montgomery_mul(value, coset_power)
        );
    }
    __syncthreads();

    for (uint32_t stage = 0; stage < log_len; ++stage) {
        const uint64_t len = 1ull << (stage + 1u);
        const uint64_t half_len = len >> 1;
        const uint64_t stage_offset = (uint64_t)stage * stage_span;
        for (uint64_t pair_idx = threadIdx.x; pair_idx < stage_span; pair_idx += blockDim.x) {
            const uint64_t block = pair_idx / half_len;
            const uint64_t pair = pair_idx % half_len;
            const uint64_t idx = block * len + pair;
            if (idx + half_len >= eval_len) {
                continue;
            }
            Bn254Value twiddle = bn254_from_canonical(
                bn254_load(stage_twiddles + (stage_offset + pair) * BN254_LIMBS)
            );
            Bn254Value u = bn254_load(out_column + idx * BN254_LIMBS);
            Bn254Value v = bn254_montgomery_mul(
                bn254_load(out_column + (idx + half_len) * BN254_LIMBS),
                twiddle
            );
            bn254_store(out_column + idx * BN254_LIMBS, bn254_add(u, v));
            bn254_store(out_column + (idx + half_len) * BN254_LIMBS, bn254_sub(u, v));
        }
        __syncthreads();
    }

    for (uint64_t idx = threadIdx.x; idx < eval_len; idx += blockDim.x) {
        Bn254Value value = bn254_load(out_column + idx * BN254_LIMBS);
        bn254_store(out_column + idx * BN254_LIMBS, bn254_to_canonical(value));
    }
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
    uint64_t state[POSEIDON_STATE_WIDTH] = {0ull, 0ull, 0ull};
    state[0] = f_add(state[0], TRACE_NODE_DOMAIN_SEED);
    state[1] = f_add(state[1], leaf_hashes[left]);
    poseidon_permute(state);
    state[0] = f_add(state[0], leaf_hashes[right]);
    state[1] = f_add(state[1], 1ull);
    poseidon_permute(state);
    parent_hashes[parent_idx] = state[0];
}

// -----------------------------------------------------------------------------.
// Public C interface.
// -----------------------------------------------------------------------------
extern "C" cudaError_t fastpq_pending_wait_cuda(void* handle) {
    PendingTransform* pending = reinterpret_cast<PendingTransform*>(handle);
    if (pending == nullptr) {
        return cudaErrorInvalidValue;
    }

    cudaError_t status = cudaSetDevice(pending->device);
    if (status != cudaSuccess) {
        destroy_pending_transform(pending);
        return status;
    }

    status = cudaEventSynchronize(pending->event);
    if (status == cudaSuccess) {
        if (pending->staging_output == nullptr || pending->host_output == nullptr) {
            status = cudaErrorInvalidValue;
        } else if (pending->staging_output != pending->host_output) {
            memcpy(pending->host_output, pending->staging_output, pending->output_byte_len);
        }
    }
    destroy_pending_transform(pending);
    return status;
}

extern "C" cudaError_t fastpq_fft_async_submit_cuda(
    uint64_t* elements,
    size_t column_count,
    uint32_t log_size,
    uint64_t root,
    void** out_handle
) {
    if (elements == nullptr || column_count == 0 || out_handle == nullptr) {
        return cudaErrorInvalidValue;
    }
    *out_handle = nullptr;
    uint64_t n = 1ull << log_size;
    size_t byte_len = 0;
    cudaError_t status = dense_buffer_bytes(column_count, n, &byte_len);
    if (status != cudaSuccess) {
        return status;
    }

    int device = 0;
    status = cudaGetDevice(&device);
    if (status != cudaSuccess) {
        return status;
    }
    AsyncDispatchBuffers* buffers = acquire_async_dispatch_buffers(device);
    if (buffers == nullptr) {
        return cudaErrorMemoryAllocation;
    }
    status = ensure_workspace_buffer(
        &buffers->dense,
        &buffers->dense_capacity_bytes,
        byte_len
    );
    if (status != cudaSuccess) {
        release_async_dispatch_buffers(buffers);
        return status;
    }
    status = ensure_pinned_workspace_buffer(
        &buffers->host_dense,
        &buffers->host_dense_capacity_bytes,
        byte_len
    );
    if (status != cudaSuccess) {
        release_async_dispatch_buffers(buffers);
        return status;
    }
    status = ensure_async_dispatch_stream(buffers);
    if (status != cudaSuccess) {
        release_async_dispatch_buffers(buffers);
        return status;
    }
    memcpy(buffers->host_dense, elements, byte_len);
    uint64_t* host_dense_io = buffers->host_dense;
    void* staging_output = buffers->host_dense;

    PendingTransform* pending = nullptr;
    status = create_pending_transform(
        device,
        buffers,
        elements,
        staging_output,
        byte_len,
        &pending
    );
    if (status != cudaSuccess) {
        release_async_dispatch_buffers(buffers);
        return status;
    }

    status = cudaMemcpyAsync(
        buffers->dense,
        host_dense_io,
        byte_len,
        cudaMemcpyHostToDevice,
        buffers->stream
    );
    if (status != cudaSuccess) {
        destroy_pending_transform(pending);
        return status;
    }
    const unsigned int threads_per_block = 128u;
    size_t grid_size_host = (column_count + threads_per_block - 1u) / threads_per_block;
    if (grid_size_host == 0) {
        grid_size_host = 1;
    }
    if (grid_size_host > (size_t)UINT32_MAX) {
        destroy_pending_transform(pending);
        return cudaErrorInvalidValue;
    }
    fastpq_fft_kernel<<<(unsigned int)grid_size_host, threads_per_block, 0, buffers->stream>>>(
        buffers->dense,
        column_count,
        n,
        log_size,
        root
    );
    status = cudaGetLastError();
    if (status != cudaSuccess) {
        destroy_pending_transform(pending);
        return status;
    }
    status = cudaMemcpyAsync(
        staging_output,
        buffers->dense,
        byte_len,
        cudaMemcpyDeviceToHost,
        buffers->stream
    );
    if (status != cudaSuccess) {
        destroy_pending_transform(pending);
        return status;
    }
    status = cudaEventRecord(pending->event, buffers->stream);
    if (status != cudaSuccess) {
        destroy_pending_transform(pending);
        return status;
    }
    *out_handle = pending;
    return cudaSuccess;
}

extern "C" cudaError_t fastpq_ifft_async_submit_cuda(
    uint64_t* elements,
    size_t column_count,
    uint32_t log_size,
    uint64_t root,
    void** out_handle
) {
    if (elements == nullptr || column_count == 0 || out_handle == nullptr) {
        return cudaErrorInvalidValue;
    }
    *out_handle = nullptr;
    uint64_t n = 1ull << log_size;
    size_t byte_len = 0;
    cudaError_t status = dense_buffer_bytes(column_count, n, &byte_len);
    if (status != cudaSuccess) {
        return status;
    }

    int device = 0;
    status = cudaGetDevice(&device);
    if (status != cudaSuccess) {
        return status;
    }
    AsyncDispatchBuffers* buffers = acquire_async_dispatch_buffers(device);
    if (buffers == nullptr) {
        return cudaErrorMemoryAllocation;
    }
    status = ensure_workspace_buffer(
        &buffers->dense,
        &buffers->dense_capacity_bytes,
        byte_len
    );
    if (status != cudaSuccess) {
        release_async_dispatch_buffers(buffers);
        return status;
    }
    status = ensure_pinned_workspace_buffer(
        &buffers->host_dense,
        &buffers->host_dense_capacity_bytes,
        byte_len
    );
    if (status != cudaSuccess) {
        release_async_dispatch_buffers(buffers);
        return status;
    }
    status = ensure_async_dispatch_stream(buffers);
    if (status != cudaSuccess) {
        release_async_dispatch_buffers(buffers);
        return status;
    }
    memcpy(buffers->host_dense, elements, byte_len);
    uint64_t* host_dense_io = buffers->host_dense;
    void* staging_output = buffers->host_dense;

    PendingTransform* pending = nullptr;
    status = create_pending_transform(
        device,
        buffers,
        elements,
        staging_output,
        byte_len,
        &pending
    );
    if (status != cudaSuccess) {
        release_async_dispatch_buffers(buffers);
        return status;
    }

    status = cudaMemcpyAsync(
        buffers->dense,
        host_dense_io,
        byte_len,
        cudaMemcpyHostToDevice,
        buffers->stream
    );
    if (status != cudaSuccess) {
        destroy_pending_transform(pending);
        return status;
    }
    const unsigned int threads_per_block = 128u;
    size_t grid_size_host = (column_count + threads_per_block - 1u) / threads_per_block;
    if (grid_size_host == 0) {
        grid_size_host = 1;
    }
    if (grid_size_host > (size_t)UINT32_MAX) {
        destroy_pending_transform(pending);
        return cudaErrorInvalidValue;
    }
    fastpq_ifft_kernel<<<(unsigned int)grid_size_host, threads_per_block, 0, buffers->stream>>>(
        buffers->dense,
        column_count,
        n,
        log_size,
        root
    );
    status = cudaGetLastError();
    if (status != cudaSuccess) {
        destroy_pending_transform(pending);
        return status;
    }
    status = cudaMemcpyAsync(
        staging_output,
        buffers->dense,
        byte_len,
        cudaMemcpyDeviceToHost,
        buffers->stream
    );
    if (status != cudaSuccess) {
        destroy_pending_transform(pending);
        return status;
    }
    status = cudaEventRecord(pending->event, buffers->stream);
    if (status != cudaSuccess) {
        destroy_pending_transform(pending);
        return status;
    }
    *out_handle = pending;
    return cudaSuccess;
}

extern "C" cudaError_t fastpq_lde_async_submit_cuda(
    const uint64_t* coeffs,
    size_t column_count,
    uint32_t trace_log,
    uint32_t blowup_log,
    uint64_t lde_root,
    uint64_t coset,
    uint64_t* out,
    void** out_handle
) {
    if (coeffs == nullptr || out == nullptr || column_count == 0 || out_handle == nullptr) {
        return cudaErrorInvalidValue;
    }
    *out_handle = nullptr;
    uint64_t trace_len = 1ull << trace_log;
    uint64_t eval_len = 1ull << (trace_log + blowup_log);

    size_t coeff_byte_len = 0;
    cudaError_t status = dense_buffer_bytes(column_count, trace_len, &coeff_byte_len);
    if (status != cudaSuccess) {
        return status;
    }
    size_t eval_byte_len = 0;
    status = dense_buffer_bytes(column_count, eval_len, &eval_byte_len);
    if (status != cudaSuccess) {
        return status;
    }

    int device = 0;
    status = cudaGetDevice(&device);
    if (status != cudaSuccess) {
        return status;
    }
    AsyncDispatchBuffers* buffers = acquire_async_dispatch_buffers(device);
    if (buffers == nullptr) {
        return cudaErrorMemoryAllocation;
    }
    status = ensure_workspace_buffer(
        &buffers->coeffs,
        &buffers->coeff_capacity_bytes,
        coeff_byte_len
    );
    if (status != cudaSuccess) {
        release_async_dispatch_buffers(buffers);
        return status;
    }
    status = ensure_workspace_buffer(
        &buffers->evals,
        &buffers->eval_capacity_bytes,
        eval_byte_len
    );
    if (status != cudaSuccess) {
        release_async_dispatch_buffers(buffers);
        return status;
    }
    status = ensure_pinned_workspace_buffer(
        &buffers->host_coeffs,
        &buffers->host_coeff_capacity_bytes,
        coeff_byte_len
    );
    if (status != cudaSuccess) {
        release_async_dispatch_buffers(buffers);
        return status;
    }
    status = ensure_pinned_workspace_buffer(
        &buffers->host_evals,
        &buffers->host_eval_capacity_bytes,
        eval_byte_len
    );
    if (status != cudaSuccess) {
        release_async_dispatch_buffers(buffers);
        return status;
    }
    status = ensure_async_dispatch_stream(buffers);
    if (status != cudaSuccess) {
        release_async_dispatch_buffers(buffers);
        return status;
    }
    memcpy(buffers->host_coeffs, coeffs, coeff_byte_len);
    const uint64_t* host_coeff_input = buffers->host_coeffs;
    uint64_t* host_eval_output = buffers->host_evals;
    void* staging_output = buffers->host_evals;

    PendingTransform* pending = nullptr;
    status = create_pending_transform(
        device,
        buffers,
        out,
        staging_output,
        eval_byte_len,
        &pending
    );
    if (status != cudaSuccess) {
        release_async_dispatch_buffers(buffers);
        return status;
    }

    status = cudaMemcpyAsync(
        buffers->coeffs,
        host_coeff_input,
        coeff_byte_len,
        cudaMemcpyHostToDevice,
        buffers->stream
    );
    if (status != cudaSuccess) {
        destroy_pending_transform(pending);
        return status;
    }
    if (column_count > (size_t)UINT32_MAX) {
        destroy_pending_transform(pending);
        return cudaErrorInvalidValue;
    }
    fastpq_lde_kernel<<<(unsigned int)column_count, 1, 0, buffers->stream>>>(
        buffers->coeffs,
        buffers->evals,
        trace_len,
        eval_len,
        trace_log + blowup_log,
        lde_root,
        coset
    );
    status = cudaGetLastError();
    if (status != cudaSuccess) {
        destroy_pending_transform(pending);
        return status;
    }
    status = cudaMemcpyAsync(
        host_eval_output,
        buffers->evals,
        eval_byte_len,
        cudaMemcpyDeviceToHost,
        buffers->stream
    );
    if (status != cudaSuccess) {
        destroy_pending_transform(pending);
        return status;
    }
    status = cudaEventRecord(pending->event, buffers->stream);
    if (status != cudaSuccess) {
        destroy_pending_transform(pending);
        return status;
    }
    *out_handle = pending;
    return cudaSuccess;
}

extern "C" cudaError_t fastpq_fft_cuda(
    uint64_t* elements,
    size_t column_count,
    uint32_t log_size,
    uint64_t root
) {
    if (elements == nullptr || column_count == 0) {
        return cudaErrorInvalidValue;
    }
    uint64_t n = 1ull << log_size;
    size_t byte_len = 0;
    cudaError_t status = dense_buffer_bytes(column_count, n, &byte_len);
    if (status != cudaSuccess) {
        return status;
    }
    std::lock_guard<std::mutex> guard(transform_workspace_mutex());
    TransformWorkspace* workspace = current_transform_workspace();
    if (workspace == nullptr) {
        return cudaErrorInvalidValue;
    }
    status = ensure_workspace_buffer(
        &workspace->dense,
        &workspace->dense_capacity_bytes,
        byte_len
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMemcpy(workspace->dense, elements, byte_len, cudaMemcpyHostToDevice);
    if (status != cudaSuccess) {
        return status;
    }
    const unsigned int threads_per_block = 128u;
    size_t grid_size_host = (column_count + threads_per_block - 1u) / threads_per_block;
    if (grid_size_host == 0) {
        grid_size_host = 1;
    }
    if (grid_size_host > (size_t)UINT32_MAX) {
        return cudaErrorInvalidValue;
    }
    fastpq_fft_kernel<<<(unsigned int)grid_size_host, threads_per_block>>>(
        workspace->dense,
        column_count,
        n,
        log_size,
        root
    );
    status = cudaGetLastError();
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMemcpy(elements, workspace->dense, byte_len, cudaMemcpyDeviceToHost);
    if (status != cudaSuccess) {
        return status;
    }
    return cudaSuccess;
}

extern "C" cudaError_t fastpq_ifft_cuda(
    uint64_t* elements,
    size_t column_count,
    uint32_t log_size,
    uint64_t root
) {
    if (elements == nullptr || column_count == 0) {
        return cudaErrorInvalidValue;
    }
    uint64_t n = 1ull << log_size;
    size_t byte_len = 0;
    cudaError_t status = dense_buffer_bytes(column_count, n, &byte_len);
    if (status != cudaSuccess) {
        return status;
    }
    std::lock_guard<std::mutex> guard(transform_workspace_mutex());
    TransformWorkspace* workspace = current_transform_workspace();
    if (workspace == nullptr) {
        return cudaErrorInvalidValue;
    }
    status = ensure_workspace_buffer(
        &workspace->dense,
        &workspace->dense_capacity_bytes,
        byte_len
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMemcpy(workspace->dense, elements, byte_len, cudaMemcpyHostToDevice);
    if (status != cudaSuccess) {
        return status;
    }
    const unsigned int threads_per_block = 128u;
    size_t grid_size_host = (column_count + threads_per_block - 1u) / threads_per_block;
    if (grid_size_host == 0) {
        grid_size_host = 1;
    }
    if (grid_size_host > (size_t)UINT32_MAX) {
        return cudaErrorInvalidValue;
    }
    fastpq_ifft_kernel<<<(unsigned int)grid_size_host, threads_per_block>>>(
        workspace->dense,
        column_count,
        n,
        log_size,
        root
    );
    status = cudaGetLastError();
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMemcpy(elements, workspace->dense, byte_len, cudaMemcpyDeviceToHost);
    if (status != cudaSuccess) {
        return status;
    }
    return cudaSuccess;
}

extern "C" cudaError_t fastpq_lde_cuda(
    const uint64_t* coeffs,
    size_t column_count,
    uint32_t trace_log,
    uint32_t blowup_log,
    uint64_t lde_root,
    uint64_t coset,
    uint64_t* out
) {
    if (coeffs == nullptr || out == nullptr || column_count == 0) {
        return cudaErrorInvalidValue;
    }
    uint64_t trace_len = 1ull << trace_log;
    uint64_t eval_len = 1ull << (trace_log + blowup_log);
    size_t coeff_byte_len = 0;
    cudaError_t status = dense_buffer_bytes(column_count, trace_len, &coeff_byte_len);
    if (status != cudaSuccess) {
        return status;
    }
    size_t eval_byte_len = 0;
    status = dense_buffer_bytes(column_count, eval_len, &eval_byte_len);
    if (status != cudaSuccess) {
        return status;
    }
    std::lock_guard<std::mutex> guard(transform_workspace_mutex());
    TransformWorkspace* workspace = current_transform_workspace();
    if (workspace == nullptr) {
        return cudaErrorInvalidValue;
    }
    status = ensure_workspace_buffer(
        &workspace->coeffs,
        &workspace->coeff_capacity_bytes,
        coeff_byte_len
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = ensure_workspace_buffer(
        &workspace->evals,
        &workspace->eval_capacity_bytes,
        eval_byte_len
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMemcpy(workspace->coeffs, coeffs, coeff_byte_len, cudaMemcpyHostToDevice);
    if (status != cudaSuccess) {
        return status;
    }
    if (column_count > (size_t)UINT32_MAX) {
        return cudaErrorInvalidValue;
    }
    fastpq_lde_kernel<<<(unsigned int)column_count, 1>>>(
        workspace->coeffs,
        workspace->evals,
        trace_len,
        eval_len,
        trace_log + blowup_log,
        lde_root,
        coset
    );
    status = cudaGetLastError();
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMemcpy(out, workspace->evals, eval_byte_len, cudaMemcpyDeviceToHost);
    if (status != cudaSuccess) {
        return status;
    }
    return cudaSuccess;
}

extern "C" cudaError_t fastpq_bn254_fft_cuda(
    uint64_t* elements,
    size_t column_count,
    uint32_t log_size,
    const uint64_t* stage_twiddles,
    size_t stage_twiddle_len
) {
    if (elements == nullptr || stage_twiddles == nullptr || column_count == 0 || stage_twiddle_len == 0) {
        return cudaErrorInvalidValue;
    }
    uint64_t n = 1ull << log_size;
    size_t dense_byte_len = 0;
    cudaError_t status = bn254_dense_buffer_bytes(column_count, n, &dense_byte_len);
    if (status != cudaSuccess) {
        return status;
    }
    size_t twiddle_byte_len = 0;
    status = limb_buffer_bytes(stage_twiddle_len, &twiddle_byte_len);
    if (status != cudaSuccess) {
        return status;
    }
    std::lock_guard<std::mutex> guard(transform_workspace_mutex());
    TransformWorkspace* workspace = current_transform_workspace();
    if (workspace == nullptr) {
        return cudaErrorInvalidValue;
    }
    status = ensure_workspace_buffer(
        &workspace->dense,
        &workspace->dense_capacity_bytes,
        dense_byte_len
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = ensure_workspace_buffer(
        &workspace->bn254_twiddles,
        &workspace->bn254_twiddle_capacity_bytes,
        twiddle_byte_len
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMemcpy(workspace->dense, elements, dense_byte_len, cudaMemcpyHostToDevice);
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMemcpy(
        workspace->bn254_twiddles,
        stage_twiddles,
        twiddle_byte_len,
        cudaMemcpyHostToDevice
    );
    if (status != cudaSuccess) {
        return status;
    }
    if (column_count > (size_t)UINT32_MAX) {
        return cudaErrorInvalidValue;
    }
    const unsigned int threads_per_block = 128u;
    fastpq_bn254_fft_kernel<<<(unsigned int)column_count, threads_per_block>>>(
        workspace->dense,
        column_count,
        n,
        log_size,
        workspace->bn254_twiddles
    );
    status = cudaGetLastError();
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMemcpy(elements, workspace->dense, dense_byte_len, cudaMemcpyDeviceToHost);
    if (status != cudaSuccess) {
        return status;
    }
    return cudaSuccess;
}

extern "C" cudaError_t fastpq_bn254_lde_cuda(
    const uint64_t* coeffs,
    size_t column_count,
    uint32_t trace_log,
    uint32_t blowup_log,
    const uint64_t* stage_twiddles,
    size_t stage_twiddle_len,
    const uint64_t* coset,
    uint64_t* out
) {
    if (
        coeffs == nullptr || stage_twiddles == nullptr || coset == nullptr || out == nullptr
        || column_count == 0 || stage_twiddle_len == 0
    ) {
        return cudaErrorInvalidValue;
    }
    uint64_t trace_len = 1ull << trace_log;
    uint64_t eval_len = 1ull << (trace_log + blowup_log);
    size_t coeff_byte_len = 0;
    cudaError_t status = bn254_dense_buffer_bytes(column_count, trace_len, &coeff_byte_len);
    if (status != cudaSuccess) {
        return status;
    }
    size_t eval_byte_len = 0;
    status = bn254_dense_buffer_bytes(column_count, eval_len, &eval_byte_len);
    if (status != cudaSuccess) {
        return status;
    }
    size_t twiddle_byte_len = 0;
    status = limb_buffer_bytes(stage_twiddle_len, &twiddle_byte_len);
    if (status != cudaSuccess) {
        return status;
    }
    size_t coset_byte_len = 0;
    status = limb_buffer_bytes(BN254_LIMBS, &coset_byte_len);
    if (status != cudaSuccess) {
        return status;
    }
    std::lock_guard<std::mutex> guard(transform_workspace_mutex());
    TransformWorkspace* workspace = current_transform_workspace();
    if (workspace == nullptr) {
        return cudaErrorInvalidValue;
    }
    status = ensure_workspace_buffer(
        &workspace->coeffs,
        &workspace->coeff_capacity_bytes,
        coeff_byte_len
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = ensure_workspace_buffer(
        &workspace->evals,
        &workspace->eval_capacity_bytes,
        eval_byte_len
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = ensure_workspace_buffer(
        &workspace->bn254_twiddles,
        &workspace->bn254_twiddle_capacity_bytes,
        twiddle_byte_len
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = ensure_workspace_buffer(
        &workspace->bn254_coset,
        &workspace->bn254_coset_capacity_bytes,
        coset_byte_len
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMemcpy(workspace->coeffs, coeffs, coeff_byte_len, cudaMemcpyHostToDevice);
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMemcpy(
        workspace->bn254_twiddles,
        stage_twiddles,
        twiddle_byte_len,
        cudaMemcpyHostToDevice
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMemcpy(workspace->bn254_coset, coset, coset_byte_len, cudaMemcpyHostToDevice);
    if (status != cudaSuccess) {
        return status;
    }
    if (column_count > (size_t)UINT32_MAX) {
        return cudaErrorInvalidValue;
    }
    const unsigned int threads_per_block = 128u;
    fastpq_bn254_lde_kernel<<<(unsigned int)column_count, threads_per_block>>>(
        workspace->coeffs,
        workspace->evals,
        trace_len,
        eval_len,
        trace_log,
        blowup_log,
        workspace->bn254_twiddles,
        workspace->bn254_coset
    );
    status = cudaGetLastError();
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMemcpy(out, workspace->evals, eval_byte_len, cudaMemcpyDeviceToHost);
    if (status != cudaSuccess) {
        return status;
    }
    return cudaSuccess;
}

extern "C" cudaError_t fastpq_poseidon_permute_cuda(uint64_t* states, size_t state_count) {
    if (states == nullptr || state_count == 0) {
        return cudaErrorInvalidValue;
    }

    const size_t total_elements = state_count * (size_t)POSEIDON_STATE_WIDTH;
    const size_t byte_len = total_elements * sizeof(uint64_t);

    std::lock_guard<std::mutex> guard(poseidon_workspace_mutex());
    PoseidonWorkspace* workspace = current_poseidon_workspace();
    if (workspace == nullptr) {
        return cudaErrorInvalidValue;
    }
    cudaError_t status = ensure_workspace_buffer(
        &workspace->states,
        &workspace->state_capacity_bytes,
        byte_len
    );
    if (status != cudaSuccess) {
        return status;
    }
    uint64_t* device_states = workspace->states;

    status = cudaMemcpy(device_states, states, byte_len, cudaMemcpyHostToDevice);
    if (status != cudaSuccess) {
        return status;
    }

    const unsigned int threads_per_block = 128u;
    size_t grid_size_host = (state_count + threads_per_block - 1u) / threads_per_block;
    if (grid_size_host == 0) {
        grid_size_host = 1;
    }
    if (grid_size_host > (size_t)UINT32_MAX) {
        return cudaErrorInvalidValue;
    }

    fastpq_poseidon_permute_kernel<<<(unsigned int)grid_size_host, threads_per_block>>>(
        device_states,
        state_count
    );
    status = cudaGetLastError();
    if (status == cudaSuccess) {
        status = cudaMemcpy(states, device_states, byte_len, cudaMemcpyDeviceToHost);
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

    std::lock_guard<std::mutex> guard(poseidon_workspace_mutex());
    PoseidonWorkspace* workspace = current_poseidon_workspace();
    if (workspace == nullptr) {
        return cudaErrorInvalidValue;
    }
    cudaError_t status = ensure_workspace_buffer(
        &workspace->payloads,
        &workspace->payload_capacity_bytes,
        payload_bytes
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = ensure_workspace_buffer(
        &workspace->slices,
        &workspace->slice_capacity_bytes,
        column_count * sizeof(PoseidonSlice)
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = ensure_workspace_buffer(
        &workspace->states,
        &workspace->state_capacity_bytes,
        state_bytes
    );
    if (status != cudaSuccess) {
        return status;
    }
    uint64_t* device_payloads = workspace->payloads;
    PoseidonSlice* device_slices = workspace->slices;
    uint64_t* device_states = workspace->states;

    status = cudaMemcpy(device_payloads, payloads, payload_bytes, cudaMemcpyHostToDevice);
    if (status != cudaSuccess) {
        return status;
    }
    status = cudaMemcpy(
        device_slices,
        slices,
        column_count * sizeof(PoseidonSlice),
        cudaMemcpyHostToDevice
    );
    if (status != cudaSuccess) {
        return status;
    }

    const unsigned int threads_per_block = 128u;
    size_t grid_size_host = (column_count + threads_per_block - 1u) / threads_per_block;
    if (grid_size_host == 0) {
        grid_size_host = 1;
    }
    if (grid_size_host > (size_t)UINT32_MAX) {
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
        status = cudaMemcpy(out_states, device_states, state_bytes, cudaMemcpyDeviceToHost);
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

    const unsigned int threads_per_block = 128u;
    size_t grid_size_host = 0;
    size_t parent_grid = 0;
    std::lock_guard<std::mutex> guard(poseidon_workspace_mutex());
    PoseidonWorkspace* workspace = current_poseidon_workspace();
    if (workspace == nullptr) {
        return cudaErrorInvalidValue;
    }
    cudaError_t status = ensure_workspace_buffer(
        &workspace->payloads,
        &workspace->payload_capacity_bytes,
        payload_bytes
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = ensure_workspace_buffer(
        &workspace->slices,
        &workspace->slice_capacity_bytes,
        slice_bytes
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = ensure_workspace_buffer(
        &workspace->states,
        &workspace->state_capacity_bytes,
        state_bytes
    );
    if (status != cudaSuccess) {
        return status;
    }
    status = ensure_workspace_buffer(
        &workspace->hashes,
        &workspace->hash_capacity_bytes,
        hash_bytes
    );
    if (status != cudaSuccess) {
        return status;
    }
    uint64_t* device_payloads = workspace->payloads;
    PoseidonSlice* device_slices = workspace->slices;
    uint64_t* device_states = workspace->states;
    uint64_t* device_hashes = workspace->hashes;

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

    grid_size_host = (column_count + threads_per_block - 1u) / threads_per_block;
    if (grid_size_host == 0) {
        grid_size_host = 1;
    }
    if (grid_size_host > (size_t)UINT32_MAX) {
        status = cudaErrorInvalidValue;
        goto fused_cleanup;
    }
    fastpq_poseidon_hash_columns_kernel<<<(unsigned int)grid_size_host, threads_per_block>>>(
        device_payloads,
        device_slices,
        block_count,
        column_count,
        device_states
    );
    status = cudaGetLastError();
    if (status != cudaSuccess) {
        goto fused_cleanup;
    }

    fastpq_poseidon_leaf_extract_kernel<<<(unsigned int)grid_size_host, threads_per_block>>>(
        device_states,
        column_count,
        device_hashes
    );
    status = cudaGetLastError();
    if (status != cudaSuccess) {
        goto fused_cleanup;
    }

    parent_grid = (parents + threads_per_block - 1u) / threads_per_block;
    if (parent_grid == 0) {
        parent_grid = 1;
    }
    if (parent_grid > (size_t)UINT32_MAX) {
        status = cudaErrorInvalidValue;
        goto fused_cleanup;
    }
    fastpq_poseidon_parent_kernel<<<(unsigned int)parent_grid, threads_per_block>>>(
        device_hashes,
        column_count,
        device_hashes + column_count
    );
    status = cudaGetLastError();
    if (status != cudaSuccess) {
        goto fused_cleanup;
    }

    status = cudaMemcpy(out_hashes, device_hashes, hash_bytes, cudaMemcpyDeviceToHost);

fused_cleanup:
    return status;
}
