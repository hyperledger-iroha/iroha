#ifndef FASTPQ_METAL_PARAMS_H
#define FASTPQ_METAL_PARAMS_H

struct FftArgs {
    ulong column_len;
    uint log_len;
    uint column_count;
    uint inverse;
    uint local_stage_limit;
    uint threadgroup_lanes;
    uint column_offset;
    uint _padding;
};

struct LdeArgs {
    ulong trace_len;
    ulong eval_len;
    uint trace_log;
    uint blowup_log;
    uint column_count;
    uint column_offset;
    uint threadgroup_lanes;
    uint local_stage_limit;
    ulong coset;
};

struct PostTileArgs {
    ulong column_len;
    uint log_len;
    uint column_count;
    uint column_offset;
    uint stage_start;
    uint inverse;
    uint threadgroup_lanes;
    ulong coset;
};

#endif // FASTPQ_METAL_PARAMS_H
