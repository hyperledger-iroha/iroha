#include <stdint.h>

// Compute SHA-256 digest for an array of 64-byte blocks, one block per thread.
// Each block is a fully padded single-block message (e.g., data of length L<=55,
// followed by 0x80 and zeros, and 64-bit big-endian bit length at the end).
// Output is 8 x u32 state words per block, written in array-of-structs order.
extern "C" __global__ void sha256_leaves(const uint8_t* blocks, uint32_t* out_states, uint32_t count) {
    const uint32_t K[64] = {
        0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,
        0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
        0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,
        0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
        0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,
        0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
        0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,
        0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
        0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,
        0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
        0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,
        0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
        0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,
        0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
        0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,
        0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
    };
    const uint32_t H0[8] = {
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19
    };

    uint32_t i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i >= count) return;

    const uint8_t* block = blocks + (size_t)i * 64;

    uint32_t w[64];
    #pragma unroll
    for (int t = 0; t < 16; ++t) {
        w[t] = ((uint32_t)block[t*4] << 24) | ((uint32_t)block[t*4+1] << 16) |
               ((uint32_t)block[t*4+2] << 8) | (uint32_t)block[t*4+3];
    }
    #pragma unroll
    for (int t = 16; t < 64; ++t) {
        uint32_t x = w[t-15];
        uint32_t s0 = __funnelshift_r(x, x, 7) ^ __funnelshift_r(x, x, 18) ^ (x >> 3);
        uint32_t y = w[t-2];
        uint32_t s1 = __funnelshift_r(y, y, 17) ^ __funnelshift_r(y, y, 19) ^ (y >> 10);
        w[t] = w[t-16] + s0 + w[t-7] + s1;
    }
    uint32_t a=H0[0], b=H0[1], c=H0[2], d=H0[3], e=H0[4], f=H0[5], g=H0[6], h=H0[7];
    #pragma unroll
    for (int t = 0; t < 64; ++t) {
        uint32_t S1 = __funnelshift_r(e,e,6) ^ __funnelshift_r(e,e,11) ^ __funnelshift_r(e,e,25);
        uint32_t ch = (e & f) ^ ((~e) & g);
        uint32_t temp1 = h + S1 + ch + K[t] + w[t];
        uint32_t S0 = __funnelshift_r(a,a,2) ^ __funnelshift_r(a,a,13) ^ __funnelshift_r(a,a,22);
        uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
        uint32_t temp2 = S0 + maj;
        h = g; g = f; f = e; e = d + temp1; d = c; c = b; b = a; a = temp1 + temp2;
    }
    uint32_t out0 = H0[0] + a;
    uint32_t out1 = H0[1] + b;
    uint32_t out2 = H0[2] + c;
    uint32_t out3 = H0[3] + d;
    uint32_t out4 = H0[4] + e;
    uint32_t out5 = H0[5] + f;
    uint32_t out6 = H0[6] + g;
    uint32_t out7 = H0[7] + h;

    uint32_t* dst = out_states + (size_t)i * 8;
    dst[0] = out0; dst[1] = out1; dst[2] = out2; dst[3] = out3;
    dst[4] = out4; dst[5] = out5; dst[6] = out6; dst[7] = out7;
}

