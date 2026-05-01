#import <Metal/Metal.h>
#import <Foundation/Foundation.h>
#include <stdint.h>
#include <stddef.h>

static NSString* kMSLSource = @"\n"
"using namespace metal;\n"
"constant ulong CRC64_POLY = 0xC96C5795D7870F42ul;\n"
"constant ulong CRC64_CHUNK_SIZE = 16384ul;\n"
"kernel void classify_json(\n"
"    device const uchar* in  [[buffer(0)]],\n"
"    device uint* outS       [[buffer(1)]],\n"
"    device uint* outQ       [[buffer(2)]],\n"
"    device uint* outB       [[buffer(3)]],\n"
"    constant uint& n        [[buffer(4)]],\n"
"    uint gid [[thread_position_in_grid]]) {\n"
"    uint base = gid * 32u;\n"
"    if (base >= n) return;\n"
"    uint mS=0, mQ=0, mB=0;\n"
"    #pragma unroll\n"
"    for (uint i = 0; i < 32 && base + i < n; ++i) {\n"
"        uchar c = in[base + i];\n"
"        mQ |= uint(c == 34) << i;\n"
"        mB |= uint(c == 92) << i;\n"
"        bool s = (c=='{'||c=='}'||c=='['||c==']'||c==':'||c==',');\n"
"        mS |= uint(s) << i;\n"
"    }\n"
"    outS[gid] = mS; outQ[gid] = mQ; outB[gid] = mB;\n"
"}\n"
"\n"
"kernel void crc64_chunks(\n"
"    device const uchar* in [[buffer(0)]],\n"
"    constant ulong& n      [[buffer(1)]],\n"
"    device ulong* out      [[buffer(2)]],\n"
"    uint gid [[thread_position_in_grid]]) {\n"
"    ulong start = (ulong)gid * CRC64_CHUNK_SIZE;\n"
"    if (start >= n) { return; }\n"
"    ulong end = (n - start < CRC64_CHUNK_SIZE) ? n : (start + CRC64_CHUNK_SIZE);\n"
"    ulong crc = 0ul;\n"
"    for (ulong i = start; i < end; ++i) {\n"
"        crc ^= (ulong)in[i];\n"
"        for (uint bit = 0u; bit < 8u; ++bit) {\n"
"            if ((crc & 1ul) != 0ul) {\n"
"                crc = (crc >> 1) ^ CRC64_POLY;\n"
"            } else {\n"
"                crc >>= 1;\n"
"            }\n"
"        }\n"
"    }\n"
"    out[gid] = crc;\n"
"}\n"
"\n"
"struct SequenceSpan { ulong start; ulong end; };\n"
"constant uchar FLAG_COMPACT_LEN = 0x02;\n"
"bool read_u64_le(device const uchar* in, ulong n, ulong offset, thread ulong& out) {\n"
"    if (offset > n || n - offset < 8ul) { return false; }\n"
"    ulong value = 0ul;\n"
"    for (uint i = 0u; i < 8u; ++i) { value |= ((ulong)in[offset + i]) << (8u * i); }\n"
"    out = value;\n"
"    return true;\n"
"}\n"
"ulong varint_len(ulong value) {\n"
"    ulong len = 1ul;\n"
"    while (value >= 0x80ul) { value >>= 7u; len += 1ul; }\n"
"    return len;\n"
"}\n"
"bool read_value_len(device const uchar* in, ulong n, ulong offset, uchar flags, thread ulong& len, thread ulong& headerLen) {\n"
"    if ((flags & FLAG_COMPACT_LEN) == 0) {\n"
"        if (!read_u64_le(in, n, offset, len)) { return false; }\n"
"        headerLen = 8ul;\n"
"        return true;\n"
"    }\n"
"    ulong result = 0ul;\n"
"    uint shift = 0u;\n"
"    for (ulong idx = 0ul; idx < 10ul; ++idx) {\n"
"        if (offset + idx >= n) { return false; }\n"
"        uchar byte = in[offset + idx];\n"
"        ulong payload = (ulong)(byte & 0x7f);\n"
"        if (shift == 63u && payload > 1ul) { return false; }\n"
"        result |= payload << shift;\n"
"        if ((byte & 0x80) == 0) {\n"
"            ulong used = idx + 1ul;\n"
"            if (used != varint_len(result)) { return false; }\n"
"            len = result;\n"
"            headerLen = used;\n"
"            return true;\n"
"        }\n"
"        shift += 7u;\n"
"    }\n"
"    return false;\n"
"}\n"
"kernel void sequence_fixed_offsets(\n"
"    device const uchar* in [[buffer(0)]],\n"
"    constant ulong& n [[buffer(1)]],\n"
"    constant ulong& count [[buffer(2)]],\n"
"    constant ulong& dataStart [[buffer(3)]],\n"
"    constant ulong& dataLen [[buffer(4)]],\n"
"    device SequenceSpan* spans [[buffer(5)]],\n"
"    device atomic_uint* status [[buffer(6)]],\n"
"    uint gid [[thread_position_in_grid]]) {\n"
"    ulong idx = (ulong)gid;\n"
"    if (idx >= count || atomic_load_explicit(status, memory_order_relaxed) != 0u) { return; }\n"
"    ulong prev = 0ul;\n"
"    ulong next = 0ul;\n"
"    ulong table = 8ul;\n"
"    if (!read_u64_le(in, n, table + idx * 8ul, prev) || !read_u64_le(in, n, table + (idx + 1ul) * 8ul, next) || next < prev || next > dataLen) {\n"
"        atomic_store_explicit(status, 1u, memory_order_relaxed);\n"
"        return;\n"
"    }\n"
"    spans[idx].start = dataStart + prev;\n"
"    spans[idx].end = dataStart + next;\n"
"}\n"
"\n"
"kernel void sequence_length_prefixed(\n"
"    device const uchar* in [[buffer(0)]],\n"
"    constant ulong& n [[buffer(1)]],\n"
"    constant ulong& count [[buffer(2)]],\n"
"    constant uchar& flags [[buffer(3)]],\n"
"    device SequenceSpan* spans [[buffer(4)]],\n"
"    device ulong* used [[buffer(5)]],\n"
"    device atomic_uint* status [[buffer(6)]],\n"
"    uint gid [[thread_position_in_grid]]) {\n"
"    if (gid != 0u) { return; }\n"
"    ulong offset = 8ul;\n"
"    for (ulong idx = 0ul; idx < count; ++idx) {\n"
"        ulong elemLen = 0ul;\n"
"        ulong headerLen = 0ul;\n"
"        if (!read_value_len(in, n, offset, flags, elemLen, headerLen)) { atomic_store_explicit(status, 1u, memory_order_relaxed); return; }\n"
"        if (headerLen > n - offset) { atomic_store_explicit(status, 1u, memory_order_relaxed); return; }\n"
"        ulong start = offset + headerLen;\n"
"        if (start > n || elemLen > n - start) { atomic_store_explicit(status, 1u, memory_order_relaxed); return; }\n"
"        ulong end = start + elemLen;\n"
"        spans[idx].start = start;\n"
"        spans[idx].end = end;\n"
"        offset = end;\n"
"    }\n"
"    *used = offset;\n"
"}\n";

static const uint64_t CRC64_POLY = 0xC96C5795D7870F42ULL;
static const uint64_t CRC64_INIT = 0xFFFFFFFFFFFFFFFFULL;
static const uint64_t CRC64_XOR_OUT = 0xFFFFFFFFFFFFFFFFULL;
static const size_t CRC64_CHUNK_SIZE = 16ULL * 1024ULL;
static const uint32_t LAYOUT_LENGTH_PREFIXED_HOST = 0U;
static const uint32_t LAYOUT_FIXED_OFFSETS_HOST = 1U;

typedef struct {
    size_t start;
    size_t end;
} NoritoSequenceSpanMetal;

static BOOL read_u64_le_host(const uint8_t* input, size_t input_len, size_t offset, uint64_t* out) {
    if (offset > input_len || input_len - offset < 8ULL) {
        return NO;
    }
    uint64_t value = 0;
    for (uint32_t i = 0; i < 8U; ++i) {
        value |= ((uint64_t)input[offset + i]) << (8U * i);
    }
    *out = value;
    return YES;
}

static uint64_t gf2_matrix_times(const uint64_t* mat, uint64_t vec) {
    uint64_t sum = 0;
    int idx = 0;
    while (vec != 0) {
        if (vec & 1U) {
            sum ^= mat[idx];
        }
        vec >>= 1;
        idx += 1;
    }
    return sum;
}

static void gf2_matrix_square(uint64_t* square, const uint64_t* mat) {
    for (int n = 0; n < 64; ++n) {
        square[n] = gf2_matrix_times(mat, mat[n]);
    }
}

static uint64_t crc64_shift(uint64_t crc1, size_t len2) {
    if (len2 == 0) {
        return crc1;
    }
    uint64_t mat[64];
    uint64_t square[64];
    uint64_t row = 1ULL;
    mat[0] = CRC64_POLY;
    for (int n = 1; n < 64; ++n) {
        mat[n] = row;
        row <<= 1;
    }

    size_t len = len2 * 8ULL;
    while (len != 0) {
        if (len & 1ULL) {
            crc1 = gf2_matrix_times(mat, crc1);
        }
        gf2_matrix_square(square, mat);
        for (int n = 0; n < 64; ++n) {
            mat[n] = square[n];
        }
        len >>= 1;
    }

    return crc1;
}

static uint64_t crc64_combine(uint64_t crc1, uint64_t crc2, size_t len2) {
    if (len2 == 0) {
        return crc1;
    }
    uint64_t shifted = crc64_shift(crc1, len2);
    return shifted ^ crc2;
}

static BOOL wait_for_command_buffer(id<MTLCommandBuffer> command) {
    static const NSTimeInterval kMetalCommandTimeoutSeconds = 10.0;
    NSDate* deadline = [NSDate dateWithTimeIntervalSinceNow:kMetalCommandTimeoutSeconds];
    while (true) {
        MTLCommandBufferStatus status = [command status];
        if (status == MTLCommandBufferStatusCompleted) {
            return [command error] == nil;
        }
        if (status == MTLCommandBufferStatusError) {
            return NO;
        }
        if ([deadline timeIntervalSinceNow] <= 0.0) {
            return NO;
        }
        [NSThread sleepForTimeInterval:0.001];
    }
}

// CPU finalize: combine masks into tape offsets with correct escape + quote parity across blocks.
static void finalize_tape_from_masks(const uint8_t* in, size_t n, const uint32_t* s, const uint32_t* q, const uint32_t* b, size_t blocks, uint32_t* out, size_t out_cap, size_t* out_len) {
    (void)in; // suppress unused-parameter warning; masks provide all needed info
    size_t outi = 0;
    size_t bs_carry = 0; // backslash run length carry into current block
    bool in_string = false;
    for (size_t blk = 0; blk < blocks; ++blk) {
        uint32_t sm = s[blk];
        uint32_t qm = q[blk];
        uint32_t bm = b[blk];
        size_t base = blk * 32;
        for (uint32_t bit = 0; bit < 32; ++bit) {
            size_t pos = base + bit;
            if (pos >= n) break;
            bool is_bslash = ((bm >> bit) & 1u) != 0u;
            bool is_quote  = ((qm >> bit) & 1u) != 0u;
            bool is_struct = ((sm >> bit) & 1u) != 0u;
            if (is_bslash) {
                bs_carry += 1;
                continue;
            }
            if (is_quote) {
                bool escaped = (bs_carry & 1u) == 1u;
                if (!escaped) {
                    // Unescaped quote toggles string state and is recorded.
                    in_string = !in_string;
                    if (outi < out_cap) { out[outi++] = (uint32_t)pos; }
                }
                bs_carry = 0;
                continue;
            }
            // Non-backslash, non-quote: reset run
            bs_carry = 0;
            if (!in_string && is_struct) {
                if (outi < out_cap) { out[outi++] = (uint32_t)pos; }
            }
        }
    }
    *out_len = outi;
}

int json_stage1_build_tape_metal_impl(const uint8_t* input_ptr,
                                      size_t input_len,
                                      uint32_t* out_offsets,
                                      size_t out_capacity,
                                      size_t* out_len) {
    @autoreleasepool {
        id<MTLDevice> dev = MTLCreateSystemDefaultDevice();
        if (!dev) return 3;
        NSError* err = nil;
        id<MTLLibrary> lib = [dev newLibraryWithSource:kMSLSource options:nil error:&err];
        if (!lib) { return 4; }
        id<MTLFunction> fn = [lib newFunctionWithName:@"classify_json"];
        if (!fn) { return 5; }
        id<MTLComputePipelineState> pso = [dev newComputePipelineStateWithFunction:fn error:&err];
        if (!pso) { return 6; }
        id<MTLCommandQueue> queue = [dev newCommandQueue];
        if (!queue) { return 7; }

        // Buffers
        NSUInteger n = (NSUInteger)input_len;
        NSUInteger blocks = (n + 31u) / 32u;
        MTLResourceOptions opts = MTLResourceStorageModeShared;
        id<MTLBuffer> inbuf = [dev newBufferWithLength:n options:opts];
        if (!inbuf) { return 8; }
        memcpy([inbuf contents], input_ptr, n);
        id<MTLBuffer> sBuf = [dev newBufferWithLength:blocks * sizeof(uint32_t) options:opts];
        id<MTLBuffer> qBuf = [dev newBufferWithLength:blocks * sizeof(uint32_t) options:opts];
        id<MTLBuffer> bBuf = [dev newBufferWithLength:blocks * sizeof(uint32_t) options:opts];
        id<MTLBuffer> nBuf = [dev newBufferWithLength:sizeof(uint32_t) options:opts];
        *(uint32_t*)[nBuf contents] = (uint32_t)n;

        id<MTLCommandBuffer> cb = [queue commandBuffer];
        id<MTLComputeCommandEncoder> enc = [cb computeCommandEncoder];
        [enc setComputePipelineState:pso];
        [enc setBuffer:inbuf offset:0 atIndex:0];
        [enc setBuffer:sBuf offset:0 atIndex:1];
        [enc setBuffer:qBuf offset:0 atIndex:2];
        [enc setBuffer:bBuf offset:0 atIndex:3];
        [enc setBuffer:nBuf offset:0 atIndex:4];

        NSUInteger tg = MIN((NSUInteger)pso.maxTotalThreadsPerThreadgroup, 256u);
        if (tg == 0) tg = 64u;
        MTLSize threadsPerGrid = MTLSizeMake(blocks, 1, 1);
        MTLSize threadsPerTG = MTLSizeMake(tg, 1, 1);
        [enc dispatchThreads:threadsPerGrid threadsPerThreadgroup:threadsPerTG];
        [enc endEncoding];
        [cb commit];
        if (!wait_for_command_buffer(cb)) { return 3; }

        // CPU finalize using masks
        const uint32_t* s = (const uint32_t*)[sBuf contents];
        const uint32_t* qmask = (const uint32_t*)[qBuf contents];
        const uint32_t* b = (const uint32_t*)[bBuf contents];
        finalize_tape_from_masks(input_ptr, (size_t)n, s, qmask, b, (size_t)blocks, out_offsets, out_capacity, out_len);

        return 0;
    }
}

int norito_sequence_plan_metal_impl(const uint8_t* input_ptr,
                                    size_t input_len,
                                    uint8_t flags,
                                    uint32_t layout_kind,
                                    NoritoSequenceSpanMetal* out_spans,
                                    size_t out_capacity,
                                    size_t* out_count,
                                    size_t* out_used) {
    if (!input_ptr || !out_count || !out_used) { return 1; }
    if (out_capacity > 0 && !out_spans) { return 1; }

    uint64_t count64 = 0;
    if (!read_u64_le_host(input_ptr, input_len, 0, &count64) || count64 > (uint64_t)SIZE_MAX) {
        return 1;
    }
    size_t count = (size_t)count64;
    *out_count = count;
    *out_used = 0;
    if (count > out_capacity) {
        return 2;
    }

    size_t data_start = 0;
    size_t data_len = 0;
    size_t fixed_used = 0;
    if (layout_kind == LAYOUT_FIXED_OFFSETS_HOST) {
        if (count > (SIZE_MAX / 8ULL) - 1ULL) { return 1; }
        size_t table_len = (count + 1ULL) * 8ULL;
        if (input_len < 8ULL || table_len > input_len - 8ULL) { return 1; }
        size_t table_start = 8ULL;
        size_t table_end = table_start + table_len;
        uint64_t first = 0;
        uint64_t last = 0;
        if (!read_u64_le_host(input_ptr, input_len, table_start, &first)
            || !read_u64_le_host(input_ptr, input_len, table_start + count * 8ULL, &last)
            || first != 0
            || last > (uint64_t)SIZE_MAX) {
            return 1;
        }
        data_start = table_end;
        data_len = (size_t)last;
        if (data_len > input_len - data_start) { return 1; }
        fixed_used = data_start + data_len;
        *out_used = fixed_used;
        if (count == 0) {
            return data_len == 0 ? 0 : 1;
        }
    } else if (layout_kind == LAYOUT_LENGTH_PREFIXED_HOST) {
        if (count == 0) {
            *out_used = 8ULL;
            return 0;
        }
    } else {
        return 3;
    }

    @autoreleasepool {
        id<MTLDevice> dev = MTLCreateSystemDefaultDevice();
        if (!dev) return 3;
        NSError* err = nil;
        id<MTLLibrary> lib = [dev newLibraryWithSource:kMSLSource options:nil error:&err];
        if (!lib) { return 4; }
        NSString* functionName = layout_kind == LAYOUT_FIXED_OFFSETS_HOST
            ? @"sequence_fixed_offsets"
            : @"sequence_length_prefixed";
        id<MTLFunction> fn = [lib newFunctionWithName:functionName];
        if (!fn) { return 4; }
        id<MTLComputePipelineState> pso = [dev newComputePipelineStateWithFunction:fn error:&err];
        if (!pso) { return 4; }
        id<MTLCommandQueue> queue = [dev newCommandQueue];
        if (!queue) { return 4; }

        MTLResourceOptions opts = MTLResourceStorageModeShared;
        id<MTLBuffer> inbuf = [dev newBufferWithLength:(NSUInteger)input_len options:opts];
        if (!inbuf) { return 4; }
        memcpy([inbuf contents], input_ptr, input_len);
        id<MTLBuffer> nBuf = [dev newBufferWithLength:sizeof(uint64_t) options:opts];
        id<MTLBuffer> countBuf = [dev newBufferWithLength:sizeof(uint64_t) options:opts];
        id<MTLBuffer> spanBuf = [dev newBufferWithLength:count * sizeof(NoritoSequenceSpanMetal) options:opts];
        id<MTLBuffer> statusBuf = [dev newBufferWithLength:sizeof(uint32_t) options:opts];
        id<MTLBuffer> usedBuf = [dev newBufferWithLength:sizeof(uint64_t) options:opts];
        if (!nBuf || !countBuf || !spanBuf || !statusBuf || !usedBuf) { return 4; }
        *(uint64_t*)[nBuf contents] = (uint64_t)input_len;
        *(uint64_t*)[countBuf contents] = (uint64_t)count;
        *(uint32_t*)[statusBuf contents] = 0;
        *(uint64_t*)[usedBuf contents] = (uint64_t)*out_used;

        id<MTLCommandBuffer> cb = [queue commandBuffer];
        id<MTLComputeCommandEncoder> enc = [cb computeCommandEncoder];
        [enc setComputePipelineState:pso];
        [enc setBuffer:inbuf offset:0 atIndex:0];
        [enc setBuffer:nBuf offset:0 atIndex:1];
        [enc setBuffer:countBuf offset:0 atIndex:2];
        if (layout_kind == LAYOUT_FIXED_OFFSETS_HOST) {
            id<MTLBuffer> dataStartBuf = [dev newBufferWithLength:sizeof(uint64_t) options:opts];
            id<MTLBuffer> dataLenBuf = [dev newBufferWithLength:sizeof(uint64_t) options:opts];
            if (!dataStartBuf || !dataLenBuf) { return 4; }
            *(uint64_t*)[dataStartBuf contents] = (uint64_t)data_start;
            *(uint64_t*)[dataLenBuf contents] = (uint64_t)data_len;
            [enc setBuffer:dataStartBuf offset:0 atIndex:3];
            [enc setBuffer:dataLenBuf offset:0 atIndex:4];
            [enc setBuffer:spanBuf offset:0 atIndex:5];
            [enc setBuffer:statusBuf offset:0 atIndex:6];
            NSUInteger tg = MIN((NSUInteger)pso.maxTotalThreadsPerThreadgroup, 256u);
            if (tg == 0) tg = 64u;
            [enc dispatchThreads:MTLSizeMake((NSUInteger)count, 1, 1)
            threadsPerThreadgroup:MTLSizeMake(tg, 1, 1)];
        } else {
            id<MTLBuffer> flagsBuf = [dev newBufferWithLength:sizeof(uint8_t) options:opts];
            if (!flagsBuf) { return 4; }
            *(uint8_t*)[flagsBuf contents] = flags;
            [enc setBuffer:flagsBuf offset:0 atIndex:3];
            [enc setBuffer:spanBuf offset:0 atIndex:4];
            [enc setBuffer:usedBuf offset:0 atIndex:5];
            [enc setBuffer:statusBuf offset:0 atIndex:6];
            [enc dispatchThreads:MTLSizeMake(1, 1, 1)
            threadsPerThreadgroup:MTLSizeMake(1, 1, 1)];
        }
        [enc endEncoding];
        [cb commit];
        if (!wait_for_command_buffer(cb)) { return 4; }

        uint32_t status = *(uint32_t*)[statusBuf contents];
        if (status != 0) { return 1; }
        if (layout_kind == LAYOUT_LENGTH_PREFIXED_HOST) {
            uint64_t used64 = *(uint64_t*)[usedBuf contents];
            if (used64 > (uint64_t)SIZE_MAX) { return 1; }
            *out_used = (size_t)used64;
        } else {
            *out_used = fixed_used;
        }
        memcpy(out_spans, [spanBuf contents], count * sizeof(NoritoSequenceSpanMetal));
        return 0;
    }
}

int norito_crc64_metal_impl(const uint8_t* input_ptr,
                            size_t input_len,
                            uint64_t* out_crc) {
    if (!out_crc) { return 1; }
    @autoreleasepool {
        id<MTLDevice> dev = MTLCreateSystemDefaultDevice();
        if (!dev) return 3;
        NSError* err = nil;
        id<MTLLibrary> lib = [dev newLibraryWithSource:kMSLSource options:nil error:&err];
        if (!lib) { return 4; }
        id<MTLFunction> fn = [lib newFunctionWithName:@"crc64_chunks"];
        if (!fn) { return 5; }
        id<MTLComputePipelineState> pso = [dev newComputePipelineStateWithFunction:fn error:&err];
        if (!pso) { return 6; }
        id<MTLCommandQueue> queue = [dev newCommandQueue];
        if (!queue) { return 7; }

        NSUInteger n = (NSUInteger)input_len;
        MTLResourceOptions opts = MTLResourceStorageModeShared;
        id<MTLBuffer> inbuf = [dev newBufferWithLength:n options:opts];
        if (!inbuf) { return 8; }
        memcpy([inbuf contents], input_ptr, n);

        id<MTLBuffer> nBuf = [dev newBufferWithLength:sizeof(uint64_t) options:opts];
        if (!nBuf) { return 9; }
        *(uint64_t*)[nBuf contents] = (uint64_t)n;

        NSUInteger chunks = (n + (NSUInteger)CRC64_CHUNK_SIZE - 1u) / (NSUInteger)CRC64_CHUNK_SIZE;
        if (chunks == 0) {
            *out_crc = 0;
            return 0;
        }

        id<MTLBuffer> outBuf = [dev newBufferWithLength:chunks * sizeof(uint64_t) options:opts];
        if (!outBuf) { return 10; }

        id<MTLCommandBuffer> cb = [queue commandBuffer];
        id<MTLComputeCommandEncoder> enc = [cb computeCommandEncoder];
        [enc setComputePipelineState:pso];
        [enc setBuffer:inbuf offset:0 atIndex:0];
        [enc setBuffer:nBuf offset:0 atIndex:1];
        [enc setBuffer:outBuf offset:0 atIndex:2];

        NSUInteger tg = MIN((NSUInteger)pso.maxTotalThreadsPerThreadgroup, 256u);
        if (tg == 0) tg = 64u;
        MTLSize threadsPerGrid = MTLSizeMake(chunks, 1, 1);
        MTLSize threadsPerTG = MTLSizeMake(tg, 1, 1);
        [enc dispatchThreads:threadsPerGrid threadsPerThreadgroup:threadsPerTG];
        [enc endEncoding];
        [cb commit];
        if (!wait_for_command_buffer(cb)) { return 3; }

        const uint64_t* out_ptr = (const uint64_t*)[outBuf contents];
        uint64_t crc = CRC64_INIT;
        for (NSUInteger idx = 0; idx < chunks; ++idx) {
            size_t offset = (size_t)idx * CRC64_CHUNK_SIZE;
            size_t remaining = (offset < n) ? (n - offset) : 0;
            size_t seg_len = (remaining < CRC64_CHUNK_SIZE) ? remaining : CRC64_CHUNK_SIZE;
            crc = crc64_combine(crc, out_ptr[idx], seg_len);
        }
        *out_crc = crc ^ CRC64_XOR_OUT;
        return 0;
    }
}
