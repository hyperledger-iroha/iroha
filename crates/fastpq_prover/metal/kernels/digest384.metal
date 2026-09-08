#include <metal_stdlib>
using namespace metal;

#include "field.metal"

// Match the validated explicit scalar Poseidon state layout. These lanes use
// independently generated Digest384 constants, never the auxiliary hash table.
struct Digest384PoseidonState {
    ulong x;
    ulong y;
    ulong z;
};

inline void digest384_permute(
    thread Digest384PoseidonState &state,
    constant const ulong *round_constants,
    constant const ulong *mds
) {
    for (uint round = 0; round < 65; ++round) {
        state.x = add_mod(state.x, round_constants[round * 3]);
        state.y = add_mod(state.y, round_constants[round * 3 + 1]);
        state.z = add_mod(state.z, round_constants[round * 3 + 2]);
        state.x = pow7(state.x);
        if (round < 4 || round >= 61) {
            state.y = pow7(state.y);
            state.z = pow7(state.z);
        }
        Digest384PoseidonState next;
        next.x = add_mod(add_mod(mul_mod(mds[0], state.x), mul_mod(mds[1], state.y)), mul_mod(mds[2], state.z));
        next.y = add_mod(add_mod(mul_mod(mds[3], state.x), mul_mod(mds[4], state.y)), mul_mod(mds[5], state.z));
        next.z = add_mod(add_mod(mul_mod(mds[6], state.x), mul_mod(mds[7], state.y)), mul_mod(mds[8], state.z));
        state = next;
    }
}

inline void digest384_absorb(
    thread Digest384PoseidonState &state,
    thread uint &position,
    ulong word,
    constant const ulong *round_constants,
    constant const ulong *mds
) {
    if (position == 0) {
        state.x = add_mod(state.x, word);
    } else {
        state.y = add_mod(state.y, word);
    }
    ++position;
    if (position == 2) {
        digest384_permute(state, round_constants, mds);
        position = 0;
    }
}

// Buffers use scalar words: four prefix words per lane (x,y,z,next_position),
// six lanes per job, two slice words per job (byte offset, byte length), and
// six output words per job. Typed framing and final-field length are already
// bound by the canonical CPU stream before these fresh snapshots are uploaded.
kernel void fastpq_digest384_last_fields(
    device const ulong *prefixes [[buffer(0)]],
    device const uchar *payload [[buffer(1)]],
    device const ulong *slices [[buffer(2)]],
    constant const ulong *lane_round_constants [[buffer(3)]],
    constant const ulong *mds [[buffer(4)]],
    device ulong *output [[buffer(5)]],
    constant uint &job_count [[buffer(6)]],
    uint gid [[thread_position_in_grid]]
) {
    const ulong job = (ulong)gid / 6;
    if (job >= (ulong)job_count) {
        return;
    }
    const uint lane = gid % 6;
    const ulong prefix_offset = (job * 6 + lane) * 4;
    Digest384PoseidonState state = {
        prefixes[prefix_offset],
        prefixes[prefix_offset + 1],
        prefixes[prefix_offset + 2]
    };
    uint position = (uint)prefixes[prefix_offset + 3];
    constant const ulong *round_constants = lane_round_constants + lane * 65 * 3;
    const ulong offset = slices[job * 2];
    const ulong length = slices[job * 2 + 1];
    const ulong whole = length / 7 * 7;
    for (ulong cursor = 0; cursor < whole; cursor += 7) {
        ulong word = 0;
        for (uint byte = 0; byte < 7; ++byte) {
            word |= (ulong)payload[offset + cursor + byte] << (byte * 8);
        }
        digest384_absorb(state, position, word, round_constants, mds);
    }
    ulong terminal = 0;
    const uint remaining = (uint)(length - whole);
    for (uint byte = 0; byte < remaining; ++byte) {
        terminal |= (ulong)payload[offset + whole + byte] << (byte * 8);
    }
    // The byte delimiter is mandatory even at exact seven-byte boundaries.
    terminal |= 1UL << (remaining * 8);
    digest384_absorb(state, position, terminal, round_constants, mds);
    // Separate prefix-free field-element terminator, matching stream.finalize().
    digest384_absorb(state, position, 1, round_constants, mds);
    if (position != 0) {
        digest384_permute(state, round_constants, mds);
    }
    output[job * 6 + lane] = state.x;
}
