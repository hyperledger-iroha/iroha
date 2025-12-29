#include <stdint.h>

__device__ __forceinline__ unsigned int global_idx() {
    return blockIdx.x * blockDim.x + threadIdx.x;
}

extern "C" __global__ void bitonic_step(
    uint64_t* hi,
    uint64_t* lo,
    unsigned int n,
    unsigned int j,
    unsigned int k
) {
    unsigned int idx = global_idx();
    unsigned int ixj = idx ^ j;
    if (idx >= n || ixj >= n || ixj <= idx) {
        return;
    }

    uint64_t hi_i = hi[idx];
    uint64_t lo_i = lo[idx];
    uint64_t hi_j = hi[ixj];
    uint64_t lo_j = lo[ixj];

    bool ascending = ((idx & k) == 0);
    bool greater =
        (hi_i > hi_j) || (hi_i == hi_j && lo_i > lo_j);
    bool less =
        (hi_i < hi_j) || (hi_i == hi_j && lo_i < lo_j);

    bool swap = ascending ? greater : less;
    if (swap) {
        hi[idx] = hi_j;
        lo[idx] = lo_j;
        hi[ixj] = hi_i;
        lo[ixj] = lo_i;
    }
}
