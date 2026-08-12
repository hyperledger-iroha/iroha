
            #include <metal_stdlib>
            using namespace metal;
            kernel void vadd64(device const ulong2* a [[buffer(0)]],
                               device const ulong2* b [[buffer(1)]],
                               device ulong2* out [[buffer(2)]],
                               uint id [[thread_position_in_grid]]) {
                out[id] = a[id] + b[id];
            }
        