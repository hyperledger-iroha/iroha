
            #include <metal_stdlib>
            using namespace metal;
            inline uint rotr(uint x, uint n) { return (x >> n) | (x << (32 - n)); }
            constant uint K[64] = {
                0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,
                0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
                0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,
                0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
                0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,
                0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
                0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,
                0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2
            };
            constant uint H0[8] = {
                0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,
                0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19
            };
            kernel void sha256_pairs_reduce(device const uchar* digests [[buffer(0)]],
                                            device uint* out_states [[buffer(1)]],
                                            uint id [[thread_position_in_grid]]) {
                const device uchar* left = digests + (id * 64);
                const device uchar* right = left + 32;
                uint w[64];
                for (uint t = 0; t < 16; ++t) {
                    const device uchar* p = (t < 8) ? (left + t * 4) : (right + (t - 8) * 4);
                    w[t] = (uint(p[0]) << 24) | (uint(p[1]) << 16) | (uint(p[2]) << 8) | uint(p[3]);
                }
                for (uint t = 16; t < 64; ++t) {
                    uint s0 = rotr(w[t-15],7) ^ rotr(w[t-15],18) ^ (w[t-15] >> 3);
                    uint s1 = rotr(w[t-2],17) ^ rotr(w[t-2],19) ^ (w[t-2] >> 10);
                    w[t] = w[t-16] + s0 + w[t-7] + s1;
                }
                uint a=H0[0], b=H0[1], c=H0[2], d=H0[3], e=H0[4], f=H0[5], g=H0[6], h=H0[7];
                for (uint t = 0; t < 64; ++t) {
                    uint s1 = rotr(e,6) ^ rotr(e,11) ^ rotr(e,25);
                    uint ch = (e & f) ^ ((~e) & g);
                    uint temp1 = h + s1 + ch + K[t] + w[t];
                    uint s0 = rotr(a,2) ^ rotr(a,13) ^ rotr(a,22);
                    uint maj = (a & b) ^ (a & c) ^ (b & c);
                    uint temp2 = s0 + maj;
                    h = g; g = f; f = e; e = d + temp1; d = c; c = b; b = a; a = temp1 + temp2;
                }
                uint st0 = H0[0] + a;
                uint st1 = H0[1] + b;
                uint st2 = H0[2] + c;
                uint st3 = H0[3] + d;
                uint st4 = H0[4] + e;
                uint st5 = H0[5] + f;
                uint st6 = H0[6] + g;
                uint st7 = H0[7] + h;

                uint wp[64];
                wp[0] = 0x80000000u;
                for (uint i = 1; i < 15; ++i) {
                    wp[i] = 0;
                }
                wp[15] = 512u;
                for (uint t = 16; t < 64; ++t) {
                    uint s0 = rotr(wp[t-15],7) ^ rotr(wp[t-15],18) ^ (wp[t-15] >> 3);
                    uint s1 = rotr(wp[t-2],17) ^ rotr(wp[t-2],19) ^ (wp[t-2] >> 10);
                    wp[t] = wp[t-16] + s0 + wp[t-7] + s1;
                }
                a = st0; b = st1; c = st2; d = st3; e = st4; f = st5; g = st6; h = st7;
                for (uint t = 0; t < 64; ++t) {
                    uint s1 = rotr(e,6) ^ rotr(e,11) ^ rotr(e,25);
                    uint ch = (e & f) ^ ((~e) & g);
                    uint temp1 = h + s1 + ch + K[t] + wp[t];
                    uint s0 = rotr(a,2) ^ rotr(a,13) ^ rotr(a,22);
                    uint maj = (a & b) ^ (a & c) ^ (b & c);
                    uint temp2 = s0 + maj;
                    h = g; g = f; f = e; e = d + temp1; d = c; c = b; b = a; a = temp1 + temp2;
                }
                uint out0 = st0 + a;
                uint out1 = st1 + b;
                uint out2 = st2 + c;
                uint out3 = st3 + d;
                uint out4 = st4 + e;
                uint out5 = st5 + f;
                uint out6 = st6 + g;
                uint out7 = st7 + h;
                device uint* dst = out_states + (id * 8);
                dst[0] = out0; dst[1] = out1; dst[2] = out2; dst[3] = out3;
                dst[4] = out4; dst[5] = out5; dst[6] = out6; dst[7] = out7;
            }
        