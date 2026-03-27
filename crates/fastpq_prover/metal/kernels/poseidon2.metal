#include <metal_stdlib>
using namespace metal;

#include "field.metal"
struct PoseidonArgs {
    uint state_count;
    uint states_per_lane;
    uint block_count;
    uint reserved;
};
struct PoseidonFusedArgs {
    uint state_count;
    uint states_per_lane;
    uint block_count;
    uint leaf_offset;
    uint parent_offset;
};
struct PoseidonSlice {
    uint offset;
    uint len;
};
constant ushort STATE_WIDTH = 3;
constant ushort POSEIDON_RATE = 2;
constant ushort FULL_ROUNDS_HALF = 4;
constant ushort PARTIAL_ROUNDS = 57;
constant ushort TOTAL_ROUNDS = FULL_ROUNDS_HALF * 2 + PARTIAL_ROUNDS;
constant ushort STATE_CHUNK = 4;
constant ulong TRACE_NODE_DOMAIN_SEED = 6068040151459020611ul;

inline ulong3 make_ulong3(ulong a, ulong b, ulong c) {
    ulong3 value;
    value.x = a;
    value.y = b;
    value.z = c;
    return value;
}

inline ulong3 add_mod_vec(ulong3 lhs, ulong3 rhs) {
    return make_ulong3(
        add_mod(lhs.x, rhs.x),
        add_mod(lhs.y, rhs.y),
        add_mod(lhs.z, rhs.z));
}

inline ulong3 pow5_vec(ulong3 value) {
    return make_ulong3(
        pow5(value.x),
        pow5(value.y),
        pow5(value.z));
}
constant ulong ROUND_CONSTANTS[TOTAL_ROUNDS][STATE_WIDTH] = {
    {0x0355DEFB099BED96UL, 0xFCC55FB9681355E6UL, 0x931368D725F8720AUL},
    {0x3B8752F81E4E100AUL, 0x53B864EE53DE1E7CUL, 0xFCCB110A3A839800UL},
    {0xFFF0E99012E32773UL, 0xFCA28AB2898EFF49UL, 0xB02B1C854E99B411UL},
    {0x9C45A0B2ADEDDCDDUL, 0xAB6BD8D6B4959CFAUL, 0x32EB07EB92BECCA5UL},
    {0x02698CDA7AA8674AUL, 0xA6914454FFBBC85AUL, 0x330402AF7562DE69UL},
    {0x6B9DEB689731670AUL, 0xB6ACBA1824052A6EUL, 0x3AD181A6AAED99ECUL},
    {0x871F25345B28B8E9UL, 0xCD88D358315EEEC1UL, 0xE051DA577C86BE1DUL},
    {0x153AD1817C0C125AUL, 0x97461B442E8ABE8CUL, 0x86591AF51639F5A9UL},
    {0xB5BBC2A445CE1D3CUL, 0x3CF87DF379503557UL, 0x69C87EF4B926F914UL},
    {0x941BBA1E4EF7E9F5UL, 0x2F9B29B58901FE31UL, 0x49C95BE066D4E00BUL},
    {0xCDB4FA571ED16B7EUL, 0x81526EE1686C1130UL, 0x9AFE7622FD187B9DUL},
    {0x1C6D3D32F5666A4AUL, 0xBC714A509ED0C9E2UL, 0xDBD44F433D9BB989UL},
    {0x07F9A90B7E43D95CUL, 0xBF0830BC8E11BE74UL, 0x97F82D7658D424AFUL},
    {0x232D735BD912E717UL, 0x24C5847CA7491E7AUL, 0x050E45F9AD018DF7UL},
    {0x3D875E313E445623UL, 0xF0E765B8E6350E3AUL, 0x24E1F73AACB2A5D8UL},
    {0xA2608CAC4EDDCBF6UL, 0xBB84373A14012579UL, 0x153FF026357DDB2EUL},
    {0x31C51990051B2FCEUL, 0xC4326A6DF1342061UL, 0x110AB731271E081DUL},
    {0xD7D4ED983F738F7DUL, 0x04C2243A9D2D2DD8UL, 0xEC84BD9F09F4A4EEUL},
    {0x459C6A91E8310F7DUL, 0xBD5448DB7B312560UL, 0x55405BC1D4841791UL},
    {0x9B29AA29D32FF2EFUL, 0x19B4882D7D91817DUL, 0xFF96EEFF0D35D395UL},
    {0x725BEE8D2E752EBAUL, 0xB778A21C64310114UL, 0x9991CF6141E2878AUL},
    {0x5BAF8B23F6D30359UL, 0x17D0B954A0C24E6DUL, 0x3050C4078053B0D5UL},
    {0xF9F52A3090EFA474UL, 0xA598D52915F99806UL, 0xBF48572B618FBD61UL},
    {0xA34BFA5C0B92EC3EUL, 0xAB1115736057C63CUL, 0x2F7D416846014AAAUL},
    {0xE19F214292D3A546UL, 0xB48CED4361042203UL, 0xB64F6BFC1CFCBD92UL},
    {0xE0F48EECD39B7CBFUL, 0xC4384583C0D6450DUL, 0x7460CAD33F436ACAUL},
    {0xA69F9EFF5CAF7223UL, 0x82ACD34E811E340AUL, 0xECB3D78F9881BEDFUL},
    {0x976760DB1C85079AUL, 0xA0D527128F5231CAUL, 0xA8823B80503C432BUL},
    {0xC41AFCA19EB78258UL, 0x33B30793338E26EEUL, 0x3369D80A26F26384UL},
    {0x4F5237B43FE7C081UL, 0x3E2E28D3737926D1UL, 0x9E9FD9449EAFC854UL},
    {0xF6DBF8E50187050FUL, 0x0883A7372F01A7D2UL, 0xE85C8CBE6053FA56UL},
    {0x43E66DEA535DCE9DUL, 0xFF7445DB745C243BUL, 0xDE5D71D4D7709458UL},
    {0xB14F0F45451D99F9UL, 0xFCDF81F971206FC3UL, 0x2508429ED0BBABFAUL},
    {0x408115A6F42A57E8UL, 0xFCDE33569C03693DUL, 0x83926CC606241ABDUL},
    {0xFE6191419D8E0A84UL, 0x358D71362D234258UL, 0x3BFB29C64195C104UL},
    {0x0ED6B527FE034AD7UL, 0xCD1998FE841A698AUL, 0x6AB890EBBFE772A9UL},
    {0x099A748ED5415721UL, 0x254B906A5A91BC8AUL, 0x4D5130A30F0CBB72UL},
    {0xD0932EA3FA64D467UL, 0xBE72249D94AEA218UL, 0x06215750FD6A29C5UL},
    {0xD79031484DB89846UL, 0xDD89CAD959BA24A4UL, 0xDC046DFA44871254UL},
    {0xB36F522BF4AF2EF6UL, 0x92796EAE38632589UL, 0x9796638178A01E39UL},
    {0xBE2C6A4F43121EFFUL, 0x0C20942EA145116BUL, 0xB6D21C3AF9D7AF98UL},
    {0x953AF8C79C3E074EUL, 0xC039153065B4A5C5UL, 0x9F51AFAF415B8A55UL},
    {0x6198A82927569707UL, 0xA09366E5BB6A66C7UL, 0x68E710BE9451FF59UL},
    {0x9226CCA1E4988DDDUL, 0x27E15510153C0224UL, 0x8D6A3E1FC7C197F2UL},
    {0xFAA929E25257BC60UL, 0x6B37C307AC47AA83UL, 0x907B20A35CFE4FC2UL},
    {0x50350676383D204EUL, 0x446C05DD253B59E8UL, 0x0F31889E23C5D5EAUL},
    {0x38EC3E1C2E667720UL, 0x026FDF27B554FF10UL, 0x1CA82D95C6FAFD39UL},
    {0xDDD0BC056342E191UL, 0x2D315FB44B04AC30UL, 0xFD4B193BAE11B94BUL},
    {0x086DF800B3489093UL, 0xF30FAE7604B987E2UL, 0x10A19793FE69EBE8UL},
    {0x0C77141B7EDF0449UL, 0x5EA875779328DEB1UL, 0x0B674C79767E421FUL},
    {0x4A5A726D82F3258EUL, 0x09E8C7A8BE47B1DEUL, 0xFFFA2AC085276013UL},
    {0xC09139324AA9D4F9UL, 0xB51A23205924FD17UL, 0xDCC8894949E02DA4UL},
    {0x227483529222AD6EUL, 0x65337B2101DFC3D8UL, 0xE19DFD7C78AAACC7UL},
    {0xF1B98DF340A16CA6UL, 0x18D543123D27950DUL, 0xC0C16B9B6007E396UL},
    {0xB90BC3C0D0605C8CUL, 0xDE108AC1D6C0BDCAUL, 0x5EE8196B61146783UL},
    {0x2AABA85C6BBCF12FUL, 0x02FF605B73843154UL, 0x2C37FC5E4DFEBB1EUL},
    {0xCB1CA352A46E4D33UL, 0x187D0F76FC0FB4A3UL, 0xAEADCDFE0A66B1F1UL},
    {0xC12532BFB67E3436UL, 0x4B403B0BF220033EUL, 0xE0DEC1015D69CD0BUL},
    {0x5D3EB71F9EF30C4AUL, 0xF82D9FBCDF532AEBUL, 0xA362551D86BEBD87UL},
    {0x6823E25802851126UL, 0x189CE62B77650805UL, 0xEFC261FFA36BF041UL},
    {0x478863511CBAF173UL, 0x4F4BD042C56B7936UL, 0x5B4CF8CC8584CA9AUL},
    {0xCC944E5F76063E0CUL, 0xD29A0B002C783CA7UL, 0x2F59EFCE5CDE182BUL},
    {0x93A0767D9186685CUL, 0xEE2501A761ECF4E5UL, 0xE7514FA48B145686UL},
    {0x5E0F189E8C61D97CUL, 0xEBE9BD9BEF0F5443UL, 0xD6CFE2A76189672AUL},
    {0x38FF8F40252C1AB7UL, 0x3CF9C4C3804D8166UL, 0x512F1E3AC8D0FFE5UL},
};

constant ulong MDS[STATE_WIDTH][STATE_WIDTH] = {
    {0x982513A23D22B592UL, 0xA3115DB8CF1D9C90UL, 0x46BA684B9EEE84B7UL},
    {0xBE3DCE25491DB768UL, 0xFB0A6F731943519FUL, 0xFCE5BD953CDE1896UL},
    {0xE624719C41EB1A09UL, 0xD2221B0F1AA2EBC4UL, 0x1AB5E60D03AD44BCUL},
};

inline void apply_mds(
    thread ulong3 &state,
    thread const ulong3 (&mds_rows)[STATE_WIDTH]
) {
    ulong3 next;

    ulong3 row0 = mds_rows[0];
    ulong acc = mul_mod(row0.x, state.x);
    acc = add_mod(acc, mul_mod(row0.y, state.y));
    acc = add_mod(acc, mul_mod(row0.z, state.z));
    next.x = acc;

    ulong3 row1 = mds_rows[1];
    acc = mul_mod(row1.x, state.x);
    acc = add_mod(acc, mul_mod(row1.y, state.y));
    acc = add_mod(acc, mul_mod(row1.z, state.z));
    next.y = acc;

    ulong3 row2 = mds_rows[2];
    acc = mul_mod(row2.x, state.x);
    acc = add_mod(acc, mul_mod(row2.y, state.y));
    acc = add_mod(acc, mul_mod(row2.z, state.z));
    next.z = acc;

    state = next;
}

inline void full_round_chunk(
    thread ulong3 *chunk,
    ushort count,
    threadgroup ulong3 (&rounds)[TOTAL_ROUNDS],
    uint round_idx,
    thread const ulong3 (&mds)[STATE_WIDTH]
) {
    ulong3 rc = rounds[round_idx];
    for (ushort i = 0; i < count; ++i) {
        chunk[i] = pow5_vec(add_mod_vec(chunk[i], rc));
        apply_mds(chunk[i], mds);
    }
}

inline void partial_round_chunk(
    thread ulong3 *chunk,
    ushort count,
    threadgroup ulong3 (&rounds)[TOTAL_ROUNDS],
    uint round_idx,
    thread const ulong3 (&mds)[STATE_WIDTH]
) {
    ulong3 rc = rounds[round_idx];
    for (ushort i = 0; i < count; ++i) {
        chunk[i] = add_mod_vec(chunk[i], rc);
        chunk[i].x = pow5(chunk[i].x);
        apply_mds(chunk[i], mds);
    }
}

inline void permute_chunk(
    thread ulong3 *chunk,
    ushort count,
    threadgroup ulong3 (&rounds)[TOTAL_ROUNDS],
    thread const ulong3 (&mds)[STATE_WIDTH]
) {
    uint round = 0;
#pragma clang loop unroll(full)
    for (uint i = 0; i < FULL_ROUNDS_HALF; ++i) {
        full_round_chunk(chunk, count, rounds, round, mds);
        round += 1;
    }
#pragma clang loop unroll(full)
    for (uint i = 0; i < PARTIAL_ROUNDS; ++i) {
        partial_round_chunk(chunk, count, rounds, round, mds);
        round += 1;
    }
#pragma clang loop unroll(full)
    for (uint i = 0; i < FULL_ROUNDS_HALF; ++i) {
        full_round_chunk(chunk, count, rounds, round, mds);
        round += 1;
    }
}

inline ushort load_state_chunk(
    thread ulong3 *chunk,
    ushort capacity,
    uint start_state,
    uint total_states,
    device const ulong *states
) {
    ushort loaded = 0;
    for (; loaded < capacity; ++loaded) {
        uint idx = start_state + loaded;
        if (idx >= total_states) {
            break;
        }
        uint base = idx * STATE_WIDTH;
        chunk[loaded].x = states[base];
        chunk[loaded].y = states[base + 1];
        chunk[loaded].z = states[base + 2];
    }
    return loaded;
}

inline void store_state_chunk(
    thread const ulong3 *chunk,
    ushort count,
    uint start_state,
    device ulong *states
) {
    for (ushort i = 0; i < count; ++i) {
        uint base = (start_state + i) * STATE_WIDTH;
        states[base] = chunk[i].x;
        states[base + 1] = chunk[i].y;
        states[base + 2] = chunk[i].z;
    }
}

inline void zero_state_chunk(thread ulong3 *chunk, ushort count) {
    for (ushort i = 0; i < count; ++i) {
        chunk[i] = make_ulong3(0, 0, 0);
    }
}

inline void absorb_block_chunk(
    thread ulong3 *chunk,
    ushort count,
    uint start_state,
    uint block,
    device const PoseidonSlice *slices,
    device const ulong *payloads
) {
    uint block_offset = block * POSEIDON_RATE;
    for (ushort i = 0; i < count; ++i) {
        uint column = start_state + i;
        PoseidonSlice slice = slices[column];
        ulong base = (ulong)slice.offset + block_offset;
        chunk[i].x = add_mod(chunk[i].x, payloads[base]);
        chunk[i].y = add_mod(chunk[i].y, payloads[base + 1]);
    }
}

kernel void poseidon_permute(device ulong *states [[ buffer(0) ]],
                             constant PoseidonArgs &args [[ buffer(1) ]],
                             uint3 grid_pos [[ thread_position_in_grid ]],
                             uint3 tg_pos [[ thread_position_in_threadgroup ]],
                             uint3 tg_size [[ threads_per_threadgroup ]]) {
    uint lane = tg_pos.x;
    threadgroup ulong3 shared_rounds[TOTAL_ROUNDS];
    threadgroup ulong3 shared_mds[STATE_WIDTH];

    uint lane_stride = tg_size.x == 0 ? 1 : tg_size.x;
    for (uint round = lane; round < TOTAL_ROUNDS; round += lane_stride) {
        shared_rounds[round] = make_ulong3(
            ROUND_CONSTANTS[round][0],
            ROUND_CONSTANTS[round][1],
            ROUND_CONSTANTS[round][2]);
    }
    for (uint row = lane; row < STATE_WIDTH; row += lane_stride) {
        shared_mds[row] = make_ulong3(MDS[row][0], MDS[row][1], MDS[row][2]);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    thread ulong3 local_mds[STATE_WIDTH];
    for (uint row = 0; row < STATE_WIDTH; ++row) {
        local_mds[row] = shared_mds[row];
    }

    uint states_per_lane = max(args.states_per_lane, 1u);
    uint state_offset = grid_pos.x * states_per_lane;
    thread ulong3 chunk[STATE_CHUNK];
    uint processed = 0;

    while (processed < states_per_lane) {
        uint start_state = state_offset + processed;
        if (start_state >= args.state_count) {
            break;
        }
        uint remaining = states_per_lane - processed;
        uint desired = remaining < STATE_CHUNK ? remaining : STATE_CHUNK;
        ushort capacity = (ushort)desired;
        ushort loaded = load_state_chunk(
            chunk,
            capacity,
            start_state,
            args.state_count,
            states);
        if (loaded == 0) {
            break;
        }
        permute_chunk(chunk, loaded, shared_rounds, local_mds);
        store_state_chunk(chunk, loaded, start_state, states);
        processed += loaded;
    }
}

kernel void poseidon_hash_columns(device const ulong *payloads [[ buffer(0) ]],
                                  device const PoseidonSlice *slices [[ buffer(1) ]],
                                  device ulong *states [[ buffer(2) ]],
                                  constant PoseidonArgs &args [[ buffer(3) ]],
                                  uint3 grid_pos [[ thread_position_in_grid ]],
                                  uint3 tg_pos [[ thread_position_in_threadgroup ]],
                                  uint3 tg_size [[ threads_per_threadgroup ]]) {
    if (args.block_count == 0) {
        return;
    }
    uint lane = tg_pos.x;
    threadgroup ulong3 shared_rounds[TOTAL_ROUNDS];
    threadgroup ulong3 shared_mds[STATE_WIDTH];

    uint lane_stride = tg_size.x == 0 ? 1 : tg_size.x;
    for (uint round = lane; round < TOTAL_ROUNDS; round += lane_stride) {
        shared_rounds[round] = make_ulong3(
            ROUND_CONSTANTS[round][0],
            ROUND_CONSTANTS[round][1],
            ROUND_CONSTANTS[round][2]);
    }
    for (uint row = lane; row < STATE_WIDTH; row += lane_stride) {
        shared_mds[row] = make_ulong3(MDS[row][0], MDS[row][1], MDS[row][2]);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    thread ulong3 local_mds[STATE_WIDTH];
    for (uint row = 0; row < STATE_WIDTH; ++row) {
        local_mds[row] = shared_mds[row];
    }

    uint states_per_lane = max(args.states_per_lane, 1u);
    uint state_offset = grid_pos.x * states_per_lane;
    thread ulong3 chunk[STATE_CHUNK];
    uint processed = 0;

    while (processed < states_per_lane) {
        uint start_state = state_offset + processed;
        if (start_state >= args.state_count) {
            break;
        }
        uint remaining = states_per_lane - processed;
        uint desired = remaining < STATE_CHUNK ? remaining : STATE_CHUNK;
        ushort capacity = (ushort)desired;
        ushort loaded = 0;
        for (; loaded < capacity; ++loaded) {
            uint idx = start_state + loaded;
            if (idx >= args.state_count) {
                break;
            }
            chunk[loaded] = make_ulong3(0, 0, 0);
        }
        if (loaded == 0) {
            break;
        }
        for (uint block = 0; block < args.block_count; ++block) {
            absorb_block_chunk(
                chunk,
                loaded,
                start_state,
                block,
                slices,
                payloads);
            permute_chunk(chunk, loaded, shared_rounds, local_mds);
        }
        store_state_chunk(chunk, loaded, start_state, states);
        processed += loaded;
    }
}

kernel void poseidon_trace_fused(device const ulong *payloads [[ buffer(0) ]],
                                 device const PoseidonSlice *slices [[ buffer(1) ]],
                                 device ulong *out_hashes [[ buffer(2) ]],
                                 constant PoseidonFusedArgs &args [[ buffer(3) ]],
                                 uint3 grid_pos [[ thread_position_in_grid ]],
                                 uint3 tg_pos [[ thread_position_in_threadgroup ]],
                                 uint3 tg_size [[ threads_per_threadgroup ]]) {
    if (args.block_count == 0) {
        return;
    }
    uint lane = tg_pos.x;
    threadgroup ulong3 shared_rounds[TOTAL_ROUNDS];
    threadgroup ulong3 shared_mds[STATE_WIDTH];

    uint lane_stride = tg_size.x == 0 ? 1 : tg_size.x;
    for (uint round = lane; round < TOTAL_ROUNDS; round += lane_stride) {
        shared_rounds[round] = make_ulong3(
            ROUND_CONSTANTS[round][0],
            ROUND_CONSTANTS[round][1],
            ROUND_CONSTANTS[round][2]);
    }
    for (uint row = lane; row < STATE_WIDTH; row += lane_stride) {
        shared_mds[row] = make_ulong3(MDS[row][0], MDS[row][1], MDS[row][2]);
    }
    threadgroup_barrier(mem_flags::mem_threadgroup);

    thread ulong3 local_mds[STATE_WIDTH];
    for (uint row = 0; row < STATE_WIDTH; ++row) {
        local_mds[row] = shared_mds[row];
    }

    uint states_per_lane = max(args.states_per_lane, 1u);
    uint state_offset = grid_pos.x * states_per_lane;
    thread ulong3 chunk[STATE_CHUNK];
    uint processed = 0;
    ulong pending_leaf = 0;
    uint pending_index = 0;
    bool has_pending_leaf = false;

    while (processed < states_per_lane) {
        uint start_state = state_offset + processed;
        if (start_state >= args.state_count) {
            break;
        }
        uint remaining = states_per_lane - processed;
        uint desired = remaining < STATE_CHUNK ? remaining : STATE_CHUNK;
        ushort capacity = (ushort)desired;
        ushort loaded = 0;
        for (; loaded < capacity; ++loaded) {
            uint idx = start_state + loaded;
            if (idx >= args.state_count) {
                break;
            }
            chunk[loaded] = make_ulong3(0, 0, 0);
        }
        if (loaded == 0) {
            break;
        }
        for (uint block = 0; block < args.block_count; ++block) {
            absorb_block_chunk(
                chunk,
                loaded,
                start_state,
                block,
                slices,
                payloads);
            permute_chunk(chunk, loaded, shared_rounds, local_mds);
        }
        for (ushort i = 0; i < loaded; ++i) {
            uint column = start_state + i;
            ulong leaf = chunk[i].x;
            out_hashes[args.leaf_offset + column] = leaf;
            if ((column & 1u) == 0u) {
                pending_leaf = leaf;
                pending_index = column >> 1;
                has_pending_leaf = true;
                if (column + 1u >= args.state_count) {
                    thread ulong3 parent_chunk[1];
                    parent_chunk[0].x = TRACE_NODE_DOMAIN_SEED;
                    parent_chunk[0].y = leaf;
                    parent_chunk[0].z = 0;
                    permute_chunk(parent_chunk, 1, shared_rounds, local_mds);
                    parent_chunk[0].x = add_mod(parent_chunk[0].x, leaf);
                    parent_chunk[0].y = add_mod(parent_chunk[0].y, 1ul);
                    permute_chunk(parent_chunk, 1, shared_rounds, local_mds);
                    out_hashes[args.parent_offset + pending_index] = parent_chunk[0].x;
                    has_pending_leaf = false;
                }
            } else if (has_pending_leaf) {
                thread ulong3 parent_chunk[1];
                parent_chunk[0].x = TRACE_NODE_DOMAIN_SEED;
                parent_chunk[0].y = pending_leaf;
                parent_chunk[0].z = 0;
                permute_chunk(parent_chunk, 1, shared_rounds, local_mds);
                parent_chunk[0].x = add_mod(parent_chunk[0].x, leaf);
                parent_chunk[0].y = add_mod(parent_chunk[0].y, 1ul);
                permute_chunk(parent_chunk, 1, shared_rounds, local_mds);
                out_hashes[args.parent_offset + (column >> 1)] = parent_chunk[0].x;
                has_pending_leaf = false;
            }
        }
        processed += loaded;
    }
}
