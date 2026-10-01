#include <metal_stdlib>
using namespace metal;

// FIPS 202 Keccak-f[1600]. Continuations are exact borrowed SHA3 states;
// the typed CPU owner has already encoded the complete prefix and body.
constant ulong keccak256_rc[24] = {
    0x0000000000000001UL,
    0x0000000000008082UL,
    0x800000000000808aUL,
    0x8000000080008000UL,
    0x000000000000808bUL,
    0x0000000080000001UL,
    0x8000000080008081UL,
    0x8000000000008009UL,
    0x000000000000008aUL,
    0x0000000000000088UL,
    0x0000000080008009UL,
    0x000000008000000aUL,
    0x000000008000808bUL,
    0x800000000000008bUL,
    0x8000000000008089UL,
    0x8000000000008003UL,
    0x8000000000008002UL,
    0x8000000000000080UL,
    0x000000000000800aUL,
    0x800000008000000aUL,
    0x8000000080008081UL,
    0x8000000000008080UL,
    0x0000000080000001UL,
    0x8000000080008008UL
};
constant uint keccak256_rho[25] = {0,1,62,28,27,36,44,6,55,20,3,10,43,25,39,41,45,15,21,8,18,2,61,56,14};
inline ulong keccak256_rot(ulong a, uint n) {
    return (a << n) | (a >> ((64u - n) & 63u));
}
// All software-addressable private permutation storage is in a per-job,
// volatile, sensitive shared allocation. The kernel clears all60 words before
// successful completion; the host clears pages again only after completion.
// This does not assert physical GPU-register or driver-scratch sanitization.
inline void keccak256_permute(device volatile ulong *s) {
    for (uint round=0; round<24; ++round) {
        for (uint x=0;x<5;++x)
            s[25+x]=s[x]^s[x+5]^s[x+10]^s[x+15]^s[x+20];
        for (uint x=0;x<5;++x) {
            s[30+x]=s[25+(x+4)%5]^keccak256_rot(s[25+(x+1)%5],1);
            for (uint y=0;y<5;++y) s[x+5*y]^=s[30+x];
        }
        for (uint x=0;x<5;++x) for (uint y=0;y<5;++y)
            s[35+y+5*((2*x+3*y)%5)]=keccak256_rot(s[x+5*y],keccak256_rho[x+5*y]);
        for (uint x=0;x<5;++x) for (uint y=0;y<5;++y)
            s[x+5*y]=s[35+x+5*y]^((~s[35+(x+1)%5+5*y])&s[35+(x+2)%5+5*y]);
        s[0]^=keccak256_rc[round];
    }
}
kernel void fastpq_sha3_256_continuations(
    device const ulong *prefixes [[buffer(0)]],
    device const uchar *bodies [[buffer(1)]],
    device const ulong *slices [[buffer(2)]],
    device ulong *output [[buffer(3)]],
    device volatile ulong *scratch [[buffer(4)]],
    constant uint &job_count [[buffer(5)]],
    uint gid [[thread_position_in_grid]]
) {
    if (gid>=job_count) return;
    device volatile ulong *s=scratch+60ul*gid;
    for (uint word=0;word<25;++word) s[word]=prefixes[26ul*gid+word];
    uint position=(uint)prefixes[26ul*gid+25];
    const ulong start=slices[2ul*gid], length=slices[2ul*gid+1];
    for (ulong i=0;i<length;++i) {
        s[position/8]^=((ulong)bodies[start+i])<<((position%8)*8);
        if (++position==136) { keccak256_permute(s); position=0; }
    }
    s[position/8]^=0x06ul<<((position%8)*8);
    s[16]^=0x80ul<<56;
    keccak256_permute(s);
    for (uint word=0;word<4;++word) output[4ul*gid+word]=s[word];
    for (uint word=0;word<60;++word) s[word]=0;
}
