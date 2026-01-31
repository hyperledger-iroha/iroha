#import <Foundation/Foundation.h>
#import <Metal/Metal.h>
#include <stdint.h>
#include <string.h>

enum {
    RC_OK = 0,
    RC_INVALID = 1,
    RC_NO_SPACE = 2,
    RC_GPU_UNAVAILABLE = 3,
    RC_ZSTD = 4,
};

typedef struct {
    uint32_t len;
    uint32_t chunk_size;
    uint32_t min_match;
    uint32_t max_match;
} GpuZstdParams;

typedef struct {
    uint32_t lit_len;
    uint32_t match_len;
    uint32_t offset;
    uint32_t reserved;
} GpuZstdSequence;

static NSString *const kKernelSource =
    @"#include <metal_stdlib>\n"
    "using namespace metal;\n"
    "struct GpuZstdSequence { uint lit_len; uint match_len; uint offset; uint reserved; };\n"
    "struct GpuZstdParams { uint len; uint chunk_size; uint min_match; uint max_match; };\n"
    "constexpr uint BITSTREAM_MAX_BITS = 56u;\n"
    "struct BitWriter { ulong buffer; uint count; device uchar* out; uint out_len; uint out_pos; };\n"
    "inline void bitwriter_init(thread BitWriter& w, device uchar* out, uint out_len) {\n"
    "    w.buffer = 0ul;\n"
    "    w.count = 0u;\n"
    "    w.out = out;\n"
    "    w.out_len = out_len;\n"
    "    w.out_pos = 0u;\n"
    "}\n"
    "inline bool bitwriter_write_bits(thread BitWriter& w, ulong value, uint bits) {\n"
    "    if (bits > BITSTREAM_MAX_BITS) return false;\n"
    "    if (bits > 0u && bits < 64u && (value >> bits) != 0ul) return false;\n"
    "    if (bits == 0u) return true;\n"
    "    w.buffer |= (value << w.count);\n"
    "    w.count += bits;\n"
    "    while (w.count >= 8u) {\n"
    "        if (w.out_pos >= w.out_len) return false;\n"
    "        w.out[w.out_pos++] = uchar(w.buffer & 0xfful);\n"
    "        w.buffer >>= 8u;\n"
    "        w.count -= 8u;\n"
    "    }\n"
    "    return true;\n"
    "}\n"
    "inline bool bitwriter_flush_byte_aligned(thread BitWriter& w) {\n"
    "    if (w.count == 0u) return true;\n"
    "    if (w.out_pos >= w.out_len) return false;\n"
    "    w.out[w.out_pos++] = uchar(w.buffer & 0xfful);\n"
    "    w.buffer = 0ul;\n"
    "    w.count = 0u;\n"
    "    return true;\n"
    "}\n"
    "struct BitReader { ulong buffer; uint count; device const uchar* in; uint in_len; uint in_pos; };\n"
    "inline void bitreader_init(thread BitReader& r, device const uchar* in, uint in_len) {\n"
    "    r.buffer = 0ul;\n"
    "    r.count = 0u;\n"
    "    r.in = in;\n"
    "    r.in_len = in_len;\n"
    "    r.in_pos = 0u;\n"
    "}\n"
    "inline bool bitreader_fill(thread BitReader& r, uint bits) {\n"
    "    while (r.count < bits) {\n"
    "        if (r.in_pos >= r.in_len) return false;\n"
    "        r.buffer |= (ulong(r.in[r.in_pos++]) << r.count);\n"
    "        r.count += 8u;\n"
    "    }\n"
    "    return true;\n"
    "}\n"
    "inline bool bitreader_read_bits(thread BitReader& r, uint bits, thread ulong& out) {\n"
    "    if (bits > BITSTREAM_MAX_BITS) return false;\n"
    "    if (bits == 0u) { out = 0ul; return true; }\n"
    "    if (!bitreader_fill(r, bits)) return false;\n"
    "    ulong mask = (1ul << bits) - 1ul;\n"
    "    out = r.buffer & mask;\n"
    "    r.buffer >>= bits;\n"
    "    r.count -= bits;\n"
    "    return true;\n"
    "}\n"
    "inline void bitreader_align_to_byte(thread BitReader& r) {\n"
    "    uint skip = r.count & 7u;\n"
    "    r.buffer >>= skip;\n"
    "    r.count -= skip;\n"
    "}\n"
    "struct BitReaderRev { device const uchar* in; uint bit_pos; };\n"
    "inline void bitreader_rev_init(thread BitReaderRev& r, device const uchar* in, uint bit_len) {\n"
    "    r.in = in;\n"
    "    r.bit_pos = bit_len;\n"
    "}\n"
    "inline bool bitreader_rev_read_bits(thread BitReaderRev& r, uint bits, thread ulong& out) {\n"
    "    if (bits > BITSTREAM_MAX_BITS) return false;\n"
    "    if (bits == 0u) { out = 0ul; return true; }\n"
    "    if (bits > r.bit_pos) return false;\n"
    "    ulong value = 0ul;\n"
    "    for (uint i = 0; i < bits; ++i) {\n"
    "        r.bit_pos -= 1u;\n"
    "        uint idx = r.bit_pos;\n"
    "        uint byte_idx = idx >> 3;\n"
    "        uint bit_idx = idx & 7u;\n"
    "        ulong bit = (ulong(r.in[byte_idx]) >> bit_idx) & 1ul;\n"
    "        uint shift = bits - 1u - i;\n"
    "        value |= (bit << shift);\n"
    "    }\n"
    "    out = value;\n"
    "    return true;\n"
    "}\n"
    "constexpr uint HUFFMAN_SYMBOLS = 256u;\n"
    "constexpr uint HUFFMAN_MAX_NODES = 512u;\n"
    "inline ulong huff_reverse_bits(ulong value, uint bits) {\n"
    "    ulong out = 0ul;\n"
    "    for (uint i = 0; i < bits; ++i) {\n"
    "        out = (out << 1) | (value & 1ul);\n"
    "        value >>= 1u;\n"
    "    }\n"
    "    return out;\n"
    "}\n"
    "inline bool huff_cmp_node(uint a, uint b, thread const uint* weights, thread const ushort* node_id) {\n"
    "    if (weights[a] != weights[b]) return weights[a] < weights[b];\n"
    "    if (node_id[a] != node_id[b]) return node_id[a] < node_id[b];\n"
    "    return a < b;\n"
    "}\n"
    "inline void huff_order_nodes(uint a, uint b, thread const uint* weights, thread const ushort* node_id,\n"
    "                             thread uint& out_left, thread uint& out_right) {\n"
    "    if (huff_cmp_node(a, b, weights, node_id)) {\n"
    "        out_left = a;\n"
    "        out_right = b;\n"
    "    } else {\n"
    "        out_left = b;\n"
    "        out_right = a;\n"
    "    }\n"
    "}\n"
    "inline bool huff_select_two_smallest(thread const int* parent, thread const uint* weights,\n"
    "                                     thread const ushort* node_id, uint node_count,\n"
    "                                     thread uint& out_first, thread uint& out_second) {\n"
    "    bool has_first = false;\n"
    "    bool has_second = false;\n"
    "    uint first = 0;\n"
    "    uint second = 0;\n"
    "    for (uint i = 0; i < node_count; ++i) {\n"
    "        if (parent[i] >= 0) continue;\n"
    "        if (!has_first) {\n"
    "            first = i;\n"
    "            has_first = true;\n"
    "            continue;\n"
    "        }\n"
    "        if (huff_cmp_node(i, first, weights, node_id)) {\n"
    "            second = first;\n"
    "            has_second = has_first;\n"
    "            first = i;\n"
    "            has_first = true;\n"
    "            continue;\n"
    "        }\n"
    "        if (!has_second || huff_cmp_node(i, second, weights, node_id)) {\n"
    "            second = i;\n"
    "            has_second = true;\n"
    "        }\n"
    "    }\n"
    "    if (!has_first || !has_second) return false;\n"
    "    out_first = first;\n"
    "    out_second = second;\n"
    "    return true;\n"
    "}\n"
    "kernel void gpuzstd_huff_encode(\n"
    "    device const uchar* in [[buffer(0)]],\n"
    "    device uchar* out [[buffer(1)]],\n"
    "    device uchar* out_lengths [[buffer(2)]],\n"
    "    device uint* out_size [[buffer(3)]],\n"
    "    device uint* out_status [[buffer(4)]],\n"
    "    constant uint& in_len [[buffer(5)]],\n"
    "    constant uint& out_capacity [[buffer(6)]],\n"
    "    uint gid [[thread_position_in_grid]]) {\n"
    "    if (gid != 0) return;\n"
    "    for (uint i = 0; i < HUFFMAN_SYMBOLS; ++i) {\n"
    "        out_lengths[i] = 0;\n"
    "    }\n"
    "    out_size[0] = 0;\n"
    "    out_status[0] = 0;\n"
    "    if (in_len == 0) return;\n"
    "    thread uint counts[HUFFMAN_SYMBOLS];\n"
    "    for (uint i = 0; i < HUFFMAN_SYMBOLS; ++i) counts[i] = 0;\n"
    "    for (uint i = 0; i < in_len; ++i) {\n"
    "        counts[in[i]] += 1;\n"
    "    }\n"
    "    thread uint weights[HUFFMAN_MAX_NODES];\n"
    "    thread int parent[HUFFMAN_MAX_NODES];\n"
    "    thread ushort node_id[HUFFMAN_MAX_NODES];\n"
    "    thread int left[HUFFMAN_MAX_NODES];\n"
    "    thread int right[HUFFMAN_MAX_NODES];\n"
    "    thread int leaves[HUFFMAN_SYMBOLS];\n"
    "    for (uint i = 0; i < HUFFMAN_MAX_NODES; ++i) {\n"
    "        parent[i] = -1;\n"
    "        left[i] = -1;\n"
    "        right[i] = -1;\n"
    "        weights[i] = 0;\n"
    "        node_id[i] = 0;\n"
    "    }\n"
    "    uint leaf_count = 0;\n"
    "    for (uint sym = 0; sym < HUFFMAN_SYMBOLS; ++sym) {\n"
    "        if (counts[sym] == 0) {\n"
    "            leaves[sym] = -1;\n"
    "            continue;\n"
    "        }\n"
    "        uint idx = leaf_count++;\n"
    "        weights[idx] = counts[sym];\n"
    "        node_id[idx] = ushort(sym);\n"
    "        leaves[sym] = int(idx);\n"
    "    }\n"
    "    if (leaf_count == 0) return;\n"
    "    thread uchar lengths[HUFFMAN_SYMBOLS];\n"
    "    for (uint i = 0; i < HUFFMAN_SYMBOLS; ++i) lengths[i] = 0;\n"
    "    if (leaf_count == 1) {\n"
    "        for (uint sym = 0; sym < HUFFMAN_SYMBOLS; ++sym) {\n"
    "            if (leaves[sym] >= 0) {\n"
    "                lengths[sym] = 1;\n"
    "                out_lengths[sym] = 1;\n"
    "                break;\n"
    "            }\n"
    "        }\n"
    "        BitWriter writer;\n"
    "        bitwriter_init(writer, out, out_capacity);\n"
    "        for (uint i = 0; i < in_len; ++i) {\n"
    "            if (!bitwriter_write_bits(writer, 0ul, 1u)) {\n"
    "                out_status[0] = 2;\n"
    "                return;\n"
    "            }\n"
    "        }\n"
    "        if (!bitwriter_write_bits(writer, 1ul, 1u)) {\n"
    "            out_status[0] = 2;\n"
    "            return;\n"
    "        }\n"
    "        if (!bitwriter_flush_byte_aligned(writer)) {\n"
    "            out_status[0] = 2;\n"
    "            return;\n"
    "        }\n"
    "        out_size[0] = writer.out_pos;\n"
    "        return;\n"
    "    }\n"
    "    uint node_count = leaf_count;\n"
    "    uint remaining = leaf_count;\n"
    "    while (remaining > 1) {\n"
    "        uint first = 0;\n"
    "        uint second = 0;\n"
    "        if (!huff_select_two_smallest(parent, weights, node_id, node_count, first, second)) {\n"
    "            out_status[0] = 1;\n"
    "            return;\n"
    "        }\n"
    "        uint left_idx = 0;\n"
    "        uint right_idx = 0;\n"
    "        huff_order_nodes(first, second, weights, node_id, left_idx, right_idx);\n"
    "        uint new_idx = node_count++;\n"
    "        if (new_idx >= HUFFMAN_MAX_NODES) {\n"
    "            out_status[0] = 1;\n"
    "            return;\n"
    "        }\n"
    "        weights[new_idx] = weights[left_idx] + weights[right_idx];\n"
    "        node_id[new_idx] = min(node_id[left_idx], node_id[right_idx]);\n"
    "        parent[left_idx] = int(new_idx);\n"
    "        parent[right_idx] = int(new_idx);\n"
    "        left[new_idx] = int(left_idx);\n"
    "        right[new_idx] = int(right_idx);\n"
    "        remaining -= 1;\n"
    "    }\n"
    "    uchar max_len = 0;\n"
    "    for (uint sym = 0; sym < HUFFMAN_SYMBOLS; ++sym) {\n"
    "        int leaf = leaves[sym];\n"
    "        if (leaf < 0) continue;\n"
    "        uchar depth = 0;\n"
    "        int node = leaf;\n"
    "        while (node >= 0) {\n"
    "            int p = parent[uint(node)];\n"
    "            if (p < 0) break;\n"
    "            depth += 1;\n"
    "            node = p;\n"
    "        }\n"
    "        if (depth == 0) depth = 1;\n"
    "        if (depth > BITSTREAM_MAX_BITS) {\n"
    "            out_status[0] = 1;\n"
    "            return;\n"
    "        }\n"
    "        lengths[sym] = depth;\n"
    "        out_lengths[sym] = depth;\n"
    "        if (depth > max_len) max_len = depth;\n"
    "    }\n"
    "    thread uchar lens_sorted[HUFFMAN_SYMBOLS];\n"
    "    thread ushort syms_sorted[HUFFMAN_SYMBOLS];\n"
    "    uint pair_count = 0;\n"
    "    for (uint sym = 0; sym < HUFFMAN_SYMBOLS; ++sym) {\n"
    "        if (lengths[sym] > 0) {\n"
    "            lens_sorted[pair_count] = lengths[sym];\n"
    "            syms_sorted[pair_count] = ushort(sym);\n"
    "            pair_count += 1;\n"
    "        }\n"
    "    }\n"
    "    for (uint i = 0; i + 1 < pair_count; ++i) {\n"
    "        uint min_idx = i;\n"
    "        for (uint j = i + 1; j < pair_count; ++j) {\n"
    "            uchar len_a = lens_sorted[j];\n"
    "            uchar len_b = lens_sorted[min_idx];\n"
    "            ushort sym_a = syms_sorted[j];\n"
    "            ushort sym_b = syms_sorted[min_idx];\n"
    "            if (len_a < len_b || (len_a == len_b && sym_a < sym_b)) {\n"
    "                min_idx = j;\n"
    "            }\n"
    "        }\n"
    "        if (min_idx != i) {\n"
    "            uchar tmp_len = lens_sorted[i];\n"
    "            lens_sorted[i] = lens_sorted[min_idx];\n"
    "            lens_sorted[min_idx] = tmp_len;\n"
    "            ushort tmp_sym = syms_sorted[i];\n"
    "            syms_sorted[i] = syms_sorted[min_idx];\n"
    "            syms_sorted[min_idx] = tmp_sym;\n"
    "        }\n"
    "    }\n"
    "    thread ulong codes[HUFFMAN_SYMBOLS];\n"
    "    for (uint i = 0; i < HUFFMAN_SYMBOLS; ++i) codes[i] = 0ul;\n"
    "    ulong code = 0ul;\n"
    "    uchar cur_len = lens_sorted[0];\n"
    "    for (uint idx = 0; idx < pair_count; ++idx) {\n"
    "        uchar len = lens_sorted[idx];\n"
    "        ushort sym = syms_sorted[idx];\n"
    "        if (len > cur_len) {\n"
    "            code <<= uint(len - cur_len);\n"
    "            cur_len = len;\n"
    "        }\n"
    "        ulong reversed = huff_reverse_bits(code, len);\n"
    "        codes[sym] = reversed;\n"
    "        code += 1ul;\n"
    "    }\n"
    "    BitWriter writer;\n"
    "    bitwriter_init(writer, out, out_capacity);\n"
    "    for (uint idx = in_len; idx > 0; --idx) {\n"
    "        uchar sym = in[idx - 1];\n"
    "        uchar len = lengths[sym];\n"
    "        ulong sym_code = codes[sym];\n"
    "        if (!bitwriter_write_bits(writer, sym_code, len)) {\n"
    "            out_status[0] = 2;\n"
    "            return;\n"
    "        }\n"
    "    }\n"
    "    if (!bitwriter_write_bits(writer, 1ul, 1u)) {\n"
    "        out_status[0] = 2;\n"
    "        return;\n"
    "    }\n"
    "    if (!bitwriter_flush_byte_aligned(writer)) {\n"
    "        out_status[0] = 2;\n"
    "        return;\n"
    "    }\n"
    "    out_size[0] = writer.out_pos;\n"
    "}\n"
    "kernel void gpuzstd_huff_decode(\n"
    "    device const uchar* encoded [[buffer(0)]],\n"
    "    device const uchar* in_lengths [[buffer(1)]],\n"
    "    device uchar* out [[buffer(2)]],\n"
    "    device uint* out_status [[buffer(3)]],\n"
    "    constant uint& encoded_len [[buffer(4)]],\n"
    "    constant uint& out_len [[buffer(5)]],\n"
    "    uint gid [[thread_position_in_grid]]) {\n"
    "    if (gid != 0) return;\n"
    "    out_status[0] = 0;\n"
    "    if (out_len == 0) return;\n"
    "    thread uchar lengths[HUFFMAN_SYMBOLS];\n"
    "    for (uint i = 0; i < HUFFMAN_SYMBOLS; ++i) {\n"
    "        uchar len = in_lengths[i];\n"
    "        if (len > BITSTREAM_MAX_BITS) {\n"
    "            out_status[0] = 1;\n"
    "            return;\n"
    "        }\n"
    "        lengths[i] = len;\n"
    "    }\n"
    "    thread uchar lens_sorted[HUFFMAN_SYMBOLS];\n"
    "    thread ushort syms_sorted[HUFFMAN_SYMBOLS];\n"
    "    uint pair_count = 0;\n"
    "    for (uint sym = 0; sym < HUFFMAN_SYMBOLS; ++sym) {\n"
    "        if (lengths[sym] > 0) {\n"
    "            lens_sorted[pair_count] = lengths[sym];\n"
    "            syms_sorted[pair_count] = ushort(sym);\n"
    "            pair_count += 1;\n"
    "        }\n"
    "    }\n"
    "    if (pair_count == 0) {\n"
    "        out_status[0] = 1;\n"
    "        return;\n"
    "    }\n"
    "    for (uint i = 0; i + 1 < pair_count; ++i) {\n"
    "        uint min_idx = i;\n"
    "        for (uint j = i + 1; j < pair_count; ++j) {\n"
    "            uchar len_a = lens_sorted[j];\n"
    "            uchar len_b = lens_sorted[min_idx];\n"
    "            ushort sym_a = syms_sorted[j];\n"
    "            ushort sym_b = syms_sorted[min_idx];\n"
    "            if (len_a < len_b || (len_a == len_b && sym_a < sym_b)) {\n"
    "                min_idx = j;\n"
    "            }\n"
    "        }\n"
    "        if (min_idx != i) {\n"
    "            uchar tmp_len = lens_sorted[i];\n"
    "            lens_sorted[i] = lens_sorted[min_idx];\n"
    "            lens_sorted[min_idx] = tmp_len;\n"
    "            ushort tmp_sym = syms_sorted[i];\n"
    "            syms_sorted[i] = syms_sorted[min_idx];\n"
    "            syms_sorted[min_idx] = tmp_sym;\n"
    "        }\n"
    "    }\n"
    "    thread int child0[HUFFMAN_MAX_NODES];\n"
    "    thread int child1[HUFFMAN_MAX_NODES];\n"
    "    thread short symbol[HUFFMAN_MAX_NODES];\n"
    "    for (uint i = 0; i < HUFFMAN_MAX_NODES; ++i) {\n"
    "        child0[i] = -1;\n"
    "        child1[i] = -1;\n"
    "        symbol[i] = -1;\n"
    "    }\n"
    "    uint node_count = 1;\n"
    "    ulong code = 0ul;\n"
    "    uchar cur_len = lens_sorted[0];\n"
    "    for (uint idx = 0; idx < pair_count; ++idx) {\n"
    "        uchar len = lens_sorted[idx];\n"
    "        ushort sym = syms_sorted[idx];\n"
    "        if (len > cur_len) {\n"
    "            code <<= uint(len - cur_len);\n"
    "            cur_len = len;\n"
    "        }\n"
    "        ulong reversed = huff_reverse_bits(code, len);\n"
    "        uint node = 0;\n"
    "        for (uint bit_idx = 0; bit_idx < len; ++bit_idx) {\n"
    "            ulong bit = (reversed >> bit_idx) & 1ul;\n"
    "            int* next = bit == 0ul ? &child0[node] : &child1[node];\n"
    "            if (*next < 0) {\n"
    "                if (node_count >= HUFFMAN_MAX_NODES) {\n"
    "                    out_status[0] = 1;\n"
    "                    return;\n"
    "                }\n"
    "                *next = int(node_count++);\n"
    "            }\n"
    "            node = uint(*next);\n"
    "        }\n"
    "        symbol[node] = short(sym);\n"
    "        code += 1ul;\n"
    "    }\n"
    "    if (encoded_len == 0u) {\n"
    "        out_status[0] = 1;\n"
    "        return;\n"
    "    }\n"
    "    uchar last = encoded[encoded_len - 1];\n"
    "    if (last == 0u) {\n"
    "        out_status[0] = 1;\n"
    "        return;\n"
    "    }\n"
    "    uint highbit = 0u;\n"
    "    for (uint i = 0; i < 8; ++i) {\n"
    "        if ((last >> (i + 1)) == 0u) {\n"
    "            highbit = i;\n"
    "            break;\n"
    "        }\n"
    "    }\n"
    "    uint bit_len = (encoded_len - 1u) * 8u + highbit;\n"
    "    BitReaderRev reader;\n"
    "    bitreader_rev_init(reader, encoded, bit_len);\n"
    "    for (uint out_idx = 0; out_idx < out_len; ++out_idx) {\n"
    "        uint node = 0;\n"
    "        while (symbol[node] < 0) {\n"
    "            ulong bit = 0ul;\n"
    "            if (!bitreader_rev_read_bits(reader, 1u, bit)) {\n"
    "                out_status[0] = 1;\n"
    "                return;\n"
    "            }\n"
    "            int next = bit == 0ul ? child0[node] : child1[node];\n"
    "            if (next < 0) {\n"
    "                out_status[0] = 1;\n"
    "                return;\n"
    "            }\n"
    "            node = uint(next);\n"
    "        }\n"
    "        out[out_idx] = uchar(symbol[node]);\n"
    "    }\n"
    "}\n"
    "constexpr uint FSE_MAX_SYMBOLS = 256u;\n"
    "constexpr uint FSE_MAX_TABLE_LOG = 12u;\n"
    "constexpr uint FSE_MAX_TABLE_SIZE = 1u << FSE_MAX_TABLE_LOG;\n"
    "inline uint fse_highbit(uint v) {\n"
    "    uint hb = 0u;\n"
    "    while (v > 1u) {\n"
    "        v >>= 1u;\n"
    "        hb += 1u;\n"
    "    }\n"
    "    return hb;\n"
    "}\n"
    "kernel void gpuzstd_fse_encode(\n"
    "    device const ushort* symbols [[buffer(0)]],\n"
    "    device const short* normalized [[buffer(1)]],\n"
    "    device uchar* out_bytes [[buffer(2)]],\n"
    "    device uint* out_size [[buffer(3)]],\n"
    "    device uint* out_status [[buffer(4)]],\n"
    "    constant uint& symbols_len [[buffer(5)]],\n"
    "    constant uint& normalized_len [[buffer(6)]],\n"
    "    constant uint& max_symbol [[buffer(7)]],\n"
    "    constant uint& table_log [[buffer(8)]],\n"
    "    constant uint& out_capacity [[buffer(9)]],\n"
    "    uint gid [[thread_position_in_grid]]) {\n"
    "    if (gid != 0) return;\n"
    "    out_status[0] = 0;\n"
    "    out_size[0] = 0;\n"
    "    if (out_capacity < 4u) {\n"
    "        out_status[0] = 2;\n"
    "        return;\n"
    "    }\n"
    "    if (symbols_len == 0) {\n"
    "        out_bytes[0] = 0u;\n"
    "        out_bytes[1] = 0u;\n"
    "        out_bytes[2] = 0u;\n"
    "        out_bytes[3] = 0u;\n"
    "        out_size[0] = 4u;\n"
    "        return;\n"
    "    }\n"
    "    if (table_log > FSE_MAX_TABLE_LOG || max_symbol >= normalized_len) {\n"
    "        out_status[0] = 1;\n"
    "        return;\n"
    "    }\n"
    "    uint table_size = 1u << table_log;\n"
    "    if (table_size > FSE_MAX_TABLE_SIZE) {\n"
    "        out_status[0] = 1;\n"
    "        return;\n"
    "    }\n"
    "    uint table_mask = table_size - 1u;\n"
    "    uint step = (table_size >> 1) + (table_size >> 3) + 3u;\n"
    "    thread ushort table_symbol[FSE_MAX_TABLE_SIZE];\n"
    "    for (uint i = 0; i < table_size; ++i) table_symbol[i] = 0;\n"
    "    uint position = 0u;\n"
    "    uint total = 0u;\n"
    "    for (uint sym = 0; sym <= max_symbol; ++sym) {\n"
    "        int count = normalized[sym];\n"
    "        if (count <= 0) continue;\n"
    "        total += uint(count);\n"
    "        for (int i = 0; i < count; ++i) {\n"
    "            table_symbol[position] = ushort(sym);\n"
    "            position = (position + step) & table_mask;\n"
    "        }\n"
    "    }\n"
    "    if (position != 0u || total != table_size) {\n"
    "        out_status[0] = 1;\n"
    "        return;\n"
    "    }\n"
    "    thread uint cumul[FSE_MAX_SYMBOLS + 1];\n"
    "    cumul[0] = 0u;\n"
    "    for (uint sym = 0; sym <= max_symbol; ++sym) {\n"
    "        int count = normalized[sym];\n"
    "        cumul[sym + 1] = cumul[sym] + uint(max(count, 0));\n"
    "    }\n"
    "    thread ushort state_table[FSE_MAX_TABLE_SIZE];\n"
    "    thread uint cumul_pos[FSE_MAX_SYMBOLS + 1];\n"
    "    for (uint i = 0; i <= max_symbol + 1; ++i) {\n"
    "        cumul_pos[i] = cumul[i];\n"
    "    }\n"
    "    for (uint u = 0; u < table_size; ++u) {\n"
    "        uint sym = uint(table_symbol[u]);\n"
    "        uint idx = cumul_pos[sym]++;\n"
    "        state_table[idx] = ushort(table_size + u);\n"
    "    }\n"
    "    thread int delta_find_state[FSE_MAX_SYMBOLS];\n"
    "    thread uint delta_nb_bits[FSE_MAX_SYMBOLS];\n"
    "    uint total_states = 0u;\n"
    "    for (uint sym = 0; sym <= max_symbol; ++sym) {\n"
    "        int count = normalized[sym];\n"
    "        if (count <= 0) {\n"
    "            delta_nb_bits[sym] = ((table_log + 1u) << 16) - table_size;\n"
    "            delta_find_state[sym] = 0;\n"
    "            continue;\n"
    "        }\n"
    "        if (count == 1) {\n"
    "            delta_nb_bits[sym] = (table_log << 16) - table_size;\n"
    "            delta_find_state[sym] = int(total_states) - 1;\n"
    "            total_states += 1u;\n"
    "            continue;\n"
    "        }\n"
    "        uint max_bits_out = table_log - fse_highbit(uint(count - 1));\n"
    "        uint min_state_plus = uint(count) << max_bits_out;\n"
    "        delta_nb_bits[sym] = (max_bits_out << 16) - min_state_plus;\n"
    "        delta_find_state[sym] = int(total_states) - count;\n"
    "        total_states += uint(count);\n"
    "    }\n"
    "    BitWriter writer;\n"
    "    bitwriter_init(writer, out_bytes + 4, out_capacity - 4);\n"
    "    uint state = table_size;\n"
    "    uint total_bits = 0u;\n"
    "    for (uint idx = symbols_len; idx > 0; --idx) {\n"
    "        uint sym = uint(symbols[idx - 1]);\n"
    "        if (sym > max_symbol) {\n"
    "            out_status[0] = 1;\n"
    "            return;\n"
    "        }\n"
    "        uint nb_bits_out = (state + delta_nb_bits[sym]) >> 16;\n"
    "        uint next_total = total_bits + nb_bits_out;\n"
    "        if (next_total < total_bits) {\n"
    "            out_status[0] = 1;\n"
    "            return;\n"
    "        }\n"
    "        total_bits = next_total;\n"
    "        ulong mask = nb_bits_out == 0u ? 0ul : ((1ul << nb_bits_out) - 1ul);\n"
    "        if (!bitwriter_write_bits(writer, ulong(state) & mask, nb_bits_out)) {\n"
    "            out_status[0] = 2;\n"
    "            return;\n"
    "        }\n"
    "        int next = int(state >> nb_bits_out) + delta_find_state[sym];\n"
    "        if (next < 0 || uint(next) >= table_size) {\n"
    "            out_status[0] = 1;\n"
    "            return;\n"
    "        }\n"
    "        state = uint(state_table[uint(next)]);\n"
    "    }\n"
    "    ulong flush_mask = table_log == 0u ? 0ul : ((1ul << table_log) - 1ul);\n"
    "    if (total_bits + table_log < total_bits) {\n"
    "        out_status[0] = 1;\n"
    "        return;\n"
    "    }\n"
    "    total_bits += table_log;\n"
    "    if (!bitwriter_write_bits(writer, ulong(state) & flush_mask, table_log)) {\n"
    "        out_status[0] = 2;\n"
    "        return;\n"
    "    }\n"
    "    if (!bitwriter_flush_byte_aligned(writer)) {\n"
    "        out_status[0] = 2;\n"
    "        return;\n"
    "    }\n"
    "    out_bytes[0] = uchar(total_bits & 0xffu);\n"
    "    out_bytes[1] = uchar((total_bits >> 8) & 0xffu);\n"
    "    out_bytes[2] = uchar((total_bits >> 16) & 0xffu);\n"
    "    out_bytes[3] = uchar((total_bits >> 24) & 0xffu);\n"
    "    out_size[0] = writer.out_pos + 4u;\n"
    "}\n"
    "kernel void gpuzstd_fse_decode(\n"
    "    device const uchar* encoded [[buffer(0)]],\n"
    "    device const short* normalized [[buffer(1)]],\n"
    "    device ushort* out_symbols [[buffer(2)]],\n"
    "    device uint* out_status [[buffer(3)]],\n"
    "    constant uint& encoded_len [[buffer(4)]],\n"
    "    constant uint& normalized_len [[buffer(5)]],\n"
    "    constant uint& max_symbol [[buffer(6)]],\n"
    "    constant uint& table_log [[buffer(7)]],\n"
    "    constant uint& out_len [[buffer(8)]],\n"
    "    uint gid [[thread_position_in_grid]]) {\n"
    "    if (gid != 0) return;\n"
    "    out_status[0] = 0;\n"
    "    if (out_len == 0) return;\n"
    "    if (encoded_len < 4u) {\n"
    "        out_status[0] = 1;\n"
    "        return;\n"
    "    }\n"
    "    uint bit_len = uint(encoded[0]) | (uint(encoded[1]) << 8) | (uint(encoded[2]) << 16) | (uint(encoded[3]) << 24);\n"
    "    device const uchar* payload = encoded + 4;\n"
    "    uint payload_len = encoded_len - 4u;\n"
    "    if (bit_len > payload_len * 8u) {\n"
    "        out_status[0] = 1;\n"
    "        return;\n"
    "    }\n"
    "    if (table_log > FSE_MAX_TABLE_LOG || max_symbol >= normalized_len) {\n"
    "        out_status[0] = 1;\n"
    "        return;\n"
    "    }\n"
    "    uint table_size = 1u << table_log;\n"
    "    if (table_size > FSE_MAX_TABLE_SIZE) {\n"
    "        out_status[0] = 1;\n"
    "        return;\n"
    "    }\n"
    "    uint table_mask = table_size - 1u;\n"
    "    uint step = (table_size >> 1) + (table_size >> 3) + 3u;\n"
    "    thread ushort table_symbol[FSE_MAX_TABLE_SIZE];\n"
    "    for (uint i = 0; i < table_size; ++i) table_symbol[i] = 0;\n"
    "    uint position = 0u;\n"
    "    uint total = 0u;\n"
    "    for (uint sym = 0; sym <= max_symbol; ++sym) {\n"
    "        int count = normalized[sym];\n"
    "        if (count <= 0) continue;\n"
    "        total += uint(count);\n"
    "        for (int i = 0; i < count; ++i) {\n"
    "            table_symbol[position] = ushort(sym);\n"
    "            position = (position + step) & table_mask;\n"
    "        }\n"
    "    }\n"
    "    if (position != 0u || total != table_size) {\n"
    "        out_status[0] = 1;\n"
    "        return;\n"
    "    }\n"
    "    thread uint symbol_next[FSE_MAX_SYMBOLS];\n"
    "    for (uint sym = 0; sym <= max_symbol; ++sym) {\n"
    "        int count = normalized[sym];\n"
    "        symbol_next[sym] = uint(max(count, 0));\n"
    "    }\n"
    "    thread ushort decode_symbol[FSE_MAX_TABLE_SIZE];\n"
    "    thread uchar decode_nb_bits[FSE_MAX_TABLE_SIZE];\n"
    "    thread uint decode_new_state[FSE_MAX_TABLE_SIZE];\n"
    "    for (uint u = 0; u < table_size; ++u) {\n"
    "        uint sym = uint(table_symbol[u]);\n"
    "        uint next_state = symbol_next[sym]++;\n"
    "        uint nb_bits = table_log - fse_highbit(next_state);\n"
    "        uint new_state = (next_state << nb_bits) - table_size;\n"
    "        decode_symbol[u] = ushort(sym);\n"
    "        decode_nb_bits[u] = uchar(nb_bits);\n"
    "        decode_new_state[u] = new_state;\n"
    "    }\n"
    "    BitReaderRev reader;\n"
    "    bitreader_rev_init(reader, payload, bit_len);\n"
    "    ulong state = 0ul;\n"
    "    if (!bitreader_rev_read_bits(reader, table_log, state)) {\n"
    "        out_status[0] = 1;\n"
    "        return;\n"
    "    }\n"
    "    for (uint idx = 0; idx < out_len; ++idx) {\n"
    "        if (state >= table_size) {\n"
    "            out_status[0] = 1;\n"
    "            return;\n"
    "        }\n"
    "        uint st = uint(state);\n"
    "        ushort sym = decode_symbol[st];\n"
    "        out_symbols[idx] = sym;\n"
    "        uint nb_bits = decode_nb_bits[st];\n"
    "        ulong bits = 0ul;\n"
    "        if (!bitreader_rev_read_bits(reader, nb_bits, bits)) {\n"
    "            out_status[0] = 1;\n"
    "            return;\n"
    "        }\n"
    "        state = ulong(decode_new_state[st] + uint(bits));\n"
    "    }\n"
    "}\n"
    "constexpr uint HASH_SIZE = 4096;\n"
    "inline uint hash4(const device uchar* in, uint pos) {\n"
    "    uint v = uint(in[pos]) | (uint(in[pos + 1]) << 8) | (uint(in[pos + 2]) << 16) | (uint(in[pos + 3]) << 24);\n"
    "    v ^= v >> 16;\n"
    "    v *= 0x7feb352d;\n"
    "    v ^= v >> 15;\n"
    "    v *= 0x846ca68b;\n"
    "    v ^= v >> 16;\n"
    "    return v;\n"
    "}\n"
    "kernel void gpuzstd_count_sequences(\n"
    "    device const uchar* in [[buffer(0)]],\n"
    "    device uint* out_counts [[buffer(1)]],\n"
    "    constant GpuZstdParams& params [[buffer(2)]],\n"
    "    uint gid [[thread_position_in_grid]]) {\n"
    "    uint start = gid * params.chunk_size;\n"
    "    if (start >= params.len) {\n"
    "        return;\n"
    "    }\n"
    "    uint end = start + params.chunk_size;\n"
    "    if (end > params.len) end = params.len;\n"
    "    threadgroup int hash_table[HASH_SIZE];\n"
    "    for (uint i = 0; i < HASH_SIZE; ++i) {\n"
    "        hash_table[i] = -1;\n"
    "    }\n"
    "    uint seq_count = 0;\n"
    "    uint lit_start = start;\n"
    "    uint pos = start;\n"
    "    while (pos + params.min_match <= end) {\n"
    "        uint match_len = 0;\n"
    "        uint offset = 0;\n"
    "        if (pos + 4 <= end) {\n"
    "            uint h = hash4(in, pos) & (HASH_SIZE - 1);\n"
    "            int match_pos = hash_table[h];\n"
    "            hash_table[h] = int(pos);\n"
    "            if (match_pos >= int(start) && pos > uint(match_pos)) {\n"
    "                uint max_len = params.max_match;\n"
    "                uint max_from_pos = end - pos;\n"
    "                uint max_from_match = end - uint(match_pos);\n"
    "                if (max_len > max_from_pos) max_len = max_from_pos;\n"
    "                if (max_len > max_from_match) max_len = max_from_match;\n"
    "                while (match_len < max_len && in[uint(match_pos) + match_len] == in[pos + match_len]) {\n"
    "                    match_len++;\n"
    "                }\n"
    "                if (match_len >= params.min_match) {\n"
    "                    offset = pos - uint(match_pos);\n"
    "                } else {\n"
    "                    match_len = 0;\n"
    "                }\n"
    "            }\n"
    "        }\n"
    "        if (match_len >= params.min_match) {\n"
    "            seq_count++;\n"
    "            pos += match_len;\n"
    "            lit_start = pos;\n"
    "        } else {\n"
    "            pos += 1;\n"
    "        }\n"
    "    }\n"
    "    seq_count += 1;\n"
    "    out_counts[gid] = seq_count;\n"
    "}\n"
    "kernel void gpuzstd_write_sequences(\n"
    "    device const uchar* in [[buffer(0)]],\n"
    "    device const uint* offsets [[buffer(1)]],\n"
    "    device GpuZstdSequence* out_seqs [[buffer(2)]],\n"
    "    constant GpuZstdParams& params [[buffer(3)]],\n"
    "    uint gid [[thread_position_in_grid]]) {\n"
    "    uint start = gid * params.chunk_size;\n"
    "    if (start >= params.len) {\n"
    "        return;\n"
    "    }\n"
    "    uint end = start + params.chunk_size;\n"
    "    if (end > params.len) end = params.len;\n"
    "    threadgroup int hash_table[HASH_SIZE];\n"
    "    for (uint i = 0; i < HASH_SIZE; ++i) {\n"
    "        hash_table[i] = -1;\n"
    "    }\n"
    "    uint seq_idx = offsets[gid];\n"
    "    uint lit_start = start;\n"
    "    uint pos = start;\n"
    "    while (pos + params.min_match <= end) {\n"
    "        uint match_len = 0;\n"
    "        uint offset = 0;\n"
    "        if (pos + 4 <= end) {\n"
    "            uint h = hash4(in, pos) & (HASH_SIZE - 1);\n"
    "            int match_pos = hash_table[h];\n"
    "            hash_table[h] = int(pos);\n"
    "            if (match_pos >= int(start) && pos > uint(match_pos)) {\n"
    "                uint max_len = params.max_match;\n"
    "                uint max_from_pos = end - pos;\n"
    "                uint max_from_match = end - uint(match_pos);\n"
    "                if (max_len > max_from_pos) max_len = max_from_pos;\n"
    "                if (max_len > max_from_match) max_len = max_from_match;\n"
    "                while (match_len < max_len && in[uint(match_pos) + match_len] == in[pos + match_len]) {\n"
    "                    match_len++;\n"
    "                }\n"
    "                if (match_len >= params.min_match) {\n"
    "                    offset = pos - uint(match_pos);\n"
    "                } else {\n"
    "                    match_len = 0;\n"
    "                }\n"
    "            }\n"
    "        }\n"
    "        if (match_len >= params.min_match) {\n"
    "            GpuZstdSequence seq;\n"
    "            seq.lit_len = pos - lit_start;\n"
    "            seq.match_len = match_len;\n"
    "            seq.offset = offset;\n"
    "            seq.reserved = 0;\n"
    "            out_seqs[seq_idx++] = seq;\n"
    "            pos += match_len;\n"
    "            lit_start = pos;\n"
    "        } else {\n"
    "            pos += 1;\n"
    "        }\n"
    "    }\n"
    "    GpuZstdSequence tail;\n"
    "    tail.lit_len = end - lit_start;\n"
    "    tail.match_len = 0;\n"
    "    tail.offset = 0;\n"
    "    tail.reserved = 0;\n"
    "    out_seqs[seq_idx] = tail;\n"
    "}\n";

static BOOL build_pipelines(id<MTLDevice> device,
                            id<MTLComputePipelineState> *count_pipeline,
                            id<MTLComputePipelineState> *write_pipeline) {
    NSError *error = nil;
    id<MTLLibrary> library = [device newLibraryWithSource:kKernelSource options:nil error:&error];
    if (!library) {
        return NO;
    }
    id<MTLFunction> count_fn = [library newFunctionWithName:@"gpuzstd_count_sequences"];
    id<MTLFunction> write_fn = [library newFunctionWithName:@"gpuzstd_write_sequences"];
    if (!count_fn || !write_fn) {
        return NO;
    }
    *count_pipeline = [device newComputePipelineStateWithFunction:count_fn error:&error];
    if (!*count_pipeline) {
        return NO;
    }
    *write_pipeline = [device newComputePipelineStateWithFunction:write_fn error:&error];
    if (!*write_pipeline) {
        return NO;
    }
    return YES;
}

static BOOL build_huffman_pipelines(id<MTLDevice> device,
                                    id<MTLComputePipelineState> *encode_pipeline,
                                    id<MTLComputePipelineState> *decode_pipeline) {
    NSError *error = nil;
    id<MTLLibrary> library = [device newLibraryWithSource:kKernelSource options:nil error:&error];
    if (!library) {
        return NO;
    }
    id<MTLFunction> encode_fn = [library newFunctionWithName:@"gpuzstd_huff_encode"];
    id<MTLFunction> decode_fn = [library newFunctionWithName:@"gpuzstd_huff_decode"];
    if (!encode_fn || !decode_fn) {
        return NO;
    }
    *encode_pipeline = [device newComputePipelineStateWithFunction:encode_fn error:&error];
    if (!*encode_pipeline) {
        return NO;
    }
    *decode_pipeline = [device newComputePipelineStateWithFunction:decode_fn error:&error];
    if (!*decode_pipeline) {
        return NO;
    }
    return YES;
}

static BOOL build_fse_pipelines(id<MTLDevice> device,
                                id<MTLComputePipelineState> *encode_pipeline,
                                id<MTLComputePipelineState> *decode_pipeline) {
    NSError *error = nil;
    id<MTLLibrary> library = [device newLibraryWithSource:kKernelSource options:nil error:&error];
    if (!library) {
        return NO;
    }
    id<MTLFunction> encode_fn = [library newFunctionWithName:@"gpuzstd_fse_encode"];
    id<MTLFunction> decode_fn = [library newFunctionWithName:@"gpuzstd_fse_decode"];
    if (!encode_fn || !decode_fn) {
        return NO;
    }
    *encode_pipeline = [device newComputePipelineStateWithFunction:encode_fn error:&error];
    if (!*encode_pipeline) {
        return NO;
    }
    *decode_pipeline = [device newComputePipelineStateWithFunction:decode_fn error:&error];
    if (!*decode_pipeline) {
        return NO;
    }
    return YES;
}

__attribute__((visibility("default")))
int gpuzstd_metal_count_sequences(const uint8_t *input,
                                  size_t len,
                                  uint32_t chunk_size,
                                  uint32_t min_match,
                                  uint32_t max_match,
                                  uint32_t *out_counts,
                                  uint32_t counts_len) {
    if (input == NULL || out_counts == NULL) {
        return RC_INVALID;
    }
    if (len > UINT32_MAX) {
        return RC_INVALID;
    }
    if (len == 0) {
        if (counts_len > 0) {
            memset(out_counts, 0, counts_len * sizeof(uint32_t));
        }
        return RC_OK;
    }
    if (chunk_size == 0) {
        return RC_INVALID;
    }
    @autoreleasepool {
        id<MTLDevice> device = MTLCreateSystemDefaultDevice();
        if (!device) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLComputePipelineState> count_pipeline = nil;
        id<MTLComputePipelineState> write_pipeline = nil;
        if (!build_pipelines(device, &count_pipeline, &write_pipeline)) {
            return RC_GPU_UNAVAILABLE;
        }
        uint64_t chunk_count64 = (len + chunk_size - 1) / chunk_size;
        if (chunk_count64 > counts_len) {
            return RC_NO_SPACE;
        }
        uint32_t chunk_count = (uint32_t)chunk_count64;
        GpuZstdParams params = {
            .len = (uint32_t)len,
            .chunk_size = chunk_size,
            .min_match = min_match,
            .max_match = max_match,
        };
        id<MTLBuffer> input_buf =
            [device newBufferWithBytes:input length:len options:MTLResourceStorageModeShared];
        id<MTLBuffer> out_buf =
            [device newBufferWithLength:chunk_count * sizeof(uint32_t)
                                options:MTLResourceStorageModeShared];
        id<MTLBuffer> params_buf =
            [device newBufferWithBytes:&params length:sizeof(params) options:MTLResourceStorageModeShared];
        if (!input_buf || !out_buf || !params_buf) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLCommandQueue> queue = [device newCommandQueue];
        if (!queue) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLCommandBuffer> command = [queue commandBuffer];
        if (!command) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLComputeCommandEncoder> encoder = [command computeCommandEncoder];
        if (!encoder) {
            return RC_GPU_UNAVAILABLE;
        }
        [encoder setComputePipelineState:count_pipeline];
        [encoder setBuffer:input_buf offset:0 atIndex:0];
        [encoder setBuffer:out_buf offset:0 atIndex:1];
        [encoder setBuffer:params_buf offset:0 atIndex:2];
        MTLSize grid = MTLSizeMake(chunk_count, 1, 1);
        MTLSize threads_per_group = MTLSizeMake(1, 1, 1);
        [encoder dispatchThreads:grid threadsPerThreadgroup:threads_per_group];
        [encoder endEncoding];
        [command commit];
        [command waitUntilCompleted];
        if (command.error != nil) {
            return RC_GPU_UNAVAILABLE;
        }
        memcpy(out_counts, out_buf.contents, chunk_count * sizeof(uint32_t));
        return RC_OK;
    }
}

__attribute__((visibility("default")))
int gpuzstd_metal_write_sequences(const uint8_t *input,
                                  size_t len,
                                  uint32_t chunk_size,
                                  uint32_t min_match,
                                  uint32_t max_match,
                                  const uint32_t *offsets,
                                  uint32_t offsets_len,
                                  GpuZstdSequence *out_seqs,
                                  uint32_t seq_capacity) {
    if (input == NULL || out_seqs == NULL || offsets == NULL) {
        return RC_INVALID;
    }
    if (len > UINT32_MAX) {
        return RC_INVALID;
    }
    if (len == 0) {
        return RC_OK;
    }
    if (chunk_size == 0) {
        return RC_INVALID;
    }
    @autoreleasepool {
        id<MTLDevice> device = MTLCreateSystemDefaultDevice();
        if (!device) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLComputePipelineState> count_pipeline = nil;
        id<MTLComputePipelineState> write_pipeline = nil;
        if (!build_pipelines(device, &count_pipeline, &write_pipeline)) {
            return RC_GPU_UNAVAILABLE;
        }
        uint64_t chunk_count64 = (len + chunk_size - 1) / chunk_size;
        if (chunk_count64 > offsets_len) {
            return RC_INVALID;
        }
        uint32_t chunk_count = (uint32_t)chunk_count64;
        GpuZstdParams params = {
            .len = (uint32_t)len,
            .chunk_size = chunk_size,
            .min_match = min_match,
            .max_match = max_match,
        };
        id<MTLBuffer> input_buf =
            [device newBufferWithBytes:input length:len options:MTLResourceStorageModeShared];
        id<MTLBuffer> offsets_buf =
            [device newBufferWithBytes:offsets
                                 length:offsets_len * sizeof(uint32_t)
                                options:MTLResourceStorageModeShared];
        id<MTLBuffer> out_buf =
            [device newBufferWithLength:seq_capacity * sizeof(GpuZstdSequence)
                                options:MTLResourceStorageModeShared];
        id<MTLBuffer> params_buf =
            [device newBufferWithBytes:&params length:sizeof(params) options:MTLResourceStorageModeShared];
        if (!input_buf || !offsets_buf || !out_buf || !params_buf) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLCommandQueue> queue = [device newCommandQueue];
        if (!queue) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLCommandBuffer> command = [queue commandBuffer];
        if (!command) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLComputeCommandEncoder> encoder = [command computeCommandEncoder];
        if (!encoder) {
            return RC_GPU_UNAVAILABLE;
        }
        [encoder setComputePipelineState:write_pipeline];
        [encoder setBuffer:input_buf offset:0 atIndex:0];
        [encoder setBuffer:offsets_buf offset:0 atIndex:1];
        [encoder setBuffer:out_buf offset:0 atIndex:2];
        [encoder setBuffer:params_buf offset:0 atIndex:3];
        MTLSize grid = MTLSizeMake(chunk_count, 1, 1);
        MTLSize threads_per_group = MTLSizeMake(1, 1, 1);
        [encoder dispatchThreads:grid threadsPerThreadgroup:threads_per_group];
        [encoder endEncoding];
        [command commit];
        [command waitUntilCompleted];
        if (command.error != nil) {
            return RC_GPU_UNAVAILABLE;
        }
        memcpy(out_seqs, out_buf.contents, seq_capacity * sizeof(GpuZstdSequence));
        return RC_OK;
    }
}

__attribute__((visibility("default")))
int gpuzstd_metal_huff_encode(const uint8_t *input,
                              size_t len,
                              uint8_t *out_bytes,
                              size_t out_capacity,
                              size_t *out_len,
                              uint8_t *out_lengths,
                              size_t lengths_len) {
    if (!input || !out_bytes || !out_len || !out_lengths) {
        return RC_INVALID;
    }
    if (lengths_len < 256) {
        return RC_INVALID;
    }
    if (len > UINT32_MAX || out_capacity > UINT32_MAX) {
        return RC_INVALID;
    }
    @autoreleasepool {
        id<MTLDevice> device = MTLCreateSystemDefaultDevice();
        if (!device) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLComputePipelineState> encode_pipeline = nil;
        id<MTLComputePipelineState> decode_pipeline = nil;
        if (!build_huffman_pipelines(device, &encode_pipeline, &decode_pipeline)) {
            return RC_GPU_UNAVAILABLE;
        }
        uint32_t in_len32 = (uint32_t)len;
        uint32_t out_cap32 = (uint32_t)out_capacity;
        id<MTLBuffer> in_buf =
            [device newBufferWithBytes:input length:len options:MTLResourceStorageModeShared];
        id<MTLBuffer> out_buf =
            [device newBufferWithLength:out_capacity options:MTLResourceStorageModeShared];
        id<MTLBuffer> lengths_buf =
            [device newBufferWithLength:256 options:MTLResourceStorageModeShared];
        id<MTLBuffer> out_size_buf =
            [device newBufferWithLength:sizeof(uint32_t) options:MTLResourceStorageModeShared];
        id<MTLBuffer> status_buf =
            [device newBufferWithLength:sizeof(uint32_t) options:MTLResourceStorageModeShared];
        id<MTLBuffer> in_len_buf =
            [device newBufferWithBytes:&in_len32 length:sizeof(in_len32) options:MTLResourceStorageModeShared];
        id<MTLBuffer> out_cap_buf =
            [device newBufferWithBytes:&out_cap32 length:sizeof(out_cap32) options:MTLResourceStorageModeShared];
        if (!in_buf || !out_buf || !lengths_buf || !out_size_buf || !status_buf || !in_len_buf ||
            !out_cap_buf) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLCommandQueue> queue = [device newCommandQueue];
        if (!queue) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLCommandBuffer> command = [queue commandBuffer];
        if (!command) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLComputeCommandEncoder> encoder = [command computeCommandEncoder];
        if (!encoder) {
            return RC_GPU_UNAVAILABLE;
        }
        [encoder setComputePipelineState:encode_pipeline];
        [encoder setBuffer:in_buf offset:0 atIndex:0];
        [encoder setBuffer:out_buf offset:0 atIndex:1];
        [encoder setBuffer:lengths_buf offset:0 atIndex:2];
        [encoder setBuffer:out_size_buf offset:0 atIndex:3];
        [encoder setBuffer:status_buf offset:0 atIndex:4];
        [encoder setBuffer:in_len_buf offset:0 atIndex:5];
        [encoder setBuffer:out_cap_buf offset:0 atIndex:6];
        MTLSize grid = MTLSizeMake(1, 1, 1);
        MTLSize threads_per_group = MTLSizeMake(1, 1, 1);
        [encoder dispatchThreads:grid threadsPerThreadgroup:threads_per_group];
        [encoder endEncoding];
        [command commit];
        [command waitUntilCompleted];
        if (command.error != nil) {
            return RC_GPU_UNAVAILABLE;
        }
        uint32_t status = *(uint32_t *)status_buf.contents;
        uint32_t out_size = *(uint32_t *)out_size_buf.contents;
        if (status != 0) {
            return status == 2 ? RC_NO_SPACE : RC_INVALID;
        }
        if (out_size > out_capacity) {
            return RC_NO_SPACE;
        }
        memcpy(out_bytes, out_buf.contents, out_size);
        memcpy(out_lengths, lengths_buf.contents, 256);
        *out_len = out_size;
    }
    return RC_OK;
}

__attribute__((visibility("default")))
int gpuzstd_metal_huff_decode(const uint8_t *encoded,
                              size_t encoded_len,
                              const uint8_t *lengths,
                              size_t lengths_len,
                              uint8_t *out_bytes,
                              size_t out_len) {
    if (!encoded || !lengths || !out_bytes) {
        return RC_INVALID;
    }
    if (lengths_len < 256) {
        return RC_INVALID;
    }
    if (encoded_len > UINT32_MAX || out_len > UINT32_MAX) {
        return RC_INVALID;
    }
    @autoreleasepool {
        id<MTLDevice> device = MTLCreateSystemDefaultDevice();
        if (!device) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLComputePipelineState> encode_pipeline = nil;
        id<MTLComputePipelineState> decode_pipeline = nil;
        if (!build_huffman_pipelines(device, &encode_pipeline, &decode_pipeline)) {
            return RC_GPU_UNAVAILABLE;
        }
        uint32_t encoded_len32 = (uint32_t)encoded_len;
        uint32_t out_len32 = (uint32_t)out_len;
        id<MTLBuffer> encoded_buf =
            [device newBufferWithBytes:encoded
                                 length:encoded_len
                                options:MTLResourceStorageModeShared];
        id<MTLBuffer> lengths_buf =
            [device newBufferWithBytes:lengths length:256 options:MTLResourceStorageModeShared];
        id<MTLBuffer> out_buf =
            [device newBufferWithLength:out_len options:MTLResourceStorageModeShared];
        id<MTLBuffer> status_buf =
            [device newBufferWithLength:sizeof(uint32_t) options:MTLResourceStorageModeShared];
        id<MTLBuffer> encoded_len_buf =
            [device newBufferWithBytes:&encoded_len32
                                 length:sizeof(encoded_len32)
                                options:MTLResourceStorageModeShared];
        id<MTLBuffer> out_len_buf =
            [device newBufferWithBytes:&out_len32 length:sizeof(out_len32) options:MTLResourceStorageModeShared];
        if (!encoded_buf || !lengths_buf || !out_buf || !status_buf || !encoded_len_buf || !out_len_buf) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLCommandQueue> queue = [device newCommandQueue];
        if (!queue) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLCommandBuffer> command = [queue commandBuffer];
        if (!command) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLComputeCommandEncoder> encoder = [command computeCommandEncoder];
        if (!encoder) {
            return RC_GPU_UNAVAILABLE;
        }
        [encoder setComputePipelineState:decode_pipeline];
        [encoder setBuffer:encoded_buf offset:0 atIndex:0];
        [encoder setBuffer:lengths_buf offset:0 atIndex:1];
        [encoder setBuffer:out_buf offset:0 atIndex:2];
        [encoder setBuffer:status_buf offset:0 atIndex:3];
        [encoder setBuffer:encoded_len_buf offset:0 atIndex:4];
        [encoder setBuffer:out_len_buf offset:0 atIndex:5];
        MTLSize grid = MTLSizeMake(1, 1, 1);
        MTLSize threads_per_group = MTLSizeMake(1, 1, 1);
        [encoder dispatchThreads:grid threadsPerThreadgroup:threads_per_group];
        [encoder endEncoding];
        [command commit];
        [command waitUntilCompleted];
        if (command.error != nil) {
            return RC_GPU_UNAVAILABLE;
        }
        uint32_t status = *(uint32_t *)status_buf.contents;
        if (status != 0) {
            return RC_INVALID;
        }
        memcpy(out_bytes, out_buf.contents, out_len);
    }
    return RC_OK;
}

__attribute__((visibility("default")))
int gpuzstd_metal_fse_encode(const uint16_t *symbols,
                             size_t symbols_len,
                             const int16_t *normalized,
                             size_t normalized_len,
                             uint32_t max_symbol,
                             uint32_t table_log,
                             uint8_t *out_bytes,
                             size_t out_capacity,
                             size_t *out_len) {
    if (!symbols || !normalized || !out_bytes || !out_len) {
        return RC_INVALID;
    }
    if (symbols_len > UINT32_MAX || normalized_len > UINT32_MAX || out_capacity > UINT32_MAX) {
        return RC_INVALID;
    }
    @autoreleasepool {
        id<MTLDevice> device = MTLCreateSystemDefaultDevice();
        if (!device) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLComputePipelineState> encode_pipeline = nil;
        id<MTLComputePipelineState> decode_pipeline = nil;
        if (!build_fse_pipelines(device, &encode_pipeline, &decode_pipeline)) {
            return RC_GPU_UNAVAILABLE;
        }
        uint32_t symbols_len32 = (uint32_t)symbols_len;
        uint32_t normalized_len32 = (uint32_t)normalized_len;
        uint32_t out_cap32 = (uint32_t)out_capacity;
        id<MTLBuffer> symbols_buf =
            [device newBufferWithBytes:symbols
                                 length:symbols_len * sizeof(uint16_t)
                                options:MTLResourceStorageModeShared];
        id<MTLBuffer> normalized_buf =
            [device newBufferWithBytes:normalized
                                 length:normalized_len * sizeof(int16_t)
                                options:MTLResourceStorageModeShared];
        id<MTLBuffer> out_buf =
            [device newBufferWithLength:out_capacity options:MTLResourceStorageModeShared];
        id<MTLBuffer> out_size_buf =
            [device newBufferWithLength:sizeof(uint32_t) options:MTLResourceStorageModeShared];
        id<MTLBuffer> status_buf =
            [device newBufferWithLength:sizeof(uint32_t) options:MTLResourceStorageModeShared];
        id<MTLBuffer> symbols_len_buf =
            [device newBufferWithBytes:&symbols_len32
                                 length:sizeof(symbols_len32)
                                options:MTLResourceStorageModeShared];
        id<MTLBuffer> normalized_len_buf =
            [device newBufferWithBytes:&normalized_len32
                                 length:sizeof(normalized_len32)
                                options:MTLResourceStorageModeShared];
        id<MTLBuffer> max_symbol_buf =
            [device newBufferWithBytes:&max_symbol length:sizeof(max_symbol) options:MTLResourceStorageModeShared];
        id<MTLBuffer> table_log_buf =
            [device newBufferWithBytes:&table_log length:sizeof(table_log) options:MTLResourceStorageModeShared];
        id<MTLBuffer> out_cap_buf =
            [device newBufferWithBytes:&out_cap32 length:sizeof(out_cap32) options:MTLResourceStorageModeShared];
        if (!symbols_buf || !normalized_buf || !out_buf || !out_size_buf || !status_buf ||
            !symbols_len_buf || !normalized_len_buf || !max_symbol_buf || !table_log_buf ||
            !out_cap_buf) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLCommandQueue> queue = [device newCommandQueue];
        if (!queue) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLCommandBuffer> command = [queue commandBuffer];
        if (!command) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLComputeCommandEncoder> encoder = [command computeCommandEncoder];
        if (!encoder) {
            return RC_GPU_UNAVAILABLE;
        }
        [encoder setComputePipelineState:encode_pipeline];
        [encoder setBuffer:symbols_buf offset:0 atIndex:0];
        [encoder setBuffer:normalized_buf offset:0 atIndex:1];
        [encoder setBuffer:out_buf offset:0 atIndex:2];
        [encoder setBuffer:out_size_buf offset:0 atIndex:3];
        [encoder setBuffer:status_buf offset:0 atIndex:4];
        [encoder setBuffer:symbols_len_buf offset:0 atIndex:5];
        [encoder setBuffer:normalized_len_buf offset:0 atIndex:6];
        [encoder setBuffer:max_symbol_buf offset:0 atIndex:7];
        [encoder setBuffer:table_log_buf offset:0 atIndex:8];
        [encoder setBuffer:out_cap_buf offset:0 atIndex:9];
        MTLSize grid = MTLSizeMake(1, 1, 1);
        MTLSize threads_per_group = MTLSizeMake(1, 1, 1);
        [encoder dispatchThreads:grid threadsPerThreadgroup:threads_per_group];
        [encoder endEncoding];
        [command commit];
        [command waitUntilCompleted];
        if (command.error != nil) {
            return RC_GPU_UNAVAILABLE;
        }
        uint32_t status = *(uint32_t *)status_buf.contents;
        uint32_t out_size = *(uint32_t *)out_size_buf.contents;
        if (status != 0) {
            return status == 2 ? RC_NO_SPACE : RC_INVALID;
        }
        if (out_size > out_capacity) {
            return RC_NO_SPACE;
        }
        memcpy(out_bytes, out_buf.contents, out_size);
        *out_len = out_size;
    }
    return RC_OK;
}

__attribute__((visibility("default")))
int gpuzstd_metal_fse_decode(const uint8_t *encoded,
                             size_t encoded_len,
                             const int16_t *normalized,
                             size_t normalized_len,
                             uint32_t max_symbol,
                             uint32_t table_log,
                             uint16_t *out_symbols,
                             size_t out_len) {
    if (!encoded || !normalized || !out_symbols) {
        return RC_INVALID;
    }
    if (encoded_len > UINT32_MAX || normalized_len > UINT32_MAX || out_len > UINT32_MAX) {
        return RC_INVALID;
    }
    @autoreleasepool {
        id<MTLDevice> device = MTLCreateSystemDefaultDevice();
        if (!device) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLComputePipelineState> encode_pipeline = nil;
        id<MTLComputePipelineState> decode_pipeline = nil;
        if (!build_fse_pipelines(device, &encode_pipeline, &decode_pipeline)) {
            return RC_GPU_UNAVAILABLE;
        }
        uint32_t encoded_len32 = (uint32_t)encoded_len;
        uint32_t normalized_len32 = (uint32_t)normalized_len;
        uint32_t out_len32 = (uint32_t)out_len;
        id<MTLBuffer> encoded_buf =
            [device newBufferWithBytes:encoded
                                 length:encoded_len
                                options:MTLResourceStorageModeShared];
        id<MTLBuffer> normalized_buf =
            [device newBufferWithBytes:normalized
                                 length:normalized_len * sizeof(int16_t)
                                options:MTLResourceStorageModeShared];
        id<MTLBuffer> out_buf =
            [device newBufferWithLength:out_len * sizeof(uint16_t)
                                options:MTLResourceStorageModeShared];
        id<MTLBuffer> status_buf =
            [device newBufferWithLength:sizeof(uint32_t) options:MTLResourceStorageModeShared];
        id<MTLBuffer> encoded_len_buf =
            [device newBufferWithBytes:&encoded_len32
                                 length:sizeof(encoded_len32)
                                options:MTLResourceStorageModeShared];
        id<MTLBuffer> normalized_len_buf =
            [device newBufferWithBytes:&normalized_len32
                                 length:sizeof(normalized_len32)
                                options:MTLResourceStorageModeShared];
        id<MTLBuffer> max_symbol_buf =
            [device newBufferWithBytes:&max_symbol length:sizeof(max_symbol) options:MTLResourceStorageModeShared];
        id<MTLBuffer> table_log_buf =
            [device newBufferWithBytes:&table_log length:sizeof(table_log) options:MTLResourceStorageModeShared];
        id<MTLBuffer> out_len_buf =
            [device newBufferWithBytes:&out_len32 length:sizeof(out_len32) options:MTLResourceStorageModeShared];
        if (!encoded_buf || !normalized_buf || !out_buf || !status_buf || !encoded_len_buf ||
            !normalized_len_buf || !max_symbol_buf || !table_log_buf || !out_len_buf) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLCommandQueue> queue = [device newCommandQueue];
        if (!queue) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLCommandBuffer> command = [queue commandBuffer];
        if (!command) {
            return RC_GPU_UNAVAILABLE;
        }
        id<MTLComputeCommandEncoder> encoder = [command computeCommandEncoder];
        if (!encoder) {
            return RC_GPU_UNAVAILABLE;
        }
        [encoder setComputePipelineState:decode_pipeline];
        [encoder setBuffer:encoded_buf offset:0 atIndex:0];
        [encoder setBuffer:normalized_buf offset:0 atIndex:1];
        [encoder setBuffer:out_buf offset:0 atIndex:2];
        [encoder setBuffer:status_buf offset:0 atIndex:3];
        [encoder setBuffer:encoded_len_buf offset:0 atIndex:4];
        [encoder setBuffer:normalized_len_buf offset:0 atIndex:5];
        [encoder setBuffer:max_symbol_buf offset:0 atIndex:6];
        [encoder setBuffer:table_log_buf offset:0 atIndex:7];
        [encoder setBuffer:out_len_buf offset:0 atIndex:8];
        MTLSize grid = MTLSizeMake(1, 1, 1);
        MTLSize threads_per_group = MTLSizeMake(1, 1, 1);
        [encoder dispatchThreads:grid threadsPerThreadgroup:threads_per_group];
        [encoder endEncoding];
        [command commit];
        [command waitUntilCompleted];
        if (command.error != nil) {
            return RC_GPU_UNAVAILABLE;
        }
        uint32_t status = *(uint32_t *)status_buf.contents;
        if (status != 0) {
            return RC_INVALID;
        }
        memcpy(out_symbols, out_buf.contents, out_len * sizeof(uint16_t));
    }
    return RC_OK;
}
