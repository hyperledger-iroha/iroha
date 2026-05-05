#include <stdint.h>
#include <stdbool.h>

typedef int32_t fe[10];

__device__ __forceinline__ int64_t load3(const uint8_t* in) {
    return in[0] | ((int64_t)in[1] << 8) | ((int64_t)in[2] << 16);
}

__device__ __forceinline__ int64_t load4(const uint8_t* in) {
    return in[0] | ((int64_t)in[1] << 8) | ((int64_t)in[2] << 16) | ((int64_t)in[3] << 24);
}

__device__ __constant__ int32_t FE_D[10] = {
    -10913610, 13857413, -15372611, 6949391,   114729,
    -8787816,  -6275908, -3247719,  -18696448, -12055116};

__device__ __constant__ int32_t FE_D2[10] = {
    -21827239, -5839606, -30745221, 13898782, 229458,
    15978800,  -12551817, -6495438, 29715968, 9444199};

__device__ __constant__ int32_t FE_SQRTM1[10] = {
    -32595792, -7943725, 9377950, 3500415, 12389472,
    -272473,   -25146209, -2005654, 326686,  11406482};

__device__ __constant__ uint8_t BASE_POINT_BYTES[32] = {
    0x58, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66};

__device__ __forceinline__ void fe_0(fe h) {
#pragma unroll
    for (int i = 0; i < 10; ++i) {
        h[i] = 0;
    }
}

__device__ __forceinline__ void fe_1(fe h) {
    fe_0(h);
    h[0] = 1;
}

__device__ __forceinline__ void fe_copy(fe h, const fe f) {
#pragma unroll
    for (int i = 0; i < 10; ++i) {
        h[i] = f[i];
    }
}

__device__ void fe_normalize(fe h);

__device__ __forceinline__ void fe_add(fe h, const fe f, const fe g) {
#pragma unroll
    for (int i = 0; i < 10; ++i) {
        h[i] = f[i] + g[i];
    }
    fe_normalize(h);
}

__device__ __forceinline__ void fe_sub(fe h, const fe f, const fe g) {
#pragma unroll
    for (int i = 0; i < 10; ++i) {
        h[i] = f[i] - g[i];
    }
    fe_normalize(h);
}

__device__ __forceinline__ void fe_neg(fe h, const fe f) {
#pragma unroll
    for (int i = 0; i < 10; ++i) {
        h[i] = -f[i];
    }
    fe_normalize(h);
}

__device__ __forceinline__ void fe_cmov(fe f, const fe g, int b) {
    int32_t mask = -b;
#pragma unroll
    for (int i = 0; i < 10; ++i) {
        int32_t t = f[i] ^ g[i];
        t &= mask;
        f[i] ^= t;
    }
}

__device__ void fe_normalize(fe h) {
    int64_t t[10];
#pragma unroll
    for (int i = 0; i < 10; ++i) {
        t[i] = h[i];
    }

    int64_t carry0 = (t[0] + (int64_t)(1 << 25)) >> 26;
    t[1] += carry0;
    t[0] -= carry0 << 26;
    int64_t carry4 = (t[4] + (int64_t)(1 << 25)) >> 26;
    t[5] += carry4;
    t[4] -= carry4 << 26;
    int64_t carry1 = (t[1] + (int64_t)(1 << 24)) >> 25;
    t[2] += carry1;
    t[1] -= carry1 << 25;
    int64_t carry5 = (t[5] + (int64_t)(1 << 24)) >> 25;
    t[6] += carry5;
    t[5] -= carry5 << 25;
    int64_t carry2 = (t[2] + (int64_t)(1 << 25)) >> 26;
    t[3] += carry2;
    t[2] -= carry2 << 26;
    int64_t carry6 = (t[6] + (int64_t)(1 << 25)) >> 26;
    t[7] += carry6;
    t[6] -= carry6 << 26;
    int64_t carry3 = (t[3] + (int64_t)(1 << 24)) >> 25;
    t[4] += carry3;
    t[3] -= carry3 << 25;
    int64_t carry7 = (t[7] + (int64_t)(1 << 24)) >> 25;
    t[8] += carry7;
    t[7] -= carry7 << 25;
    carry4 = (t[4] + (int64_t)(1 << 25)) >> 26;
    t[5] += carry4;
    t[4] -= carry4 << 26;
    int64_t carry8 = (t[8] + (int64_t)(1 << 25)) >> 26;
    t[9] += carry8;
    t[8] -= carry8 << 26;
    int64_t carry9 = (t[9] + (int64_t)(1 << 24)) >> 25;
    t[0] += carry9 * 19;
    t[9] -= carry9 << 25;
    carry0 = (t[0] + (int64_t)(1 << 25)) >> 26;
    t[1] += carry0;
    t[0] -= carry0 << 26;

#pragma unroll
    for (int i = 0; i < 10; ++i) {
        h[i] = (int32_t)t[i];
    }
}

__device__ void fe_mul(fe h, const fe f, const fe g) {
    int32_t f0 = f[0];
    int32_t f1 = f[1];
    int32_t f2 = f[2];
    int32_t f3 = f[3];
    int32_t f4 = f[4];
    int32_t f5 = f[5];
    int32_t f6 = f[6];
    int32_t f7 = f[7];
    int32_t f8 = f[8];
    int32_t f9 = f[9];
    int32_t g0 = g[0];
    int32_t g1 = g[1];
    int32_t g2 = g[2];
    int32_t g3 = g[3];
    int32_t g4 = g[4];
    int32_t g5 = g[5];
    int32_t g6 = g[6];
    int32_t g7 = g[7];
    int32_t g8 = g[8];
    int32_t g9 = g[9];
    int32_t g1_19 = 19 * g1;
    int32_t g2_19 = 19 * g2;
    int32_t g3_19 = 19 * g3;
    int32_t g4_19 = 19 * g4;
    int32_t g5_19 = 19 * g5;
    int32_t g6_19 = 19 * g6;
    int32_t g7_19 = 19 * g7;
    int32_t g8_19 = 19 * g8;
    int32_t g9_19 = 19 * g9;
    int32_t f1_2 = 2 * f1;
    int32_t f3_2 = 2 * f3;
    int32_t f5_2 = 2 * f5;
    int32_t f7_2 = 2 * f7;
    int32_t f9_2 = 2 * f9;

    int64_t h0 = (int64_t)f0 * g0 + (int64_t)f1_2 * g9_19 + (int64_t)f2 * g8_19 +
                (int64_t)f3_2 * g7_19 + (int64_t)f4 * g6_19 + (int64_t)f5_2 * g5_19 +
                (int64_t)f6 * g4_19 + (int64_t)f7_2 * g3_19 + (int64_t)f8 * g2_19 +
                (int64_t)f9_2 * g1_19;
    int64_t h1 = (int64_t)f0 * g1 + (int64_t)f1 * g0 + (int64_t)f2 * g9_19 +
                (int64_t)f3 * g8_19 + (int64_t)f4 * g7_19 + (int64_t)f5 * g6_19 +
                (int64_t)f6 * g5_19 + (int64_t)f7 * g4_19 + (int64_t)f8 * g3_19 +
                (int64_t)f9 * g2_19;
    int64_t h2 = (int64_t)f0 * g2 + (int64_t)f1_2 * g1 + (int64_t)f2 * g0 +
                (int64_t)f3_2 * g9_19 + (int64_t)f4 * g8_19 + (int64_t)f5_2 * g7_19 +
                (int64_t)f6 * g6_19 + (int64_t)f7_2 * g5_19 + (int64_t)f8 * g4_19 +
                (int64_t)f9_2 * g3_19;
    int64_t h3 = (int64_t)f0 * g3 + (int64_t)f1 * g2 + (int64_t)f2 * g1 + (int64_t)f3 * g0 +
                (int64_t)f4 * g9_19 + (int64_t)f5 * g8_19 + (int64_t)f6 * g7_19 +
                (int64_t)f7 * g6_19 + (int64_t)f8 * g5_19 + (int64_t)f9 * g4_19;
    int64_t h4 = (int64_t)f0 * g4 + (int64_t)f1_2 * g3 + (int64_t)f2 * g2 +
                (int64_t)f3_2 * g1 + (int64_t)f4 * g0 + (int64_t)f5_2 * g9_19 +
                (int64_t)f6 * g8_19 + (int64_t)f7_2 * g7_19 + (int64_t)f8 * g6_19 +
                (int64_t)f9_2 * g5_19;
    int64_t h5 = (int64_t)f0 * g5 + (int64_t)f1 * g4 + (int64_t)f2 * g3 + (int64_t)f3 * g2 +
                (int64_t)f4 * g1 + (int64_t)f5 * g0 + (int64_t)f6 * g9_19 +
                (int64_t)f7 * g8_19 + (int64_t)f8 * g7_19 + (int64_t)f9 * g6_19;
    int64_t h6 = (int64_t)f0 * g6 + (int64_t)f1_2 * g5 + (int64_t)f2 * g4 +
                (int64_t)f3_2 * g3 + (int64_t)f4 * g2 + (int64_t)f5_2 * g1 +
                (int64_t)f6 * g0 + (int64_t)f7_2 * g9_19 + (int64_t)f8 * g8_19 +
                (int64_t)f9_2 * g7_19;
    int64_t h7 = (int64_t)f0 * g7 + (int64_t)f1 * g6 + (int64_t)f2 * g5 + (int64_t)f3 * g4 +
                (int64_t)f4 * g3 + (int64_t)f5 * g2 + (int64_t)f6 * g1 + (int64_t)f7 * g0 +
                (int64_t)f8 * g9_19 + (int64_t)f9 * g8_19;
    int64_t h8 = (int64_t)f0 * g8 + (int64_t)f1_2 * g7 + (int64_t)f2 * g6 +
                (int64_t)f3_2 * g5 + (int64_t)f4 * g4 + (int64_t)f5_2 * g3 +
                (int64_t)f6 * g2 + (int64_t)f7_2 * g1 + (int64_t)f8 * g0 +
                (int64_t)f9_2 * g9_19;
    int64_t h9 = (int64_t)f0 * g9 + (int64_t)f1 * g8 + (int64_t)f2 * g7 + (int64_t)f3 * g6 +
                (int64_t)f4 * g5 + (int64_t)f5 * g4 + (int64_t)f6 * g3 + (int64_t)f7 * g2 +
                (int64_t)f8 * g1 + (int64_t)f9 * g0;

    int64_t carry0 = (h0 + (int64_t)(1 << 25)) >> 26;
    h1 += carry0;
    h0 -= carry0 << 26;
    int64_t carry4 = (h4 + (int64_t)(1 << 25)) >> 26;
    h5 += carry4;
    h4 -= carry4 << 26;
    int64_t carry1 = (h1 + (int64_t)(1 << 24)) >> 25;
    h2 += carry1;
    h1 -= carry1 << 25;
    int64_t carry5 = (h5 + (int64_t)(1 << 24)) >> 25;
    h6 += carry5;
    h5 -= carry5 << 25;
    int64_t carry2 = (h2 + (int64_t)(1 << 25)) >> 26;
    h3 += carry2;
    h2 -= carry2 << 26;
    int64_t carry6 = (h6 + (int64_t)(1 << 25)) >> 26;
    h7 += carry6;
    h6 -= carry6 << 26;
    int64_t carry3 = (h3 + (int64_t)(1 << 24)) >> 25;
    h4 += carry3;
    h3 -= carry3 << 25;
    int64_t carry7 = (h7 + (int64_t)(1 << 24)) >> 25;
    h8 += carry7;
    h7 -= carry7 << 25;
    int64_t carry8 = (h8 + (int64_t)(1 << 25)) >> 26;
    h9 += carry8;
    h8 -= carry8 << 26;
    int64_t carry9 = (h9 + (int64_t)(1 << 24)) >> 25;
    h0 += carry9 * 19;
    h9 -= carry9 << 25;
    carry0 = (h0 + (int64_t)(1 << 25)) >> 26;
    h1 += carry0;
    h0 -= carry0 << 26;

    h[0] = (int32_t)h0;
    h[1] = (int32_t)h1;
    h[2] = (int32_t)h2;
    h[3] = (int32_t)h3;
    h[4] = (int32_t)h4;
    h[5] = (int32_t)h5;
    h[6] = (int32_t)h6;
    h[7] = (int32_t)h7;
    h[8] = (int32_t)h8;
    h[9] = (int32_t)h9;
    fe_normalize(h);
}

__device__ void fe_sq(fe h, const fe f) {
    int32_t f0 = f[0];
    int32_t f1 = f[1];
    int32_t f2 = f[2];
    int32_t f3 = f[3];
    int32_t f4 = f[4];
    int32_t f5 = f[5];
    int32_t f6 = f[6];
    int32_t f7 = f[7];
    int32_t f8 = f[8];
    int32_t f9 = f[9];
    int32_t f0_2 = 2 * f0;
    int32_t f1_2 = 2 * f1;
    int32_t f2_2 = 2 * f2;
    int32_t f3_2 = 2 * f3;
    int32_t f4_2 = 2 * f4;
    int32_t f5_2 = 2 * f5;
    int32_t f6_2 = 2 * f6;
    int32_t f7_2 = 2 * f7;
    int32_t f5_38 = 38 * f5;
    int32_t f6_19 = 19 * f6;
    int32_t f7_38 = 38 * f7;
    int32_t f8_19 = 19 * f8;
    int32_t f9_38 = 38 * f9;
    int64_t f0f0 = (int64_t)f0 * f0;
    int64_t f0f1_2 = (int64_t)f0_2 * f1;
    int64_t f0f2_2 = (int64_t)f0_2 * f2;
    int64_t f0f3_2 = (int64_t)f0_2 * f3;
    int64_t f0f4_2 = (int64_t)f0_2 * f4;
    int64_t f0f5_2 = (int64_t)f0_2 * f5;
    int64_t f0f6_2 = (int64_t)f0_2 * f6;
    int64_t f0f7_2 = (int64_t)f0_2 * f7;
    int64_t f0f8_2 = (int64_t)f0_2 * f8;
    int64_t f0f9_2 = (int64_t)f0_2 * f9;
    int64_t f1f1_2 = (int64_t)f1_2 * f1;
    int64_t f1f2_2 = (int64_t)f1_2 * f2;
    int64_t f1f3_4 = (int64_t)f1_2 * f3_2;
    int64_t f1f4_2 = (int64_t)f1_2 * f4;
    int64_t f1f5_4 = (int64_t)f1_2 * f5_2;
    int64_t f1f6_2 = (int64_t)f1_2 * f6;
    int64_t f1f7_4 = (int64_t)f1_2 * f7_2;
    int64_t f1f8_2 = (int64_t)f1_2 * f8;
    int64_t f1f9_76 = (int64_t)f1_2 * f9_38;
    int64_t f2f2 = (int64_t)f2 * f2;
    int64_t f2f3_2 = (int64_t)f2_2 * f3;
    int64_t f2f4_2 = (int64_t)f2_2 * f4;
    int64_t f2f5_2 = (int64_t)f2_2 * f5;
    int64_t f2f6_2 = (int64_t)f2_2 * f6;
    int64_t f2f7_2 = (int64_t)f2_2 * f7;
    int64_t f2f8_38 = (int64_t)f2_2 * f8_19;
    int64_t f2f9_38 = (int64_t)f2 * f9_38;
    int64_t f3f3_2 = (int64_t)f3_2 * f3;
    int64_t f3f4_2 = (int64_t)f3_2 * f4;
    int64_t f3f5_4 = (int64_t)f3_2 * f5_2;
    int64_t f3f6_2 = (int64_t)f3_2 * f6;
    int64_t f3f7_76 = (int64_t)f3_2 * f7_38;
    int64_t f3f8_38 = (int64_t)f3_2 * f8_19;
    int64_t f3f9_76 = (int64_t)f3_2 * f9_38;
    int64_t f4f4 = (int64_t)f4 * f4;
    int64_t f4f5_2 = (int64_t)f4_2 * f5;
    int64_t f4f6_38 = (int64_t)f4_2 * f6_19;
    int64_t f4f7_38 = (int64_t)f4 * f7_38;
    int64_t f4f8_38 = (int64_t)f4_2 * f8_19;
    int64_t f4f9_38 = (int64_t)f4 * f9_38;
    int64_t f5f5_38 = (int64_t)f5 * f5_38;
    int64_t f5f6_38 = (int64_t)f5_2 * f6_19;
    int64_t f5f7_76 = (int64_t)f5_2 * f7_38;
    int64_t f5f8_38 = (int64_t)f5_2 * f8_19;
    int64_t f5f9_76 = (int64_t)f5_2 * f9_38;
    int64_t f6f6_19 = (int64_t)f6 * f6_19;
    int64_t f6f7_38 = (int64_t)f6 * f7_38;
    int64_t f6f8_38 = (int64_t)f6_2 * f8_19;
    int64_t f6f9_38 = (int64_t)f6 * f9_38;
    int64_t f7f7_38 = (int64_t)f7 * f7_38;
    int64_t f7f8_38 = (int64_t)f7_2 * f8_19;
    int64_t f7f9_76 = (int64_t)f7_2 * f9_38;
    int64_t f8f8_19 = (int64_t)f8 * f8_19;
    int64_t f8f9_38 = (int64_t)f8 * f9_38;
    int64_t f9f9_38 = (int64_t)f9 * f9_38;
    int64_t h0 = f0f0 + f1f9_76 + f2f8_38 + f3f7_76 + f4f6_38 + f5f5_38;
    int64_t h1 = f0f1_2 + f2f9_38 + f3f8_38 + f4f7_38 + f5f6_38;
    int64_t h2 = f0f2_2 + f1f1_2 + f3f9_76 + f4f8_38 + f5f7_76 + f6f6_19;
    int64_t h3 = f0f3_2 + f1f2_2 + f4f9_38 + f5f8_38 + f6f7_38;
    int64_t h4 = f0f4_2 + f1f3_4 + f2f2 + f5f9_76 + f6f8_38 + f7f7_38;
    int64_t h5 = f0f5_2 + f1f4_2 + f2f3_2 + f6f9_38 + f7f8_38;
    int64_t h6 = f0f6_2 + f1f5_4 + f2f4_2 + f3f3_2 + f7f9_76 + f8f8_19;
    int64_t h7 = f0f7_2 + f1f6_2 + f2f5_2 + f3f4_2 + f8f9_38;
    int64_t h8 = f0f8_2 + f1f7_4 + f2f6_2 + f3f5_4 + f4f4 + f9f9_38;
    int64_t h9 = f0f9_2 + f1f8_2 + f2f7_2 + f3f6_2 + f4f5_2;
    int64_t carry0;
    int64_t carry1;
    int64_t carry2;
    int64_t carry3;
    int64_t carry4;
    int64_t carry5;
    int64_t carry6;
    int64_t carry7;
    int64_t carry8;
    int64_t carry9;

    carry0 = (h0 + (int64_t)(1 << 25)) >> 26;
    h1 += carry0;
    h0 -= carry0 << 26;
    carry4 = (h4 + (int64_t)(1 << 25)) >> 26;
    h5 += carry4;
    h4 -= carry4 << 26;
    carry1 = (h1 + (int64_t)(1 << 24)) >> 25;
    h2 += carry1;
    h1 -= carry1 << 25;
    carry5 = (h5 + (int64_t)(1 << 24)) >> 25;
    h6 += carry5;
    h5 -= carry5 << 25;
    carry2 = (h2 + (int64_t)(1 << 25)) >> 26;
    h3 += carry2;
    h2 -= carry2 << 26;
    carry6 = (h6 + (int64_t)(1 << 25)) >> 26;
    h7 += carry6;
    h6 -= carry6 << 26;
    carry3 = (h3 + (int64_t)(1 << 24)) >> 25;
    h4 += carry3;
    h3 -= carry3 << 25;
    carry7 = (h7 + (int64_t)(1 << 24)) >> 25;
    h8 += carry7;
    h7 -= carry7 << 25;
    carry4 = (h4 + (int64_t)(1 << 25)) >> 26;
    h5 += carry4;
    h4 -= carry4 << 26;
    carry8 = (h8 + (int64_t)(1 << 25)) >> 26;
    h9 += carry8;
    h8 -= carry8 << 26;
    carry9 = (h9 + (int64_t)(1 << 24)) >> 25;
    h0 += carry9 * 19;
    h9 -= carry9 << 25;
    carry0 = (h0 + (int64_t)(1 << 25)) >> 26;
    h1 += carry0;
    h0 -= carry0 << 26;

    h[0] = (int32_t)h0;
    h[1] = (int32_t)h1;
    h[2] = (int32_t)h2;
    h[3] = (int32_t)h3;
    h[4] = (int32_t)h4;
    h[5] = (int32_t)h5;
    h[6] = (int32_t)h6;
    h[7] = (int32_t)h7;
    h[8] = (int32_t)h8;
    h[9] = (int32_t)h9;
    fe_normalize(h);
}

__device__ void fe_sq2(fe h, const fe f) {
    int32_t f0 = f[0];
    int32_t f1 = f[1];
    int32_t f2 = f[2];
    int32_t f3 = f[3];
    int32_t f4 = f[4];
    int32_t f5 = f[5];
    int32_t f6 = f[6];
    int32_t f7 = f[7];
    int32_t f8 = f[8];
    int32_t f9 = f[9];
    int32_t f0_2 = 2 * f0;
    int32_t f1_2 = 2 * f1;
    int32_t f2_2 = 2 * f2;
    int32_t f3_2 = 2 * f3;
    int32_t f4_2 = 2 * f4;
    int32_t f5_2 = 2 * f5;
    int32_t f6_2 = 2 * f6;
    int32_t f7_2 = 2 * f7;
    int32_t f5_38 = 38 * f5;
    int32_t f6_19 = 19 * f6;
    int32_t f7_38 = 38 * f7;
    int32_t f8_19 = 19 * f8;
    int32_t f9_38 = 38 * f9;
    int64_t f0f0 = (int64_t)f0 * f0;
    int64_t f0f1_2 = (int64_t)f0_2 * f1;
    int64_t f0f2_2 = (int64_t)f0_2 * f2;
    int64_t f0f3_2 = (int64_t)f0_2 * f3;
    int64_t f0f4_2 = (int64_t)f0_2 * f4;
    int64_t f0f5_2 = (int64_t)f0_2 * f5;
    int64_t f0f6_2 = (int64_t)f0_2 * f6;
    int64_t f0f7_2 = (int64_t)f0_2 * f7;
    int64_t f0f8_2 = (int64_t)f0_2 * f8;
    int64_t f0f9_2 = (int64_t)f0_2 * f9;
    int64_t f1f1_2 = (int64_t)f1_2 * f1;
    int64_t f1f2_2 = (int64_t)f1_2 * f2;
    int64_t f1f3_4 = (int64_t)f1_2 * f3_2;
    int64_t f1f4_2 = (int64_t)f1_2 * f4;
    int64_t f1f5_4 = (int64_t)f1_2 * f5_2;
    int64_t f1f6_2 = (int64_t)f1_2 * f6;
    int64_t f1f7_4 = (int64_t)f1_2 * f7_2;
    int64_t f1f8_2 = (int64_t)f1_2 * f8;
    int64_t f1f9_76 = (int64_t)f1_2 * f9_38;
    int64_t f2f2 = (int64_t)f2 * f2;
    int64_t f2f3_2 = (int64_t)f2_2 * f3;
    int64_t f2f4_2 = (int64_t)f2_2 * f4;
    int64_t f2f5_2 = (int64_t)f2_2 * f5;
    int64_t f2f6_2 = (int64_t)f2_2 * f6;
    int64_t f2f7_2 = (int64_t)f2_2 * f7;
    int64_t f2f8_38 = (int64_t)f2_2 * f8_19;
    int64_t f2f9_38 = (int64_t)f2 * f9_38;
    int64_t f3f3_2 = (int64_t)f3_2 * f3;
    int64_t f3f4_2 = (int64_t)f3_2 * f4;
    int64_t f3f5_4 = (int64_t)f3_2 * f5_2;
    int64_t f3f6_2 = (int64_t)f3_2 * f6;
    int64_t f3f7_76 = (int64_t)f3_2 * f7_38;
    int64_t f3f8_38 = (int64_t)f3_2 * f8_19;
    int64_t f3f9_76 = (int64_t)f3_2 * f9_38;
    int64_t f4f4 = (int64_t)f4 * f4;
    int64_t f4f5_2 = (int64_t)f4_2 * f5;
    int64_t f4f6_38 = (int64_t)f4_2 * f6_19;
    int64_t f4f7_38 = (int64_t)f4 * f7_38;
    int64_t f4f8_38 = (int64_t)f4_2 * f8_19;
    int64_t f4f9_38 = (int64_t)f4 * f9_38;
    int64_t f5f5_38 = (int64_t)f5 * f5_38;
    int64_t f5f6_38 = (int64_t)f5_2 * f6_19;
    int64_t f5f7_76 = (int64_t)f5_2 * f7_38;
    int64_t f5f8_38 = (int64_t)f5_2 * f8_19;
    int64_t f5f9_76 = (int64_t)f5_2 * f9_38;
    int64_t f6f6_19 = (int64_t)f6 * f6_19;
    int64_t f6f7_38 = (int64_t)f6 * f7_38;
    int64_t f6f8_38 = (int64_t)f6_2 * f8_19;
    int64_t f6f9_38 = (int64_t)f6 * f9_38;
    int64_t f7f7_38 = (int64_t)f7 * f7_38;
    int64_t f7f8_38 = (int64_t)f7_2 * f8_19;
    int64_t f7f9_76 = (int64_t)f7_2 * f9_38;
    int64_t f8f8_19 = (int64_t)f8 * f8_19;
    int64_t f8f9_38 = (int64_t)f8 * f9_38;
    int64_t f9f9_38 = (int64_t)f9 * f9_38;
    int64_t h0 = f0f0 + f1f9_76 + f2f8_38 + f3f7_76 + f4f6_38 + f5f5_38;
    int64_t h1 = f0f1_2 + f2f9_38 + f3f8_38 + f4f7_38 + f5f6_38;
    int64_t h2 = f0f2_2 + f1f1_2 + f3f9_76 + f4f8_38 + f5f7_76 + f6f6_19;
    int64_t h3 = f0f3_2 + f1f2_2 + f4f9_38 + f5f8_38 + f6f7_38;
    int64_t h4 = f0f4_2 + f1f3_4 + f2f2 + f5f9_76 + f6f8_38 + f7f7_38;
    int64_t h5 = f0f5_2 + f1f4_2 + f2f3_2 + f6f9_38 + f7f8_38;
    int64_t h6 = f0f6_2 + f1f5_4 + f2f4_2 + f3f3_2 + f7f9_76 + f8f8_19;
    int64_t h7 = f0f7_2 + f1f6_2 + f2f5_2 + f3f4_2 + f8f9_38;
    int64_t h8 = f0f8_2 + f1f7_4 + f2f6_2 + f3f5_4 + f4f4 + f9f9_38;
    int64_t h9 = f0f9_2 + f1f8_2 + f2f7_2 + f3f6_2 + f4f5_2;
    int64_t carry0;
    int64_t carry1;
    int64_t carry2;
    int64_t carry3;
    int64_t carry4;
    int64_t carry5;
    int64_t carry6;
    int64_t carry7;
    int64_t carry8;
    int64_t carry9;

    h0 += h0;
    h1 += h1;
    h2 += h2;
    h3 += h3;
    h4 += h4;
    h5 += h5;
    h6 += h6;
    h7 += h7;
    h8 += h8;
    h9 += h9;

    carry0 = (h0 + (int64_t)(1 << 25)) >> 26;
    h1 += carry0;
    h0 -= carry0 << 26;
    carry4 = (h4 + (int64_t)(1 << 25)) >> 26;
    h5 += carry4;
    h4 -= carry4 << 26;
    carry1 = (h1 + (int64_t)(1 << 24)) >> 25;
    h2 += carry1;
    h1 -= carry1 << 25;
    carry5 = (h5 + (int64_t)(1 << 24)) >> 25;
    h6 += carry5;
    h5 -= carry5 << 25;
    carry2 = (h2 + (int64_t)(1 << 25)) >> 26;
    h3 += carry2;
    h2 -= carry2 << 26;
    carry6 = (h6 + (int64_t)(1 << 25)) >> 26;
    h7 += carry6;
    h6 -= carry6 << 26;
    carry3 = (h3 + (int64_t)(1 << 24)) >> 25;
    h4 += carry3;
    h3 -= carry3 << 25;
    carry7 = (h7 + (int64_t)(1 << 24)) >> 25;
    h8 += carry7;
    h7 -= carry7 << 25;
    carry4 = (h4 + (int64_t)(1 << 25)) >> 26;
    h5 += carry4;
    h4 -= carry4 << 26;
    carry8 = (h8 + (int64_t)(1 << 25)) >> 26;
    h9 += carry8;
    h8 -= carry8 << 26;
    carry9 = (h9 + (int64_t)(1 << 24)) >> 25;
    h0 += carry9 * 19;
    h9 -= carry9 << 25;
    carry0 = (h0 + (int64_t)(1 << 25)) >> 26;
    h1 += carry0;
    h0 -= carry0 << 26;

    h[0] = (int32_t)h0;
    h[1] = (int32_t)h1;
    h[2] = (int32_t)h2;
    h[3] = (int32_t)h3;
    h[4] = (int32_t)h4;
    h[5] = (int32_t)h5;
    h[6] = (int32_t)h6;
    h[7] = (int32_t)h7;
    h[8] = (int32_t)h8;
    h[9] = (int32_t)h9;
    fe_normalize(h);
}

__device__ void fe_mul121666(fe h, const fe f) {
    int64_t t[10];
    int64_t carry;
#pragma unroll
    for (int i = 0; i < 10; ++i) {
        t[i] = (int64_t)f[i] * 121666;
    }
    carry = (t[0] + (int64_t)(1 << 25)) >> 26;
    t[1] += carry;
    t[0] -= carry << 26;
    carry = (t[4] + (int64_t)(1 << 25)) >> 26;
    t[5] += carry;
    t[4] -= carry << 26;
    carry = (t[1] + (int64_t)(1 << 24)) >> 25;
    t[2] += carry;
    t[1] -= carry << 25;
    carry = (t[5] + (int64_t)(1 << 24)) >> 25;
    t[6] += carry;
    t[5] -= carry << 25;
    carry = (t[2] + (int64_t)(1 << 25)) >> 26;
    t[3] += carry;
    t[2] -= carry << 26;
    carry = (t[6] + (int64_t)(1 << 25)) >> 26;
    t[7] += carry;
    t[6] -= carry << 26;
    carry = (t[3] + (int64_t)(1 << 24)) >> 25;
    t[4] += carry;
    t[3] -= carry << 25;
    carry = (t[7] + (int64_t)(1 << 24)) >> 25;
    t[8] += carry;
    t[7] -= carry << 25;
    carry = (t[8] + (int64_t)(1 << 25)) >> 26;
    t[9] += carry;
    t[8] -= carry << 26;
    carry = (t[9] + (int64_t)(1 << 24)) >> 25;
    t[0] += carry * 19;
    t[9] -= carry << 25;
    carry = (t[0] + (int64_t)(1 << 25)) >> 26;
    t[1] += carry;
    t[0] -= carry << 26;
#pragma unroll
    for (int i = 0; i < 10; ++i) {
        h[i] = (int32_t)t[i];
    }
    fe_normalize(h);
}

__device__ void fe_pow22523(fe out, const fe z) {
    fe t0, t1, t2;
    int i;

    fe_sq(t0, z);
    fe_sq(t1, t0);
    fe_sq(t1, t1);
    fe_mul(t1, z, t1);
    fe_mul(t0, t0, t1);
    fe_sq(t0, t0);
    fe_mul(t0, t0, t1);
    fe_sq(t1, t0);
    for (i = 1; i < 5; ++i) {
        fe_sq(t1, t1);
    }
    fe_mul(t0, t1, t0);
    fe_sq(t1, t0);
    for (i = 1; i < 10; ++i) {
        fe_sq(t1, t1);
    }
    fe_mul(t1, t1, t0);
    fe_sq(t2, t1);
    for (i = 1; i < 20; ++i) {
        fe_sq(t2, t2);
    }
    fe_mul(t1, t2, t1);
    fe_sq(t1, t1);
    for (i = 1; i < 10; ++i) {
        fe_sq(t1, t1);
    }
    fe_mul(t0, t1, t0);
    fe_sq(t1, t0);
    for (i = 1; i < 50; ++i) {
        fe_sq(t1, t1);
    }
    fe_mul(t1, t1, t0);
    fe_sq(t2, t1);
    for (i = 1; i < 100; ++i) {
        fe_sq(t2, t2);
    }
    fe_mul(t1, t2, t1);
    fe_sq(t1, t1);
    for (i = 1; i < 50; ++i) {
        fe_sq(t1, t1);
    }
    fe_mul(t0, t1, t0);
    fe_sq(t0, t0);
    fe_sq(t0, t0);
    fe_mul(out, t0, z);
}

__device__ void fe_invert(fe out, const fe z) {
    fe t0, t1, t2, t3;
    int i;

    fe_sq(t0, z);
    fe_sq(t1, t0);
    fe_sq(t1, t1);
    fe_mul(t1, z, t1);
    fe_mul(t0, t0, t1);
    fe_sq(t2, t0);
    fe_mul(t1, t1, t2);
    fe_sq(t2, t1);
    for (i = 1; i < 5; ++i) {
        fe_sq(t2, t2);
    }
    fe_mul(t1, t2, t1);
    fe_sq(t2, t1);
    for (i = 1; i < 10; ++i) {
        fe_sq(t2, t2);
    }
    fe_mul(t2, t2, t1);
    fe_sq(t3, t2);
    for (i = 1; i < 20; ++i) {
        fe_sq(t3, t3);
    }
    fe_mul(t2, t3, t2);
    fe_sq(t2, t2);
    for (i = 1; i < 10; ++i) {
        fe_sq(t2, t2);
    }
    fe_mul(t1, t2, t1);
    fe_sq(t2, t1);
    for (i = 1; i < 50; ++i) {
        fe_sq(t2, t2);
    }
    fe_mul(t2, t2, t1);
    fe_sq(t3, t2);
    for (i = 1; i < 100; ++i) {
        fe_sq(t3, t3);
    }
    fe_mul(t2, t3, t2);
    fe_sq(t2, t2);
    for (i = 1; i < 50; ++i) {
        fe_sq(t2, t2);
    }
    fe_mul(t1, t2, t1);
    fe_sq(t1, t1);
    for (i = 1; i < 5; ++i) {
        fe_sq(t1, t1);
    }
    fe_mul(out, t1, t0);
}

__device__ void fe_tobytes(uint8_t s[32], const fe h) {
    int32_t t[10];
    fe_copy(t, h);

    int32_t q = (19 * t[9] + (1 << 24)) >> 25;
    q = (t[0] + q) >> 26;
    q = (t[1] + q) >> 25;
    q = (t[2] + q) >> 26;
    q = (t[3] + q) >> 25;
    q = (t[4] + q) >> 26;
    q = (t[5] + q) >> 25;
    q = (t[6] + q) >> 26;
    q = (t[7] + q) >> 25;
    q = (t[8] + q) >> 26;
    q = (t[9] + q) >> 25;

    t[0] += 19 * q;
    int32_t carry0 = t[0] >> 26;
    t[1] += carry0;
    t[0] -= carry0 << 26;
    int32_t carry1 = t[1] >> 25;
    t[2] += carry1;
    t[1] -= carry1 << 25;
    int32_t carry2 = t[2] >> 26;
    t[3] += carry2;
    t[2] -= carry2 << 26;
    int32_t carry3 = t[3] >> 25;
    t[4] += carry3;
    t[3] -= carry3 << 25;
    int32_t carry4 = t[4] >> 26;
    t[5] += carry4;
    t[4] -= carry4 << 26;
    int32_t carry5 = t[5] >> 25;
    t[6] += carry5;
    t[5] -= carry5 << 25;
    int32_t carry6 = t[6] >> 26;
    t[7] += carry6;
    t[6] -= carry6 << 26;
    int32_t carry7 = t[7] >> 25;
    t[8] += carry7;
    t[7] -= carry7 << 25;
    int32_t carry8 = t[8] >> 26;
    t[9] += carry8;
    t[8] -= carry8 << 26;
    int32_t carry9 = t[9] >> 25;
    t[9] -= carry9 << 25;

    s[0] = t[0] >> 0;
    s[1] = t[0] >> 8;
    s[2] = t[0] >> 16;
    s[3] = (t[0] >> 24) | (t[1] << 2);
    s[4] = t[1] >> 6;
    s[5] = t[1] >> 14;
    s[6] = (t[1] >> 22) | (t[2] << 3);
    s[7] = t[2] >> 5;
    s[8] = t[2] >> 13;
    s[9] = (t[2] >> 21) | (t[3] << 5);
    s[10] = t[3] >> 3;
    s[11] = t[3] >> 11;
    s[12] = (t[3] >> 19) | (t[4] << 6);
    s[13] = t[4] >> 2;
    s[14] = t[4] >> 10;
    s[15] = t[4] >> 18;
    s[16] = t[5] >> 0;
    s[17] = t[5] >> 8;
    s[18] = t[5] >> 16;
    s[19] = (t[5] >> 24) | (t[6] << 1);
    s[20] = t[6] >> 7;
    s[21] = t[6] >> 15;
    s[22] = (t[6] >> 23) | (t[7] << 3);
    s[23] = t[7] >> 5;
    s[24] = t[7] >> 13;
    s[25] = (t[7] >> 21) | (t[8] << 4);
    s[26] = t[8] >> 4;
    s[27] = t[8] >> 12;
    s[28] = (t[8] >> 20) | (t[9] << 6);
    s[29] = t[9] >> 2;
    s[30] = t[9] >> 10;
    s[31] = t[9] >> 18;
}

__device__ void fe_frombytes(fe h, const uint8_t s[32]) {
    int64_t h0 = load4(s);
    int64_t h1 = load3(s + 4) << 6;
    int64_t h2 = load3(s + 7) << 5;
    int64_t h3 = load3(s + 10) << 3;
    int64_t h4 = load3(s + 13) << 2;
    int64_t h5 = load4(s + 16);
    int64_t h6 = load3(s + 20) << 7;
    int64_t h7 = load3(s + 23) << 5;
    int64_t h8 = load3(s + 26) << 4;
    int64_t h9 = (load3(s + 29) & 0x7FFFFF) << 2;

    int64_t carry0;
    int64_t carry1;
    int64_t carry2;
    int64_t carry3;
    int64_t carry4;
    int64_t carry5;
    int64_t carry6;
    int64_t carry7;
    int64_t carry8;
    int64_t carry9;

    carry9 = (h9 + (int64_t)(1 << 24)) >> 25;
    h0 += carry9 * 19;
    h9 -= carry9 << 25;
    carry1 = (h1 + (int64_t)(1 << 24)) >> 25;
    h2 += carry1;
    h1 -= carry1 << 25;
    carry3 = (h3 + (int64_t)(1 << 24)) >> 25;
    h4 += carry3;
    h3 -= carry3 << 25;
    carry5 = (h5 + (int64_t)(1 << 24)) >> 25;
    h6 += carry5;
    h5 -= carry5 << 25;
    carry7 = (h7 + (int64_t)(1 << 24)) >> 25;
    h8 += carry7;
    h7 -= carry7 << 25;

    carry0 = (h0 + (int64_t)(1 << 25)) >> 26;
    h1 += carry0;
    h0 -= carry0 << 26;
    carry2 = (h2 + (int64_t)(1 << 25)) >> 26;
    h3 += carry2;
    h2 -= carry2 << 26;
    carry4 = (h4 + (int64_t)(1 << 25)) >> 26;
    h5 += carry4;
    h4 -= carry4 << 26;
    carry6 = (h6 + (int64_t)(1 << 25)) >> 26;
    h7 += carry6;
    h6 -= carry6 << 26;
    carry8 = (h8 + (int64_t)(1 << 25)) >> 26;
    h9 += carry8;
    h8 -= carry8 << 26;

    h[0] = (int32_t)h0;
    h[1] = (int32_t)h1;
    h[2] = (int32_t)h2;
    h[3] = (int32_t)h3;
    h[4] = (int32_t)h4;
    h[5] = (int32_t)h5;
    h[6] = (int32_t)h6;
    h[7] = (int32_t)h7;
    h[8] = (int32_t)h8;
    h[9] = (int32_t)h9;
}

__device__ int fe_isnonzero(const fe f) {
    uint8_t s[32];
    fe_tobytes(s, f);
    uint8_t acc = 0;
#pragma unroll
    for (int i = 0; i < 32; ++i) {
        acc |= s[i];
    }
    return acc != 0;
}

__device__ int fe_isnegative(const fe f) {
    uint8_t s[32];
    fe_tobytes(s, f);
    return s[0] & 1;
}

typedef struct {
    fe X;
    fe Y;
    fe Z;
    fe T;
} ge_p3;

__device__ void ge_identity(ge_p3* p) {
    fe_0(p->X);
    fe_1(p->Y);
    fe_1(p->Z);
    fe_0(p->T);
}

__device__ void ge_p3_identity(ge_p3* p) {
    ge_identity(p);
}

__device__ void ge_neg(ge_p3* out, const ge_p3* in) {
    fe_copy(out->X, in->X);
    fe_copy(out->Y, in->Y);
    fe_copy(out->Z, in->Z);
    fe_copy(out->T, in->T);
    fe_neg(out->X, out->X);
    fe_neg(out->T, out->T);
}

__device__ int ge_frombytes_negate_vartime(ge_p3* h, const uint8_t s[32]) {
    fe u, v, v3, vxx, check;

    fe_frombytes(h->Y, s);
    fe_1(h->Z);
    fe_sq(u, h->Y);
    fe_mul(v, u, FE_D);
    fe_sub(u, u, h->Z);
    fe_add(v, v, h->Z);
    fe_sq(v3, v);
    fe_mul(v3, v3, v);
    fe_sq(h->X, v3);
    fe_mul(h->X, h->X, v);
    fe_mul(h->X, h->X, u);
    fe_pow22523(h->X, h->X);
    fe_mul(h->X, h->X, v3);
    fe_mul(h->X, h->X, u);
    fe_sq(vxx, h->X);
    fe_mul(vxx, vxx, v);
    fe_sub(check, vxx, u);
    if (fe_isnonzero(check)) {
        fe_add(check, vxx, u);
        if (fe_isnonzero(check)) {
            return -1;
        }
        fe_mul(h->X, h->X, FE_SQRTM1);
    }
    if (fe_isnegative(h->X) == (s[31] >> 7)) {
        fe_neg(h->X, h->X);
    }
    fe_mul(h->T, h->X, h->Y);
    return 0;
}

__device__ void ge_add(ge_p3* r, const ge_p3* p, const ge_p3* q) {
    fe a, b, c, d, e, f, g, h;

    fe_sub(a, p->Y, p->X);
    fe_sub(b, q->Y, q->X);
    fe_mul(a, a, b);
    fe_add(b, p->Y, p->X);
    fe_add(c, q->Y, q->X);
    fe_mul(b, b, c);
    fe_mul(c, p->T, q->T);
    fe_mul(c, c, FE_D2);
    fe_mul(d, p->Z, q->Z);
    fe_add(d, d, d);
    fe_sub(e, b, a);
    fe_sub(f, d, c);
    fe_add(g, d, c);
    fe_add(h, b, a);
    fe_mul(r->X, e, f);
    fe_mul(r->Y, g, h);
    fe_mul(r->Z, f, g);
    fe_mul(r->T, e, h);
}

__device__ void ge_double(ge_p3* r, const ge_p3* p) {
    fe a, b, c, d, e, f, g, h;

    fe_sq(a, p->X);
    fe_sq(b, p->Y);
    fe_sq2(c, p->Z);
    fe_neg(d, a);
    fe_add(e, p->X, p->Y);
    fe_sq(e, e);
    fe_sub(e, e, a);
    fe_sub(e, e, b);
    fe_add(g, d, b);
    fe_sub(f, g, c);
    fe_sub(h, d, b);
    fe_mul(r->X, e, f);
    fe_mul(r->Y, g, h);
    fe_mul(r->Z, f, g);
    fe_mul(r->T, e, h);
}

__device__ void ge_tobytes(uint8_t s[32], const ge_p3* h) {
    fe recip, x, y;
    fe_invert(recip, h->Z);
    fe_mul(x, h->X, recip);
    fe_mul(y, h->Y, recip);
    fe_tobytes(s, y);
    s[31] ^= fe_isnegative(x) << 7;
}

__device__ __constant__ uint8_t L_ORDER[32] = {
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58,
    0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10};

__device__ __forceinline__ int scalar_bit(const uint8_t s[32], int bit) {
    return (s[bit >> 3] >> (bit & 7)) & 1;
}

__device__ bool sc_is_canonical(const uint8_t s[32]) {
    for (int i = 31; i >= 0; --i) {
        if (s[i] > L_ORDER[i]) {
            return false;
        }
        if (s[i] < L_ORDER[i]) {
            return true;
        }
    }
    return false;
}

__device__ void ge_double_scalarmult(ge_p3* r, const uint8_t a[32], const ge_p3* A, const uint8_t b[32]) {
    ge_p3 base;
    ge_p3_identity(r);
    if (ge_frombytes_negate_vartime(&base, BASE_POINT_BYTES) != 0) {
        return;
    }
    ge_neg(&base, &base);
    for (int bit = 255; bit >= 0; --bit) {
        ge_p3 tmp;
        ge_double(&tmp, r);
        fe_copy(r->X, tmp.X);
        fe_copy(r->Y, tmp.Y);
        fe_copy(r->Z, tmp.Z);
        fe_copy(r->T, tmp.T);
        if (scalar_bit(a, bit)) {
            ge_add(&tmp, r, A);
            fe_copy(r->X, tmp.X);
            fe_copy(r->Y, tmp.Y);
            fe_copy(r->Z, tmp.Z);
            fe_copy(r->T, tmp.T);
        }
        if (scalar_bit(b, bit)) {
            ge_add(&tmp, r, &base);
            fe_copy(r->X, tmp.X);
            fe_copy(r->Y, tmp.Y);
            fe_copy(r->Z, tmp.Z);
            fe_copy(r->T, tmp.T);
        }
    }
}

__device__ bool ed25519_verify_device(
    const uint8_t R[32],
    const uint8_t S[32],
    const uint8_t A[32],
    const uint8_t hram[32]
) {
    if (!sc_is_canonical(S)) {
        return false;
    }
    ge_p3 A_point;
    if (ge_frombytes_negate_vartime(&A_point, A) != 0) {
        return false;
    }
    ge_p3 R_point;
    if (ge_frombytes_negate_vartime(&R_point, R) != 0) {
        return false;
    }
    (void)R_point; // canonical check only

    ge_p3 check;
    ge_double_scalarmult(&check, hram, &A_point, S);
    uint8_t check_bytes[32];
    ge_tobytes(check_bytes, &check);
    uint8_t ok = 1;
#pragma unroll
    for (int i = 0; i < 32; ++i) {
        ok &= (check_bytes[i] == R[i]);
    }
    return ok != 0;
}

extern "C" __global__ void signature_kernel(
    const uint8_t* signatures,
    const uint8_t* public_keys,
    const uint8_t* hram_scalars,
    uint32_t count,
    uint8_t* results
) {
    uint32_t idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx >= count) {
        return;
    }
    const uint8_t* sig = signatures + idx * 64;
    const uint8_t* pk = public_keys + idx * 32;
    const uint8_t* hram = hram_scalars + idx * 32;
    const uint8_t* R = sig;
    const uint8_t* S = sig + 32;
    bool ok = ed25519_verify_device(R, S, pk, hram);
    results[idx] = ok ? 1u : 0u;
}
