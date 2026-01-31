#import <Metal/Metal.h>
#import <Foundation/Foundation.h>
#include <stdint.h>
#include <stddef.h>
#include <string.h>

static NSString* kMSLSource = @"\n"
"using namespace metal;\n"
"constant uint GPUZSTD_CHUNK = 4096u;\n"
"kernel void gpuzstd_hash_chunks(\n"
"    device const uchar* in [[buffer(0)]],\n"
"    constant uint& n [[buffer(1)]],\n"
"    device uint* out [[buffer(2)]],\n"
"    uint gid [[thread_position_in_grid]]) {\n"
"    uint start = gid * GPUZSTD_CHUNK;\n"
"    if (start >= n) { return; }\n"
"    uint end = min(n, start + GPUZSTD_CHUNK);\n"
"    uint h = 2166136261u;\n"
"    for (uint i = start; i < end; ++i) {\n"
"        h ^= uint(in[i]);\n"
"        h *= 16777619u;\n"
"    }\n"
"    out[gid] = h;\n"
"}\n";

int gpuzstd_metal_prepass(const uint8_t* input_ptr, size_t input_len, uint64_t* out_digest) {
    if (!out_digest) { return 1; }
    @autoreleasepool {
        id<MTLDevice> dev = MTLCreateSystemDefaultDevice();
        if (!dev) { return 3; }
        NSError* err = nil;
        id<MTLLibrary> lib = [dev newLibraryWithSource:kMSLSource options:nil error:&err];
        if (!lib) { return 4; }
        id<MTLFunction> fn = [lib newFunctionWithName:@"gpuzstd_hash_chunks"];
        if (!fn) { return 5; }
        id<MTLComputePipelineState> pso = [dev newComputePipelineStateWithFunction:fn error:&err];
        if (!pso) { return 6; }
        id<MTLCommandQueue> queue = [dev newCommandQueue];
        if (!queue) { return 7; }

        const uint32_t n = (uint32_t)input_len;
        if (n == 0) {
            *out_digest = 0;
            return 0;
        }
        const uint32_t chunk = 4096u;
        const uint32_t blocks = (n + (chunk - 1u)) / chunk;

        MTLResourceOptions opts = MTLResourceStorageModeShared;
        id<MTLBuffer> inbuf = [dev newBufferWithLength:n options:opts];
        if (!inbuf) { return 8; }
        memcpy([inbuf contents], input_ptr, n);
        id<MTLBuffer> outbuf = [dev newBufferWithLength:blocks * sizeof(uint32_t) options:opts];
        if (!outbuf) { return 9; }
        id<MTLBuffer> nbuf = [dev newBufferWithLength:sizeof(uint32_t) options:opts];
        *(uint32_t*)[nbuf contents] = n;

        id<MTLCommandBuffer> cb = [queue commandBuffer];
        id<MTLComputeCommandEncoder> enc = [cb computeCommandEncoder];
        [enc setComputePipelineState:pso];
        [enc setBuffer:inbuf offset:0 atIndex:0];
        [enc setBuffer:nbuf offset:0 atIndex:1];
        [enc setBuffer:outbuf offset:0 atIndex:2];

        NSUInteger tg = MIN((NSUInteger)pso.maxTotalThreadsPerThreadgroup, 256u);
        if (tg == 0) { tg = 64u; }
        MTLSize threadsPerGrid = MTLSizeMake(blocks, 1, 1);
        MTLSize threadsPerTG = MTLSizeMake(tg, 1, 1);
        [enc dispatchThreads:threadsPerGrid threadsPerThreadgroup:threadsPerTG];
        [enc endEncoding];
        [cb commit];
        [cb waitUntilCompleted];

        uint64_t digest = 0;
        const uint32_t* out = (const uint32_t*)[outbuf contents];
        for (uint32_t i = 0; i < blocks; ++i) {
            digest ^= ((uint64_t)out[i] << (i & 31u));
        }
        *out_digest = digest;
        return 0;
    }
}
