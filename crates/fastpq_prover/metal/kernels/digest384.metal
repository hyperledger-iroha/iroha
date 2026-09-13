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

// Sum three full-width products before the single Goldilocks fold.
// The 130-bit sum is (top, hi, lo), where top is at most two. Since
// 2^128 == -2^32 (mod FIELD_MODULUS), its final correction is a subtraction.
// Coefficients, state order and all six independent permutation lanes are unchanged.
inline void digest384_add_wide_product(
    thread ulong &lo, thread ulong &hi, thread ulong &top, ulong a, ulong b
) {
    const ulong product_lo = a * b;
    const ulong product_hi = mulhi(a, b);
    const ulong next_lo = lo + product_lo;
    const ulong carry = (ulong)(next_lo < lo);
    const ulong next_hi = hi + product_hi;
    top += (ulong)(next_hi < hi);
    const ulong final_hi = next_hi + carry;
    top += (ulong)(final_hi < next_hi);
    lo = next_lo;
    hi = final_hi;
}

inline ulong digest384_mds_dot3(
    ulong a, ulong b, ulong c, ulong x, ulong y, ulong z
) {
    ulong lo = a * x;
    ulong hi = mulhi(a, x);
    ulong top = 0;
    digest384_add_wide_product(lo, hi, top, b, y);
    digest384_add_wide_product(lo, hi, top, c, z);
    return sub_mod(reduce_goldilocks(lo, hi), top << 32);
}

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
        next.x = digest384_mds_dot3(mds[0], mds[1], mds[2], state.x, state.y, state.z);
        next.y = digest384_mds_dot3(mds[3], mds[4], mds[5], state.x, state.y, state.z);
        next.z = digest384_mds_dot3(mds[6], mds[7], mds[8], state.x, state.y, state.z);
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

// Whole-frame primitive contract consumed by metal.rs::digest384_hash_frames_v1.
// Frames already contain canonical typed framing, termination and rate padding.
// Each job has six independent IV/round-constant lanes and one output per lane.
kernel void digest384_hash_frames_v1(
    device const ulong *words [[buffer(0)]],
    device const ulong *descriptors [[buffer(1)]],
    constant const ulong *parameters [[buffer(2)]],
    device ulong *output [[buffer(3)]],
    constant uint2 &args [[buffer(4)]],
    uint gid [[thread_position_in_grid]]
) {
    const ulong frame = (ulong)gid / 6;
    if (frame >= (ulong)args.x) return;
    const uint lane = gid % 6;
    const ulong offset = descriptors[frame * 3];
    const ulong length = descriptors[frame * 3 + 1];
    const ulong lane_word = descriptors[frame * 3 + 2];
    if (length == 0 || (length & 1UL) != 0 || lane_word >= length ||
        offset > (ulong)args.y || length > (ulong)args.y - offset) {
        output[gid] = FIELD_MODULUS;
        return;
    }
    Digest384PoseidonState state = {
        parameters[lane * 3], parameters[lane * 3 + 1], parameters[lane * 3 + 2]
    };
    constant const ulong *round_constants = parameters + 18 + lane * 65 * 3;
    constant const ulong *mds = parameters + 1188;
    for (ulong cursor = 0; cursor < length; cursor += 2) {
        const ulong x = cursor == lane_word ? (ulong)lane : words[offset + cursor];
        const ulong y = cursor + 1 == lane_word ? (ulong)lane : words[offset + cursor + 1];
        state.x = add_mod(state.x, x);
        state.y = add_mod(state.y, y);
        digest384_permute(state, round_constants, mds);
    }
    output[gid] = state.x;
}

// Isolated exact first-coordinate nonce-search candidate. The host has bound
// the canonical public domain prefix and complete fixed suffix for lane zero.
// The complete digest remains unchanged; only targets <=63 may use this value.
kernel void digest384_indexed_first_coordinate_v1(
    constant const ulong *cache [[buffer(0)]],
    constant const ulong *parameters [[buffer(1)]],
    device ulong *output [[buffer(2)]],
    constant const ulong *args [[buffer(3)]],
    uint gid [[thread_position_in_grid]]
) {
    if ((ulong)gid >= args[1]) return;
    const ulong length = cache[4];
    const ulong index_word = cache[3];
    if (length == 0 || (length & 1UL) || index_word + 1 >= length ||
        args[0] > ULONG_MAX - (ulong)gid) {
        output[gid] = FIELD_MODULUS;
        return;
    }
    const ulong nonce = args[0] + (ulong)gid;
    Digest384PoseidonState state = {cache[0],cache[1],cache[2]};
    constant const ulong *round_constants = parameters + 18;
    constant const ulong *mds = parameters + 1188;
    for (ulong cursor=0; cursor<length; cursor+=2) {
        ulong x = cache[5+cursor];
        ulong y = cache[5+cursor+1];
        if (cursor == index_word) x = nonce & 0x00ffffffffffffffUL;
        if (cursor == index_word+1) x = (nonce >> 56) | 0x100UL;
        if (cursor+1 == index_word) y = nonce & 0x00ffffffffffffffUL;
        if (cursor+1 == index_word+1) y = (nonce >> 56) | 0x100UL;
        state.x=add_mod(state.x,x);state.y=add_mod(state.y,y);
        digest384_permute(state,round_constants,mds);
    }
    output[gid]=state.x;
}
