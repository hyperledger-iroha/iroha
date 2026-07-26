#include <stdint.h>

__device__ __forceinline__ uint64_t rotl64(uint64_t x, unsigned int n) {
    return (x << n) | (x >> (64 - n));
}

extern "C" __global__ void keccak_f1600_cuda(uint64_t* state) {
    unsigned int idx = blockIdx.x * blockDim.x + threadIdx.x;
    if (idx != 0) {
        return;
    }

    uint64_t a[25];
    for (int i = 0; i < 25; ++i) {
        a[i] = state[i];
    }

    const uint64_t RC[24] = {
        0x0000000000000001ULL,
        0x0000000000008082ULL,
        0x800000000000808aULL,
        0x8000000080008000ULL,
        0x000000000000808bULL,
        0x0000000080000001ULL,
        0x8000000080008081ULL,
        0x8000000000008009ULL,
        0x000000000000008aULL,
        0x0000000000000088ULL,
        0x0000000080008009ULL,
        0x000000008000000aULL,
        0x000000008000808bULL,
        0x800000000000008bULL,
        0x8000000000008089ULL,
        0x8000000000008003ULL,
        0x8000000000008002ULL,
        0x8000000000000080ULL,
        0x000000000000800aULL,
        0x800000008000000aULL,
        0x8000000080008081ULL,
        0x8000000000008080ULL,
        0x0000000080000001ULL,
        0x8000000080008008ULL,
    };

    const unsigned int R[5][5] = {
        {0, 36, 3, 41, 18},
        {1, 44, 10, 45, 2},
        {62, 6, 43, 15, 61},
        {28, 55, 25, 21, 56},
        {27, 20, 39, 8, 14},
    };

    for (int round = 0; round < 24; ++round) {
        uint64_t c[5];
        for (int x = 0; x < 5; ++x) {
            c[x] = a[x] ^ a[x + 5] ^ a[x + 10] ^ a[x + 15] ^ a[x + 20];
        }

        uint64_t d[5];
        for (int x = 0; x < 5; ++x) {
            d[x] = c[(x + 4) % 5] ^ rotl64(c[(x + 1) % 5], 1);
        }

        for (int x = 0; x < 5; ++x) {
            for (int y = 0; y < 5; ++y) {
                a[x + 5 * y] ^= d[x];
            }
        }

        uint64_t b[25];
        for (int x = 0; x < 5; ++x) {
            for (int y = 0; y < 5; ++y) {
                unsigned int rot = R[x][y];
                int new_x = y;
                int new_y = (2 * x + 3 * y) % 5;
                uint64_t val = a[x + 5 * y];
                b[new_x + 5 * new_y] = rot ? rotl64(val, rot) : val;
            }
        }

        for (int y = 0; y < 5; ++y) {
            for (int x = 0; x < 5; ++x) {
                uint64_t current = b[x + 5 * y];
                uint64_t next1 = b[((x + 1) % 5) + 5 * y];
                uint64_t next2 = b[((x + 2) % 5) + 5 * y];
                a[x + 5 * y] = current ^ ((~next1) & next2);
            }
        }

        a[0] ^= RC[round];
    }

    for (int i = 0; i < 25; ++i) {
        state[i] = a[i];
    }
}
