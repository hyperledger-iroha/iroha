
            #include <metal_stdlib>
            using namespace metal;
            inline ulong rotl64(ulong x, uint n) { return (x << n) | (x >> (64 - n)); }
            constant ulong RC[24] = {
                0x0000000000000001ull, 0x0000000000008082ull, 0x800000000000808aull, 0x8000000080008000ull,
                0x000000000000808bull, 0x0000000080000001ull, 0x8000000080008081ull, 0x8000000000008009ull,
                0x000000000000008aull, 0x0000000000000088ull, 0x0000000080008009ull, 0x000000008000000aull,
                0x000000008000808bull, 0x800000000000008bull, 0x8000000000008089ull, 0x8000000000008003ull,
                0x8000000000008002ull, 0x8000000000000080ull, 0x000000000000800aull, 0x800000008000000aull,
                0x8000000080008081ull, 0x8000000000008080ull, 0x0000000080000001ull, 0x8000000080008008ull
            };
            constant ushort ROT[5][5] = {
                {0, 36, 3, 41, 18},
                {1, 44, 10, 45, 2},
                {62, 6, 43, 15, 61},
                {28, 55, 25, 21, 56},
                {27, 20, 39, 8, 14}
            };
            kernel void keccak_f1600(device ulong* state [[buffer(0)]], uint tid [[thread_position_in_grid]]) {
                if (tid != 0) { return; }
                ulong a[25];
                for (uint i = 0; i < 25; ++i) { a[i] = state[i]; }
                for (uint round = 0; round < 24; ++round) {
                    ulong c[5];
                    for (uint x = 0; x < 5; ++x) {
                        c[x] = a[x] ^ a[x + 5] ^ a[x + 10] ^ a[x + 15] ^ a[x + 20];
                    }
                    ulong d[5];
                    for (uint x = 0; x < 5; ++x) {
                        d[x] = c[(x + 4) % 5] ^ rotl64(c[(x + 1) % 5], 1);
                    }
                    for (uint x = 0; x < 5; ++x) {
                        for (uint y = 0; y < 5; ++y) {
                            a[x + 5 * y] ^= d[x];
                        }
                    }
                    ulong b[25];
                    for (uint x = 0; x < 5; ++x) {
                        for (uint y = 0; y < 5; ++y) {
                            uint rot = ROT[x][y];
                            ulong val = a[x + 5 * y];
                            ulong rotated = rot ? rotl64(val, rot) : val;
                            uint new_x = y;
                            uint new_y = (2 * x + 3 * y) % 5;
                            b[new_x + 5 * new_y] = rotated;
                        }
                    }
                    for (uint y = 0; y < 5; ++y) {
                        for (uint x = 0; x < 5; ++x) {
                            ulong current = b[x + 5 * y];
                            ulong next1 = b[((x + 1) % 5) + 5 * y];
                            ulong next2 = b[((x + 2) % 5) + 5 * y];
                            a[x + 5 * y] = current ^ ((~next1) & next2);
                        }
                    }
                    a[0] ^= RC[round];
                }
                for (uint i = 0; i < 25; ++i) {
                    state[i] = a[i];
                }
            }
        