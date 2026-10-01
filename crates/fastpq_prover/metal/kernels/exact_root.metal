// Bounded exact-root Goldilocks FFT with independent threadgroups per stage.
#include <metal_stdlib>
using namespace metal;
#include "field.metal"

struct ExactRootFftArgs {
    ulong column_len;
    ulong normalization;
    uint log_len;
    uint column_count;
    uint stage;
    uint padding;
};

// Public radix-256 exponent digits index a root-specific 8KiB table. Exactly
// three multiplies replace repeated square-and-multiply exponentiation. Root,
// exponent, table addresses and execution order are independent of column data.
inline ulong exact_root_power_v1(const device ulong *powers, uint exponent) {
    ulong value = powers[exponent & 255U];
    value = mul_mod(value, powers[256U + ((exponent >> 8U) & 255U)]);
    value = mul_mod(value, powers[512U + ((exponent >> 16U) & 255U)]);
    return mul_mod(value, powers[768U + (exponent >> 24U)]);
}

// One pair is written only by its lower index. Distinct pairs never overlap.
kernel void exact_root_bit_reverse_v1(
    device ulong *columns [[buffer(0)]],
    constant ExactRootFftArgs &args [[buffer(2)]],
    uint2 position [[thread_position_in_grid]]
) {
    ulong index = (ulong)position.x;
    if (index >= args.column_len || position.y >= args.column_count) return;
    ulong reversed = 0UL;
    ulong remaining = index;
    for (uint bit = 0U; bit < args.log_len; ++bit) {
        reversed = (reversed << 1U) | (remaining & 1UL);
        remaining >>= 1U;
    }
    if (index < reversed) {
        ulong base = (ulong)position.y * args.column_len;
        ulong word = columns[base + index];
        columns[base + index] = columns[base + reversed];
        columns[base + reversed] = word;
    }
}

// Each group owns one disjoint tile. No group iterates across the column.
kernel void exact_root_local_tiles_v1(
    device ulong *columns [[buffer(0)]],
    const device ulong *root_powers [[buffer(1)]],
    constant ExactRootFftArgs &args [[buffer(2)]],
    uint2 group [[threadgroup_position_in_grid]],
    uint lane [[thread_index_in_threadgroup]]
) {
    threadgroup ulong tile[256];
    uint local_log = min(args.log_len, 8U);
    uint tile_len = 1U << local_log;
    ulong start = (ulong)group.y * args.column_len + (ulong)group.x * tile_len;
    if (lane < tile_len) tile[lane] = columns[start + lane];
    threadgroup_barrier(mem_flags::mem_threadgroup);
    for (uint stage = 0U; stage < local_log; ++stage) {
        if (lane < tile_len / 2U) {
            uint half_width = 1U << stage;
            uint offset = lane & (half_width - 1U);
            uint low = (lane - offset) * 2U + offset;
            // log_len <= 32 and stage < log_len: shift is in 0..31;
            // offset < 2^stage keeps the product below 2^31.
            uint exponent = offset << (args.log_len - stage - 1U);
            ulong twiddle = exact_root_power_v1(root_powers, exponent);
            ulong left = tile[low];
            ulong right = mul_mod(tile[low + half_width], twiddle);
            tile[low] = add_mod(left, right);
            tile[low + half_width] = sub_mod(left, right);
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
    if (lane < tile_len) {
        ulong value = tile[lane];
        if (local_log == args.log_len) value = mul_mod(value, args.normalization);
        columns[start + lane] = value;
    }
}

// One group owns 2048 consecutive butterflies, 8 per lane. A host resource
// barrier separates every invocation at different stages; a threadgroup
// barrier alone cannot synchronize these independently scheduled groups.
kernel void exact_root_global_stage_v1(
    device ulong *columns [[buffer(0)]],
    const device ulong *root_powers [[buffer(1)]],
    constant ExactRootFftArgs &args [[buffer(2)]],
    uint2 group [[threadgroup_position_in_grid]],
    uint lane [[thread_index_in_threadgroup]]
) {
    ulong half_width = 1UL << args.stage;
    ulong group_base = (ulong)group.x * 2048UL;
    ulong butterfly = group_base + (ulong)lane;
    ulong limit = args.column_len / 2UL;
    ulong offset = butterfly & (half_width - 1UL);
    uint root_shift = args.log_len - args.stage - 1U;
    ulong first_twiddle = exact_root_power_v1(root_powers, (uint)offset << root_shift);
    ulong twiddle = first_twiddle;
    ulong twiddle_stride = exact_root_power_v1(root_powers, 256U << root_shift);
    ulong column_base = (ulong)group.y * args.column_len;
    for (uint iteration = 0U; iteration < 8U && butterfly < limit; ++iteration) {
        ulong low = column_base + (butterfly - offset) * 2UL + offset;
        ulong left = columns[low];
        ulong right = mul_mod(columns[low + half_width], twiddle);
        ulong sum = add_mod(left, right);
        ulong difference = sub_mod(left, right);
        if (args.stage + 1U == args.log_len) {
            sum = mul_mod(sum, args.normalization);
            difference = mul_mod(difference, args.normalization);
        }
        columns[low] = sum;
        columns[low + half_width] = difference;
        butterfly += 256UL;
        // For half_width below 2048, every group begins at offset zero.
        // Otherwise its 2048 butterflies fit wholly inside one block.
        twiddle = offset + 256UL >= half_width
            ? first_twiddle : mul_mod(twiddle, twiddle_stride);
        offset = butterfly & (half_width - 1UL);
    }
}
