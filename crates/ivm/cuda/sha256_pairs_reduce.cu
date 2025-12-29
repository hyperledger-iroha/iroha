#include <stdint.h>

__device__ __forceinline__ uint32_t rotr(uint32_t x, int n) {
    return (x >> n) | (x << (32 - n));
}

extern "C" __global__ void sha256_pairs_reduce(const uint8_t* in_digests, uint8_t* out_digests, uint32_t count) {
    const uint32_t K[64] = {
        0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
        0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
        0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
        0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
        0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
        0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
        0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
        0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
    };
    const uint32_t H0[8] = {
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19
    };

    uint32_t tid = blockIdx.x * blockDim.x + threadIdx.x;
    uint32_t left_idx = tid * 2u;
    if (left_idx >= count) return;
    uint32_t right_idx = left_idx + 1u;

    const uint8_t* left = in_digests + (size_t)left_idx * 32;
    const uint8_t* right = right_idx < count ? (in_digests + (size_t)right_idx * 32) : nullptr;
    uint8_t* out = out_digests + (size_t)tid * 32;

    if (!right) {
        // Left-promotion: parent = left digest
        for (int i = 0; i < 32; ++i) out[i] = left[i];
        return;
    }

    // Compute SHA-256(left||right) over 64 bytes (requires two compression rounds).
    // First block = left||right interpreted as big-endian words.
    uint32_t w[64];
    #pragma unroll
    for (int t = 0; t < 16; ++t) {
        const uint8_t* p = (t < 8) ? (left + t*4) : (right + (t-8)*4);
        w[t] = ((uint32_t)p[0] << 24) | ((uint32_t)p[1] << 16) | ((uint32_t)p[2] << 8) | (uint32_t)p[3];
    }
    #pragma unroll
    for (int t = 16; t < 64; ++t) {
        uint32_t x = w[t-15];
        uint32_t s0 = rotr(x,7) ^ rotr(x,18) ^ (x >> 3);
        uint32_t y = w[t-2];
        uint32_t s1 = rotr(y,17) ^ rotr(y,19) ^ (y >> 10);
        w[t] = w[t-16] + s0 + w[t-7] + s1;
    }
    uint32_t a=H0[0], b=H0[1], c=H0[2], d=H0[3], e=H0[4], f=H0[5], g=H0[6], h=H0[7];
    #pragma unroll
    for (int t = 0; t < 64; ++t) {
        uint32_t S1 = rotr(e,6) ^ rotr(e,11) ^ rotr(e,25);
        uint32_t ch = (e & f) ^ ((~e) & g);
        uint32_t temp1 = h + S1 + ch + K[t] + w[t];
        uint32_t S0 = rotr(a,2) ^ rotr(a,13) ^ rotr(a,22);
        uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
        uint32_t temp2 = S0 + maj;
        h = g; g = f; f = e; e = d + temp1; d = c; c = b; b = a; a = temp1 + temp2;
    }
    uint32_t st0 = H0[0] + a;
    uint32_t st1 = H0[1] + b;
    uint32_t st2 = H0[2] + c;
    uint32_t st3 = H0[3] + d;
    uint32_t st4 = H0[4] + e;
    uint32_t st5 = H0[5] + f;
    uint32_t st6 = H0[6] + g;
    uint32_t st7 = H0[7] + h;

    // Second block = padding for 64-byte message: 0x80 .. 0 .. 512-bit length (0x0000000000000200)
    uint32_t wp[64];
    wp[0] = 0x80000000u;
    for (int i = 1; i < 15; ++i) wp[i] = 0;
    wp[15] = 512u; // 64 bytes * 8
    #pragma unroll
    for (int t = 16; t < 64; ++t) {
        uint32_t x = wp[t-15];
        uint32_t s0 = rotr(x,7) ^ rotr(x,18) ^ (x >> 3);
        uint32_t y = wp[t-2];
        uint32_t s1 = rotr(y,17) ^ rotr(y,19) ^ (y >> 10);
        wp[t] = wp[t-16] + s0 + wp[t-7] + s1;
    }
    a=st0; b=st1; c=st2; d=st3; e=st4; f=st5; g=st6; h=st7;
    #pragma unroll
    for (int t = 0; t < 64; ++t) {
        uint32_t S1 = rotr(e,6) ^ rotr(e,11) ^ rotr(e,25);
        uint32_t ch = (e & f) ^ ((~e) & g);
        uint32_t temp1 = h + S1 + ch + K[t] + wp[t];
        uint32_t S0 = rotr(a,2) ^ rotr(a,13) ^ rotr(a,22);
        uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
        uint32_t temp2 = S0 + maj;
        h = g; g = f; f = e; e = d + temp1; d = c; c = b; b = a; a = temp1 + temp2;
    }
    st0 += a; st1 += b; st2 += c; st3 += d; st4 += e; st5 += f; st6 += g; st7 += h;

    // Store digest big-endian
    uint8_t* dst = out;
    uint32_t st[8] = {st0,st1,st2,st3,st4,st5,st6,st7};
    for (int j = 0; j < 8; ++j) {
        uint32_t wj = st[j];
        dst[j*4+0] = (uint8_t)(wj >> 24);
        dst[j*4+1] = (uint8_t)(wj >> 16);
        dst[j*4+2] = (uint8_t)(wj >> 8);
        dst[j*4+3] = (uint8_t)(wj);
    }
}

