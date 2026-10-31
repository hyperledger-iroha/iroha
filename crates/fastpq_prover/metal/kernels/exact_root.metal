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

// Public radix-256 exponent digits index the same root-specific 8KiB table.
// A zero digit contributes 1. Load only nonzero higher digits and multiply
// only when a lower digit already contributed a factor. For k nonzero digits
// this needs max(k - 1, 0) field multiplies, including every 32-bit exponent.
// Every branch and table address depends only on the public exponent.
inline ulong exact_root_power_v1(const device ulong *powers, uint exponent) {
    ulong value = powers[exponent & 255U];
    uint digit = (exponent >> 8U) & 255U;
    if (digit != 0U) {
        ulong factor = powers[256U + digit];
        value = (exponent & 255U) == 0U ? factor : mul_mod(value, factor);
    }
    digit = (exponent >> 16U) & 255U;
    if (digit != 0U) {
        ulong factor = powers[512U + digit];
        value = (exponent & 65535U) == 0U ? factor : mul_mod(value, factor);
    }
    digit = exponent >> 24U;
    if (digit != 0U) {
        ulong factor = powers[768U + digit];
        value = (exponent & 16777215U) == 0U ? factor : mul_mod(value, factor);
    }
    return value;
}

// Every primitive power-of-two root supplies one of the two fourth roots
// +/-2^48. Shifting its public twiddle avoids a general 64-bit multiply.
// Keep the exact general product for any other public argument.
inline ulong exact_root_upper_twiddle_v1(ulong twiddle, ulong fourth_root) {
    const ulong positive = 0x0001000000000000UL;
    const ulong negative = 0xfffeffff00000001UL;
    if (fourth_root == positive || fourth_root == negative) {
        ulong shifted = reduce_goldilocks(twiddle << 48U, twiddle >> 16U);
        return fourth_root == positive ? shifted : sub_mod(0UL, shifted);
    }
    return mul_mod(twiddle, fourth_root);
}

// One pair is written only by its lower index. Distinct pairs never overlap.
kernel void exact_root_bit_reverse_v1(
    device ulong *columns [[buffer(0)]],
    constant ExactRootFftArgs &args [[buffer(2)]],
    uint2 position [[thread_position_in_grid]]
) {
    ulong index = (ulong)position.x;
    if (index >= args.column_len || position.y >= args.column_count) return;
    // Fixed public bit permutation replaces a log_len-iteration loop for
    // every word, including the full 32-bit domain supported by the facade.
    uint reversed = position.x;
    reversed = ((reversed & 0x55555555U) << 1U) | ((reversed >> 1U) & 0x55555555U);
    reversed = ((reversed & 0x33333333U) << 2U) | ((reversed >> 2U) & 0x33333333U);
    reversed = ((reversed & 0x0f0f0f0fU) << 4U) | ((reversed >> 4U) & 0x0f0f0f0fU);
    reversed = ((reversed & 0x00ff00ffU) << 8U) | ((reversed >> 8U) & 0x00ff00ffU);
    reversed = ((reversed << 16U) | (reversed >> 16U)) >> (32U - args.log_len);
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
        if (local_log == args.log_len && args.normalization != 1UL) {
            value = mul_mod(value, args.normalization);
        }
        columns[start + lane] = value;
    }
}

// A group owns 1024 independent radix-four cells (4096 words), four per
// lane. Each cell completes two adjacent radix-two stages in registers; those
// stages share no values with any other cell, so no global barrier is omitted.
// An unpaired final stage keeps the original 2048-butterfly mapping.
// Host resource barriers separate invocations, not the two fused local stages.
kernel void exact_root_global_stage_v1(
    device ulong *columns [[buffer(0)]],
    const device ulong *root_powers [[buffer(1)]],
    constant ExactRootFftArgs &args [[buffer(2)]],
    uint2 group [[threadgroup_position_in_grid]],
    uint lane [[thread_index_in_threadgroup]]
) {
    ulong half_width = 1UL << args.stage;
    ulong column_base = (ulong)group.y * args.column_len;
    if (args.stage + 1U < args.log_len) {
        ulong cell = (ulong)group.x * 1024UL + (ulong)lane;
        ulong limit = args.column_len / 4UL;
        ulong offset = cell & (half_width - 1UL);
        uint root_shift = args.log_len - args.stage - 2U;
        ulong first_twiddle = exact_root_power_v1(root_powers, (uint)offset << root_shift);
        ulong twiddle = first_twiddle;
        ulong twiddle_stride = exact_root_power_v1(root_powers, 256U << root_shift);
        // The second stage's upper-half twiddle differs by this fourth root.
        ulong fourth_root = exact_root_power_v1(root_powers, 1U << (args.log_len - 2U));
        bool normalize = args.stage + 2U == args.log_len && args.normalization != 1UL;
        for (uint iteration = 0U; iteration < 4U && cell < limit; ++iteration) {
            ulong low = column_base + (cell - offset) * 4UL + offset;
            ulong squared_twiddle = mul_mod(twiddle, twiddle);
            ulong a = columns[low];
            ulong b = mul_mod(columns[low + half_width], squared_twiddle);
            ulong c = columns[low + 2UL * half_width];
            ulong d = mul_mod(columns[low + 3UL * half_width], squared_twiddle);
            ulong ab_sum = add_mod(a, b);
            ulong ab_difference = sub_mod(a, b);
            ulong cd_sum = mul_mod(add_mod(c, d), twiddle);
            ulong cd_difference = mul_mod(sub_mod(c, d), exact_root_upper_twiddle_v1(twiddle, fourth_root));
            ulong value0 = add_mod(ab_sum, cd_sum);
            ulong value1 = add_mod(ab_difference, cd_difference);
            ulong value2 = sub_mod(ab_sum, cd_sum);
            ulong value3 = sub_mod(ab_difference, cd_difference);
            if (normalize) {
                value0 = mul_mod(value0, args.normalization);
                value1 = mul_mod(value1, args.normalization);
                value2 = mul_mod(value2, args.normalization);
                value3 = mul_mod(value3, args.normalization);
            }
            columns[low] = value0;
            columns[low + half_width] = value1;
            columns[low + 2UL * half_width] = value2;
            columns[low + 3UL * half_width] = value3;
            cell += 256UL;
            twiddle = offset + 256UL >= half_width
                ? first_twiddle : mul_mod(twiddle, twiddle_stride);
            offset = cell & (half_width - 1UL);
        }
    } else {
        ulong butterfly = (ulong)group.x * 2048UL + (ulong)lane;
        ulong limit = args.column_len / 2UL;
        ulong offset = butterfly & (half_width - 1UL);
        uint root_shift = args.log_len - args.stage - 1U;
        ulong first_twiddle = exact_root_power_v1(root_powers, (uint)offset << root_shift);
        ulong twiddle = first_twiddle;
        ulong twiddle_stride = exact_root_power_v1(root_powers, 256U << root_shift);
        for (uint iteration = 0U; iteration < 8U && butterfly < limit; ++iteration) {
            ulong low = column_base + (butterfly - offset) * 2UL + offset;
            ulong left = columns[low];
            ulong right = mul_mod(columns[low + half_width], twiddle);
            ulong sum = add_mod(left, right);
            ulong difference = sub_mod(left, right);
            if (args.normalization != 1UL) {
                sum = mul_mod(sum, args.normalization);
                difference = mul_mod(difference, args.normalization);
            }
            columns[low] = sum;
            columns[low + half_width] = difference;
            butterfly += 256UL;
            twiddle = offset + 256UL >= half_width
                ? first_twiddle : mul_mod(twiddle, twiddle_stride);
            offset = butterfly & (half_width - 1UL);
        }
    }
}
