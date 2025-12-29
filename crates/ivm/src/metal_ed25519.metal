#include <metal_stdlib>
using namespace metal;

typedef int fe[10];

inline long load3(const thread uchar* in) {
    return in[0] | ((long)in[1] << 8) | ((long)in[2] << 16);
}

inline long load4(const thread uchar* in) {
    return in[0] | ((long)in[1] << 8) | ((long)in[2] << 16) | ((long)in[3] << 24);
}

constant int FE_D[10] = {
    -10913610, 13857413, -15372611, 6949391,   114729,
    -8787816,  -6275908, -3247719,  -18696448, -12055116};

constant int FE_D2[10] = {
    -21827220, 27714826, -30745222, 13898782,   229458,
    -17575632, -12551816, -6495438, -37392896, -24110232};

constant int FE_SQRTM1[10] = {
    -32595792, -7943725, 9377950, 3500415, 12389472,
    -272473,   -25146209, -2005654, 326686,  11406482};

constant uchar BASE_POINT_BYTES[32] = {
    0x58, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66,
    0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66, 0x66};

inline void fe_0(fe h) {
    for (int i = 0; i < 10; ++i) {
        h[i] = 0;
    }
}

inline void fe_1(fe h) {
    fe_0(h);
    h[0] = 1;
}

inline void fe_copy(fe h, const fe f) {
    for (int i = 0; i < 10; ++i) {
        h[i] = f[i];
    }
}

inline void fe_copy_const(fe h, constant int f[10]) {
    for (int i = 0; i < 10; ++i) {
        h[i] = f[i];
    }
}

inline void fe_add(fe h, const fe f, const fe g) {
    for (int i = 0; i < 10; ++i) {
        h[i] = f[i] + g[i];
    }
}

inline void fe_sub(fe h, const fe f, const fe g) {
    for (int i = 0; i < 10; ++i) {
        h[i] = f[i] - g[i];
    }
}

inline void fe_neg(fe h, const fe f) {
    for (int i = 0; i < 10; ++i) {
        h[i] = -f[i];
    }
}

inline void fe_cmov(fe f, const fe g, int b) {
    int mask = -b;
    for (int i = 0; i < 10; ++i) {
        int t = f[i] ^ g[i];
        t &= mask;
        f[i] ^= t;
    }
}

inline void fe_mul(fe h, const fe f, const fe g) {
    int f0 = f[0];
    int f1 = f[1];
    int f2 = f[2];
    int f3 = f[3];
    int f4 = f[4];
    int f5 = f[5];
    int f6 = f[6];
    int f7 = f[7];
    int f8 = f[8];
    int f9 = f[9];
    int g0 = g[0];
    int g1 = g[1];
    int g2 = g[2];
    int g3 = g[3];
    int g4 = g[4];
    int g5 = g[5];
    int g6 = g[6];
    int g7 = g[7];
    int g8 = g[8];
    int g9 = g[9];
    int g1_19 = 19 * g1;
    int g2_19 = 19 * g2;
    int g3_19 = 19 * g3;
    int g4_19 = 19 * g4;
    int g5_19 = 19 * g5;
    int g6_19 = 19 * g6;
    int g7_19 = 19 * g7;
    int g8_19 = 19 * g8;
    int g9_19 = 19 * g9;
    int f1_2 = 2 * f1;
    int f3_2 = 2 * f3;
    int f5_2 = 2 * f5;
    int f7_2 = 2 * f7;
    int f9_2 = 2 * f9;

    long h0 = (long)f0 * g0 + (long)f1_2 * g9_19 + (long)f2 * g8_19 +
                (long)f3_2 * g7_19 + (long)f4 * g6_19 + (long)f5_2 * g5_19 +
                (long)f6 * g4_19 + (long)f7_2 * g3_19 + (long)f8 * g2_19 +
                (long)f9_2 * g1_19;
    long h1 = (long)f0 * g1 + (long)f1 * g0 + (long)f2 * g9_19 +
                (long)f3 * g8_19 + (long)f4 * g7_19 + (long)f5 * g6_19 +
                (long)f6 * g5_19 + (long)f7 * g4_19 + (long)f8 * g3_19 +
                (long)f9 * g2_19;
    long h2 = (long)f0 * g2 + (long)f1_2 * g1 + (long)f2 * g0 +
                (long)f3_2 * g9_19 + (long)f4 * g8_19 + (long)f5_2 * g7_19 +
                (long)f6 * g6_19 + (long)f7_2 * g5_19 + (long)f8 * g4_19 +
                (long)f9_2 * g3_19;
    long h3 = (long)f0 * g3 + (long)f1 * g2 + (long)f2 * g1 + (long)f3 * g0 +
                (long)f4 * g9_19 + (long)f5 * g8_19 + (long)f6 * g7_19 +
                (long)f7 * g6_19 + (long)f8 * g5_19 + (long)f9 * g4_19;
    long h4 = (long)f0 * g4 + (long)f1_2 * g3 + (long)f2 * g2 +
                (long)f3_2 * g1 + (long)f4 * g0 + (long)f5_2 * g9_19 +
                (long)f6 * g8_19 + (long)f7_2 * g7_19 + (long)f8 * g6_19 +
                (long)f9_2 * g5_19;
    long h5 = (long)f0 * g5 + (long)f1 * g4 + (long)f2 * g3 + (long)f3 * g2 +
                (long)f4 * g1 + (long)f5 * g0 + (long)f6 * g9_19 +
                (long)f7 * g8_19 + (long)f8 * g7_19 + (long)f9 * g6_19;
    long h6 = (long)f0 * g6 + (long)f1_2 * g5 + (long)f2 * g4 +
                (long)f3_2 * g3 + (long)f4 * g2 + (long)f5_2 * g1 +
                (long)f6 * g0 + (long)f7_2 * g9_19 + (long)f8 * g8_19 +
                (long)f9_2 * g7_19;
    long h7 = (long)f0 * g7 + (long)f1 * g6 + (long)f2 * g5 + (long)f3 * g4 +
                (long)f4 * g3 + (long)f5 * g2 + (long)f6 * g1 + (long)f7 * g0 +
                (long)f8 * g9_19 + (long)f9 * g8_19;
    long h8 = (long)f0 * g8 + (long)f1_2 * g7 + (long)f2 * g6 +
                (long)f3_2 * g5 + (long)f4 * g4 + (long)f5_2 * g3 +
                (long)f6 * g2 + (long)f7_2 * g1 + (long)f8 * g0 +
                (long)f9_2 * g9_19;
    long h9 = (long)f0 * g9 + (long)f1 * g8 + (long)f2 * g7 + (long)f3 * g6 +
                (long)f4 * g5 + (long)f5 * g4 + (long)f6 * g3 + (long)f7 * g2 +
                (long)f8 * g1 + (long)f9 * g0;

    long carry0 = (h0 + (long)(1 << 25)) >> 26;
    h1 += carry0;
    h0 -= carry0 << 26;
    long carry4 = (h4 + (long)(1 << 25)) >> 26;
    h5 += carry4;
    h4 -= carry4 << 26;
    long carry1 = (h1 + (long)(1 << 24)) >> 25;
    h2 += carry1;
    h1 -= carry1 << 25;
    long carry5 = (h5 + (long)(1 << 24)) >> 25;
    h6 += carry5;
    h5 -= carry5 << 25;
    long carry2 = (h2 + (long)(1 << 25)) >> 26;
    h3 += carry2;
    h2 -= carry2 << 26;
    long carry6 = (h6 + (long)(1 << 25)) >> 26;
    h7 += carry6;
    h6 -= carry6 << 26;
    long carry3 = (h3 + (long)(1 << 24)) >> 25;
    h4 += carry3;
    h3 -= carry3 << 25;
    long carry7 = (h7 + (long)(1 << 24)) >> 25;
    h8 += carry7;
    h7 -= carry7 << 25;
    long carry8 = (h8 + (long)(1 << 25)) >> 26;
    h9 += carry8;
    h8 -= carry8 << 26;
    long carry9 = (h9 + (long)(1 << 24)) >> 25;
    h0 += carry9 * 19;
    h9 -= carry9 << 25;
    carry0 = (h0 + (long)(1 << 25)) >> 26;
    h1 += carry0;
    h0 -= carry0 << 26;
    carry1 = (h1 + (long)(1 << 24)) >> 25;
    h2 += carry1;
    h1 -= carry1 << 25;

    h[0] = (int)h0;
    h[1] = (int)h1;
    h[2] = (int)h2;
    h[3] = (int)h3;
    h[4] = (int)h4;
    h[5] = (int)h5;
    h[6] = (int)h6;
    h[7] = (int)h7;
    h[8] = (int)h8;
    h[9] = (int)h9;
}

inline void fe_sq(fe h, const fe f) {
    fe_mul(h, f, f);
}

inline void fe_sq2(fe h, const fe f) {
    fe t;
    fe_sq(t, f);
    fe_add(h, t, t);
}

inline void fe_mul121666(fe h, const fe f) {
    long t[10];
    long carry;
    for (int i = 0; i < 10; ++i) {
        t[i] = (long)f[i] * 121666;
    }
    carry = (t[0] + (long)(1 << 25)) >> 26;
    t[1] += carry;
    t[0] -= carry << 26;
    carry = (t[4] + (long)(1 << 25)) >> 26;
    t[5] += carry;
    t[4] -= carry << 26;
    carry = (t[1] + (long)(1 << 24)) >> 25;
    t[2] += carry;
    t[1] -= carry << 25;
    carry = (t[5] + (long)(1 << 24)) >> 25;
    t[6] += carry;
    t[5] -= carry << 25;
    carry = (t[2] + (long)(1 << 25)) >> 26;
    t[3] += carry;
    t[2] -= carry << 26;
    carry = (t[6] + (long)(1 << 25)) >> 26;
    t[7] += carry;
    t[6] -= carry << 26;
    carry = (t[3] + (long)(1 << 24)) >> 25;
    t[4] += carry;
    t[3] -= carry << 25;
    carry = (t[7] + (long)(1 << 24)) >> 25;
    t[8] += carry;
    t[7] -= carry << 25;
    carry = (t[8] + (long)(1 << 25)) >> 26;
    t[9] += carry;
    t[8] -= carry << 26;
    carry = (t[9] + (long)(1 << 24)) >> 25;
    t[0] += carry * 19;
    t[9] -= carry << 25;
    carry = (t[0] + (long)(1 << 25)) >> 26;
    t[1] += carry;
    t[0] -= carry << 26;
    for (int i = 0; i < 10; ++i) {
        h[i] = (int)t[i];
    }
}

inline void fe_pow22523(fe out, const fe z) {
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
    fe_mul(out, t0, t1);
}

inline void fe_invert(fe out, const fe z) {
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

inline void fe_tobytes(uchar s[32], const fe h) {
    int t[10];
    fe_copy(t, h);

    int q = (19 * t[9] + (1 << 24)) >> 25;
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
    int carry0 = t[0] >> 26;
    t[1] += carry0;
    t[0] -= carry0 << 26;
    int carry1 = t[1] >> 25;
    t[2] += carry1;
    t[1] -= carry1 << 25;
    int carry2 = t[2] >> 26;
    t[3] += carry2;
    t[2] -= carry2 << 26;
    int carry3 = t[3] >> 25;
    t[4] += carry3;
    t[3] -= carry3 << 25;
    int carry4 = t[4] >> 26;
    t[5] += carry4;
    t[4] -= carry4 << 26;
    int carry5 = t[5] >> 25;
    t[6] += carry5;
    t[5] -= carry5 << 25;
    int carry6 = t[6] >> 26;
    t[7] += carry6;
    t[6] -= carry6 << 26;
    int carry7 = t[7] >> 25;
    t[8] += carry7;
    t[7] -= carry7 << 25;
    int carry8 = t[8] >> 26;
    t[9] += carry8;
    t[8] -= carry8 << 26;
    int carry9 = t[9] >> 25;
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

inline void fe_frombytes(fe h, const uchar s[32]) {
    long h0 = load4(s);
    long h1 = load3(s + 4) << 6;
    long h2 = load3(s + 7) << 5;
    long h3 = load3(s + 10) << 3;
    long h4 = load3(s + 13) << 2;
    long h5 = load4(s + 16);
    long h6 = load3(s + 20) << 7;
    long h7 = load3(s + 23) << 5;
    long h8 = load3(s + 26) << 4;
    long h9 = (load3(s + 29) & 0x7FFFFF) << 2;

    long carry0 = (h0 + (long)(1 << 25)) >> 26;
    h1 += carry0;
    h0 -= carry0 << 26;
    long carry1 = (h1 + (long)(1 << 24)) >> 25;
    h2 += carry1;
    h1 -= carry1 << 25;
    long carry2 = (h2 + (long)(1 << 25)) >> 26;
    h3 += carry2;
    h2 -= carry2 << 26;
    long carry3 = (h3 + (long)(1 << 24)) >> 25;
    h4 += carry3;
    h3 -= carry3 << 25;
    long carry4 = (h4 + (long)(1 << 25)) >> 26;
    h5 += carry4;
    h4 -= carry4 << 26;
    long carry5 = (h5 + (long)(1 << 24)) >> 25;
    h6 += carry5;
    h5 -= carry5 << 25;
    long carry6 = (h6 + (long)(1 << 25)) >> 26;
    h7 += carry6;
    h6 -= carry6 << 26;
    long carry7 = (h7 + (long)(1 << 24)) >> 25;
    h8 += carry7;
    h7 -= carry7 << 25;
    long carry8 = (h8 + (long)(1 << 25)) >> 26;
    h9 += carry8;
    h8 -= carry8 << 26;
    long carry9 = (h9 + (long)(1 << 24)) >> 25;
    h9 -= carry9 << 25;
    h0 += carry9 * 19;
    carry0 = (h0 + (long)(1 << 25)) >> 26;
    h1 += carry0;
    h0 -= carry0 << 26;

    h[0] = (int)h0;
    h[1] = (int)h1;
    h[2] = (int)h2;
    h[3] = (int)h3;
    h[4] = (int)h4;
    h[5] = (int)h5;
    h[6] = (int)h6;
    h[7] = (int)h7;
    h[8] = (int)h8;
    h[9] = (int)h9;
}

inline int fe_isnonzero(const fe f) {
    uchar s[32];
    fe_tobytes(s, f);
    uchar acc = 0;
    for (int i = 0; i < 32; ++i) {
        acc |= s[i];
    }
    return acc != 0;
}

inline int fe_isnegative(const fe f) {
    uchar s[32];
    fe_tobytes(s, f);
    return s[0] & 1;
}

typedef struct {
    fe X;
    fe Y;
    fe Z;
    fe T;
} ge_p3;

inline void ge_identity(thread ge_p3* p) {
    fe_0(p->X);
    fe_1(p->Y);
    fe_1(p->Z);
    fe_0(p->T);
}

inline void ge_p3_identity(thread ge_p3* p) {
    ge_identity(p);
}

inline void ge_neg(thread ge_p3* out, const thread ge_p3* in) {
    fe_copy(out->X, in->X);
    fe_copy(out->Y, in->Y);
    fe_copy(out->Z, in->Z);
    fe_copy(out->T, in->T);
    fe_neg(out->X, out->X);
    fe_neg(out->T, out->T);
}

inline int ge_frombytes_negate_vartime(thread ge_p3* h, const thread uchar s[32]) {
    fe u, v, v3, vxx, check;
    fe fe_d;
    fe fe_sqrtm1;
    fe_copy_const(fe_d, FE_D);
    fe_copy_const(fe_sqrtm1, FE_SQRTM1);

    fe_frombytes(h->Y, s);
    fe_1(h->Z);
    fe_sq(u, h->Y);
    fe_mul(v, u, fe_d);
    fe_sub(u, u, h->Z);
    fe_add(v, v, h->Z);
    fe_sq(v3, v);
    fe_mul(v3, v3, v);
    fe_mul(v3, v3, u);
    fe_pow22523(h->X, v3);
    fe_mul(h->X, h->X, u);
    fe_mul(h->X, h->X, v);
    fe_sq(vxx, h->X);
    fe_mul(vxx, vxx, v);
    fe_sub(check, vxx, u);
    if (fe_isnonzero(check)) {
        fe_add(check, vxx, u);
        if (fe_isnonzero(check)) {
            return -1;
        }
        fe_mul(h->X, h->X, fe_sqrtm1);
    }
    if (fe_isnegative(h->X) == (s[31] >> 7)) {
        fe_neg(h->X, h->X);
    }
    fe_mul(h->T, h->X, h->Y);
    return 0;
}

inline void ge_add(thread ge_p3* r, const thread ge_p3* p, const thread ge_p3* q) {
    fe a, b, c, d, e, f, g, h;
    fe fe_d2;
    fe_copy_const(fe_d2, FE_D2);

    fe_sub(a, p->Y, p->X);
    fe_sub(b, q->Y, q->X);
    fe_mul(a, a, b);
    fe_add(b, p->Y, p->X);
    fe_add(c, q->Y, q->X);
    fe_mul(b, b, c);
    fe_mul(c, p->T, q->T);
    fe_mul(c, c, fe_d2);
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

inline void ge_double(thread ge_p3* r, const thread ge_p3* p) {
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

inline void ge_tobytes(thread uchar s[32], const thread ge_p3* h) {
    fe recip, x, y;
    fe_invert(recip, h->Z);
    fe_mul(x, h->X, recip);
    fe_mul(y, h->Y, recip);
    fe_tobytes(s, y);
    s[31] ^= fe_isnegative(x) << 7;
}

constant uchar L_ORDER[32] = {
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58,
    0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10};

inline int scalar_bit(const thread uchar s[32], int bit) {
    return (s[bit >> 3] >> (bit & 7)) & 1;
}

inline bool sc_is_canonical(const thread uchar s[32]) {
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

inline void ge_double_scalarmult(
    thread ge_p3* r,
    const thread uchar a[32],
    const thread ge_p3* A,
    const thread uchar b[32]
) {
    ge_p3 base;
    ge_p3_identity(r);
    uchar base_bytes[32];
    for (int i = 0; i < 32; ++i) {
        base_bytes[i] = BASE_POINT_BYTES[i];
    }
    if (ge_frombytes_negate_vartime(&base, base_bytes) != 0) {
        return;
    }
    ge_p3 negA;
    ge_neg(&negA, A);

    for (int bit = 255; bit >= 0; --bit) {
        ge_p3 tmp;
        ge_double(&tmp, r);
        fe_copy(r->X, tmp.X);
        fe_copy(r->Y, tmp.Y);
        fe_copy(r->Z, tmp.Z);
        fe_copy(r->T, tmp.T);
        if (scalar_bit(a, bit)) {
            ge_add(&tmp, r, &negA);
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

inline bool ed25519_verify_device(
    const thread uchar R[32],
    const thread uchar S[32],
    const thread uchar A[32],
    const thread uchar hram[32]
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
    uchar check_bytes[32];
    ge_tobytes(check_bytes, &check);
    uchar ok = 1;
    for (int i = 0; i < 32; ++i) {
        ok &= (check_bytes[i] == R[i]);
    }
    return ok != 0;
}

kernel void signature_kernel(
    device const uchar* signatures [[buffer(0)]],
    device const uchar* public_keys [[buffer(1)]],
    device const uchar* hram_scalars [[buffer(2)]],
    constant uint& count [[buffer(3)]],
    device uchar* results [[buffer(4)]],
    uint idx [[thread_position_in_grid]]
) {
    if (idx >= count) {
        return;
    }
    const device uchar* sig = signatures + idx * 64;
    const device uchar* pk = public_keys + idx * 32;
    const device uchar* hram = hram_scalars + idx * 32;
    uchar R[32];
    uchar S[32];
    uchar A[32];
    uchar h_local[32];
    for (uint i = 0; i < 32; ++i) {
        R[i] = sig[i];
        S[i] = sig[32 + i];
        A[i] = pk[i];
        h_local[i] = hram[i];
    }
    bool ok = ed25519_verify_device(R, S, A, h_local);
    results[idx] = ok ? 1u : 0u;
}
