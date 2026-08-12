
            #include <metal_stdlib>
            using namespace metal;
            kernel void vadd32(device const uint4* a [[buffer(0)]],
                               device const uint4* b [[buffer(1)]],
                               device uint4* out [[buffer(2)]],
                               uint id [[thread_position_in_grid]]) {
                out[id] = a[id] + b[id];
            }
        