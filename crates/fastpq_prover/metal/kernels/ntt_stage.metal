#include <metal_stdlib>
using namespace metal;

#include "../include/params.h"
#include "field.metal"

inline ulong bit_reverse_index(ulong index, uint log_len) {
    ulong reversed = 0UL;
    ulong tmp = index;
    for (uint i = 0U; i < log_len; ++i) {
        reversed = (reversed << 1UL) | (tmp & 1UL);
        tmp >>= 1UL;
    }
    return reversed;
}

inline void bit_reverse_in_place(device ulong *data, ulong len, uint log_len) {
    if (len <= 1UL) {
        return;
    }
    for (ulong i = 1UL; i < len - 1UL; ++i) {
        ulong j = bit_reverse_index(i, log_len);
        if (i < j) {
            ulong tmp = data[i];
            data[i] = data[j];
            data[j] = tmp;
        }
    }
}

inline void fft_apply_stage_range(
    device ulong *data,
    ulong len,
    uint start_stage,
    uint log_len,
    const device ulong *stage_twiddles,
    ulong coset
) {
    for (uint stage = start_stage; stage < log_len; ++stage) {
        ulong size = 1UL << (stage + 1U);
        ulong half_size = size >> 1UL;
        ulong wlen = stage_twiddles[stage];
        bool final_stage = (stage + 1U == log_len) && (coset != 1UL);

        for (ulong start = 0UL; start < len; start += size) {
            ulong w = 1UL;
            ulong coset_low = 1UL;
            ulong coset_high = final_stage ? pow_mod(coset, half_size) : 1UL;
            for (ulong j = 0UL; j < half_size; ++j) {
                ulong u = data[start + j];
                ulong v = mul_mod(data[start + j + half_size], w);
                ulong a = add_mod(u, v);
                ulong b = sub_mod(u, v);
                if (final_stage) {
                    data[start + j] = mul_mod(a, coset_low);
                    data[start + j + half_size] = mul_mod(b, coset_high);
                    coset_low = mul_mod(coset_low, coset);
                    coset_high = mul_mod(coset_high, coset);
                } else {
                    data[start + j] = a;
                    data[start + j + half_size] = b;
                }
                w = mul_mod(w, wlen);
            }
        }
    }
}

inline void fft_inplace(
    device ulong *data,
    ulong len,
    uint log_len,
    bool inverse
) {
    if (len <= 1UL) {
        return;
    }

    for (ulong i = 1UL; i < len - 1UL; ++i) {
        ulong j = bit_reverse_index(i, log_len);
        if (i < j) {
            ulong tmp = data[i];
            data[i] = data[j];
            data[j] = tmp;
        }
    }

    ulong base_exponent = (FIELD_MODULUS - 1UL) >> log_len;
    ulong omega = pow_mod(FIELD_GENERATOR, base_exponent);
    if (inverse) {
        omega = multiplicative_inverse(omega);
    }

    for (ulong size = 2UL; size <= len; size <<= 1UL) {
        ulong step = len / size;
        ulong wlen = pow_mod(omega, step);
        ulong half_size = size >> 1UL;

        for (ulong start = 0UL; start < len; start += size) {
            ulong w = 1UL;
            for (ulong j = 0UL; j < half_size; ++j) {
                ulong u = data[start + j];
                ulong v = mul_mod(data[start + j + half_size], w);
                data[start + j] = add_mod(u, v);
                data[start + j + half_size] = sub_mod(u, v);
                w = mul_mod(w, wlen);
            }
        }
    }

    if (inverse) {
        ulong inv_len = multiplicative_inverse(len);
        for (ulong i = 0UL; i < len; ++i) {
            data[i] = mul_mod(data[i], inv_len);
        }
    }
}

inline void fft_inplace_precomputed(
    device ulong *data,
    ulong len,
    uint log_len,
    bool inverse,
    const device ulong *stage_twiddles,
    ulong coset
) {
    if (len <= 1UL) {
        return;
    }
    bit_reverse_in_place(data, len, log_len);
    fft_apply_stage_range(data, len, 0U, log_len, stage_twiddles, coset);

    if (inverse) {
        ulong inv_len = multiplicative_inverse(len);
        for (ulong i = 0UL; i < len; ++i) {
            data[i] = mul_mod(data[i], inv_len);
        }
    }
}

inline void lde_eval(
    const device ulong *coeffs,
    device ulong *out,
    ulong trace_len,
    ulong eval_len,
    uint trace_log,
    uint blowup_log,
    ulong coset,
    const device ulong *stage_twiddles
) {
    for (ulong i = 0UL; i < trace_len; ++i) {
        out[i] = coeffs[i];
    }

    fft_inplace_precomputed(out, eval_len, trace_log + blowup_log, false, stage_twiddles, coset);
}

constant uint FFT_THREADGROUP_CAPACITY = 256U;
constant uint FFT_TILE_STAGE_CAP = 32U;

inline void apply_stage_tile(
    threadgroup ulong *tile,
    uint chunk_len,
    uint stage_idx,
    uint log_len,
    const device ulong *stage_twiddles,
    ulong coset,
    uint lane,
    uint lane_stride
) {
    ulong size = 1UL << (stage_idx + 1U);
    ulong half_size = size >> 1UL;
    uint half_width = (uint)half_size;
    if ((chunk_len < size) || (lane >= half_width)) {
        return;
    }
    uint butterflies = chunk_len >> 1;
    uint blocks = butterflies / half_width;
    ulong wlen = stage_twiddles[stage_idx];
    bool final_stage = (stage_idx + 1U == log_len) && (coset != 1UL);
    ulong w_seed = pow_mod_small(wlen, lane);
    ulong w_stride = pow_mod_small(wlen, lane_stride);
    ulong coset_stride = 1UL;
    ulong coset_seed = 1UL;
    ulong coset_half = 1UL;
    if (final_stage) {
        coset_seed = pow_mod_small(coset, lane);
        coset_stride = pow_mod_small(coset, lane_stride);
        coset_half = pow_mod(coset, half_size);
    }

    uint iterations = ((half_width - lane) + lane_stride - 1U) / lane_stride;
    for (uint block = 0U; block < blocks; ++block) {
        uint start = block * (uint)size;
        ulong w = w_seed;
        ulong coset_low = coset_seed;
        ulong coset_high = mul_mod(coset_low, coset_half);

        uint offset = lane;
        for (uint iter = 0U; iter < iterations; ++iter) {
            uint i0 = start + offset;
            uint i1 = i0 + (uint)half_size;
            ulong u = tile[i0];
            ulong v = mul_mod(tile[i1], w);
            ulong a = add_mod(u, v);
            ulong b = sub_mod(u, v);
            if (final_stage) {
                tile[i0] = mul_mod(a, coset_low);
                tile[i1] = mul_mod(b, coset_high);
                coset_low = mul_mod(coset_low, coset_stride);
                coset_high = mul_mod(coset_high, coset_stride);
            } else {
                tile[i0] = a;
                tile[i1] = b;
            }
            w = mul_mod(w, w_stride);
            offset += lane_stride;
        }
    }
}

inline void process_tile_stage_range(
    device ulong *column,
    ulong column_len,
    uint stage_start,
    uint stage_count,
    uint log_len,
    const device ulong *stage_twiddles,
    ulong coset,
    threadgroup ulong *tile,
    uint lane,
    uint lane_stride
) {
    ulong chunk_len = min((ulong)FFT_THREADGROUP_CAPACITY, column_len);
    for (ulong base = 0UL; base < column_len; base += chunk_len) {
        chunk_len = min(chunk_len, column_len - base);
        for (ulong chunk_offset = lane; chunk_offset < chunk_len; chunk_offset += lane_stride) {
            tile[chunk_offset] = column[base + chunk_offset];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
        for (uint local_stage = 0U; local_stage < stage_count; ++local_stage) {
            uint stage_idx = stage_start + local_stage;
            apply_stage_tile(
                tile,
                chunk_len,
                stage_idx,
                log_len,
                stage_twiddles,
                coset,
                lane,
                lane_stride
            );
            threadgroup_barrier(mem_flags::mem_threadgroup);
        }
        if (lane < chunk_len) {
            column[base + lane] = tile[lane];
        }
        threadgroup_barrier(mem_flags::mem_threadgroup);
    }
}

inline void apply_stage_global(
    device ulong *column,
    ulong column_len,
    uint stage_idx,
    uint log_len,
    const device ulong *stage_twiddles,
    ulong coset,
    uint lane,
    uint lane_stride
) {
    ulong size = 1UL << (stage_idx + 1U);
    ulong half_size = size >> 1UL;
    if (lane >= half_size) {
        return;
    }
    ulong blocks = column_len / size;
    ulong wlen = stage_twiddles[stage_idx];
    bool final_stage = (stage_idx + 1U == log_len) && (coset != 1UL);
    ulong stride = (ulong)max(1U, lane_stride);
    ulong w_seed = pow_mod(wlen, (ulong)lane);
    ulong w_stride = pow_mod(wlen, stride);
    ulong coset_seed = 1UL;
    ulong coset_stride = 1UL;
    ulong coset_half = 1UL;
    if (final_stage) {
        coset_seed = pow_mod(coset, (ulong)lane);
        coset_stride = pow_mod(coset, stride);
        coset_half = pow_mod(coset, half_size);
    }

    ulong iterations = ((half_size - (ulong)lane) + stride - 1UL) / stride;
    for (ulong block = 0UL; block < blocks; ++block) {
        ulong start = block * size;
        ulong w = w_seed;
        ulong coset_low = coset_seed;
        ulong coset_high = mul_mod(coset_low, coset_half);

        ulong offset = (ulong)lane;
        for (ulong iter = 0UL; iter < iterations; ++iter) {
            ulong i0 = start + offset;
            ulong i1 = i0 + half_size;
            ulong u = column[i0];
            ulong v = mul_mod(column[i1], w);
            ulong a = add_mod(u, v);
            ulong b = sub_mod(u, v);
            if (final_stage) {
                column[i0] = mul_mod(a, coset_low);
                column[i1] = mul_mod(b, coset_high);
                coset_low = mul_mod(coset_low, coset_stride);
                coset_high = mul_mod(coset_high, coset_stride);
            } else {
                column[i0] = a;
                column[i1] = b;
            }
            w = mul_mod(w, w_stride);
            offset += stride;
        }
    }
}

inline void fft_apply_stage_range_global(
    device ulong *column,
    ulong column_len,
    uint stage_start,
    uint log_len,
    const device ulong *stage_twiddles,
    ulong coset,
    uint lane,
    uint lane_stride
) {
    for (uint stage = stage_start; stage < log_len; ++stage) {
        threadgroup_barrier(mem_flags::mem_device);
        apply_stage_global(
            column,
            column_len,
            stage,
            log_len,
            stage_twiddles,
            coset,
            lane,
            lane_stride
        );
        threadgroup_barrier(mem_flags::mem_device);
    }
}

kernel void fastpq_fft_columns(
    device ulong *columns [[ buffer(0) ]],
    const device ulong *stage_twiddles [[ buffer(1) ]],
    constant FftArgs &args [[ buffer(2) ]],
    uint3 threadgroup_pos [[ threadgroup_position_in_grid ]],
    uint3 thread_pos [[ thread_position_in_threadgroup ]]
) {
    uint column_idx = threadgroup_pos.x + args.column_offset;
    if (column_idx >= args.column_count) {
        return;
    }
    uint lane = thread_pos.x;
    uint lanes = max(1U, min(args.threadgroup_lanes, FFT_THREADGROUP_CAPACITY));
    ulong len = args.column_len;
    device ulong *column = columns + (ulong)column_idx * len;
    threadgroup ulong tile[FFT_THREADGROUP_CAPACITY];

    if (lane == 0U) {
        bit_reverse_in_place(column, len, args.log_len);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    uint capped_limit = min(args.local_stage_limit, FFT_TILE_STAGE_CAP);
    uint local_limit = min(args.log_len, capped_limit);
    uint processed = 0U;
    while (processed + 1U < local_limit) {
        process_tile_stage_range(
            column,
            len,
            processed,
            2U,
            args.log_len,
            stage_twiddles,
            1UL,
            tile,
            lane,
            lanes
        );
        processed += 2U;
    }
    if (processed < local_limit) {
        process_tile_stage_range(
            column,
            len,
            processed,
            1U,
            args.log_len,
            stage_twiddles,
            1UL,
            tile,
            lane,
            lanes
        );
        processed += 1U;
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);
    if ((processed >= args.log_len) && (args.inverse != 0U)) {
        ulong inv_len = multiplicative_inverse(len);
        ulong lane_stride = (ulong)lanes;
        for (ulong idx = (ulong)lane; idx < len; idx += lane_stride) {
            column[idx] = mul_mod(column[idx], inv_len);
        }
    }
}

kernel void fastpq_lde_columns(
    const device ulong *coeffs [[ buffer(0) ]],
    device ulong *out_columns [[ buffer(1) ]],
    const device ulong *stage_twiddles [[ buffer(2) ]],
    constant LdeArgs &args [[ buffer(3) ]],
    uint3 threadgroup_pos [[ threadgroup_position_in_grid ]],
    uint3 thread_pos [[ thread_position_in_threadgroup ]]
) {
    uint column_idx = threadgroup_pos.x + args.column_offset;
    if (column_idx >= args.column_count) {
        return;
    }

    uint lane = thread_pos.x;
    uint lanes = max(1U, min(args.threadgroup_lanes, FFT_THREADGROUP_CAPACITY));
    ulong trace_len = args.trace_len;
    ulong eval_len = args.eval_len;
    uint eval_log = args.trace_log + args.blowup_log;
    const device ulong *column_coeffs = coeffs + (ulong)column_idx * trace_len;
    device ulong *column = out_columns + (ulong)column_idx * eval_len;
    threadgroup ulong tile[FFT_THREADGROUP_CAPACITY];

    ulong lane_stride = (ulong)lanes;
    for (ulong idx = (ulong)lane; idx < trace_len; idx += lane_stride) {
        column[idx] = column_coeffs[idx];
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    if (lane == 0U) {
        bit_reverse_in_place(column, eval_len, eval_log);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    uint capped_limit = min(args.local_stage_limit, FFT_TILE_STAGE_CAP);
    uint local_limit = min(eval_log, capped_limit);
    uint processed = 0U;
    while (processed + 1U < local_limit) {
        process_tile_stage_range(
            column,
            eval_len,
            processed,
            2U,
            eval_log,
            stage_twiddles,
            args.coset,
            tile,
            lane,
            lanes
        );
        processed += 2U;
    }
    if (processed < local_limit) {
        process_tile_stage_range(
            column,
            eval_len,
            processed,
            1U,
            eval_log,
            stage_twiddles,
            args.coset,
            tile,
            lane,
            lanes
        );
        processed += 1U;
    }

    threadgroup_barrier(mem_flags::mem_threadgroup);
}

kernel void fastpq_fft_post_tiling(
    device ulong *columns [[ buffer(0) ]],
    const device ulong *stage_twiddles [[ buffer(1) ]],
    constant PostTileArgs &args [[ buffer(2) ]],
    uint3 threadgroup_pos [[ threadgroup_position_in_grid ]],
    uint3 thread_pos [[ thread_position_in_threadgroup ]]
) {
    if (args.stage_start >= args.log_len) {
        return;
    }
    uint column_idx = threadgroup_pos.x + args.column_offset;
    if (column_idx >= args.column_count) {
        return;
    }
    uint lane = thread_pos.x;
    uint lanes = max(1U, min(args.threadgroup_lanes, FFT_THREADGROUP_CAPACITY));
    device ulong *column = columns + (ulong)column_idx * args.column_len;
    fft_apply_stage_range_global(
        column,
        args.column_len,
        args.stage_start,
        args.log_len,
        stage_twiddles,
        args.coset,
        lane,
        lanes
    );
    threadgroup_barrier(mem_flags::mem_threadgroup);
    if (args.inverse != 0U) {
        ulong inv_len = multiplicative_inverse(args.column_len);
        ulong lane_stride = (ulong)lanes;
        for (ulong idx = (ulong)lane; idx < args.column_len; idx += lane_stride) {
            column[idx] = mul_mod(column[idx], inv_len);
        }
    }
}
