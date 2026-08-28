// Package circuit implements the repository-owned SCCP final-V1 constraint systems.
package circuit

import (
	"fmt"
	"math/big"

	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/cmp"
	"github.com/consensys/gnark/std/math/uints"
)

var blake2bIV = [8]uint64{
	0x6a09e667f3bcc908,
	0xbb67ae8584caa73b,
	0x3c6ef372fe94f82b,
	0xa54ff53a5f1d36f1,
	0x510e527fade682d1,
	0x9b05688c2b3e6c1f,
	0x1f83d9abfb41bd6b,
	0x5be0cd19137e2179,
}

var blake2bSigma = [12][16]uint8{
	{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15},
	{14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3},
	{11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4},
	{7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8},
	{9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13},
	{2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9},
	{12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11},
	{13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10},
	{6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5},
	{10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0},
	{0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15},
	{14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3},
}

// blake2b256 constrains the unkeyed BLAKE2b digest with a 32-byte output.
// data is the maximum input buffer; bytes at and after length must be zero.
func blake2b256(api frontend.API, data []uints.U8, length frontend.Variable) ([32]uints.U8, error) {
	var out [32]uints.U8
	if len(data) == 0 {
		return out, fmt.Errorf("BLAKE2b input buffer must not be empty")
	}
	u64api, err := uints.New[uints.U64](api)
	if err != nil {
		return out, fmt.Errorf("initialize uint64 API: %w", err)
	}
	byteapi, err := uints.NewBytes(api)
	if err != nil {
		return out, fmt.Errorf("initialize byte API: %w", err)
	}
	u32api, err := uints.New[uints.U32](api)
	if err != nil {
		return out, fmt.Errorf("initialize uint32 API: %w", err)
	}
	// Range-check before using the bounded comparator. All SCCP buffers are far
	// below 2^32 bytes; this prevents a near-modulus field element from wrapping
	// into the comparator's short non-negative representation.
	_ = u32api.ValueOf(length)
	bound := new(big.Int).SetUint64(uint64(len(data) + 128))
	comparator := cmp.NewBoundedComparator(api, bound, false)
	comparator.AssertIsLess(0, length)
	comparator.AssertIsLessEq(length, len(data))

	// Bytes outside the declared message are canonical zero padding. This both
	// prevents alternate witnesses and ensures every declared buffer variable is constrained.
	for i := range data {
		active := comparator.IsLess(i, length)
		value := byteapi.Value(data[i])
		api.AssertIsEqual(value, api.Mul(active, value))
	}

	var h [8]uints.U64
	for i := range h {
		h[i] = uints.NewU64(blake2bIV[i])
	}
	h[0] = u64api.Xor(h[0], uints.NewU64(0x01010020)) // digest=32, key=0, fanout=1, depth=1

	blocks := (len(data) + 127) / 128
	for block := 0; block < blocks; block++ {
		start := block * 128
		end := start + 128
		active := comparator.IsLess(start, length)
		last := api.Mul(active, comparator.IsLessEq(length, end))
		counter := api.Select(comparator.IsLess(length, end), length, end)

		var blockBytes [128]uints.U8
		for i := range blockBytes {
			index := start + i
			if index < len(data) {
				blockBytes[i] = data[index]
			} else {
				blockBytes[i] = uints.NewU8(0)
			}
		}
		compressed := blake2bCompress(api, u64api, h, blockBytes, counter, last)
		for i := range h {
			h[i] = selectU64(api, active, compressed[i], h[i])
		}
	}

	for word := 0; word < 4; word++ {
		copy(out[word*8:(word+1)*8], h[word][:])
	}
	return out, nil
}

func blake2bCompress(
	api frontend.API,
	u64api *uints.BinaryField[uints.U64],
	h [8]uints.U64,
	block [128]uints.U8,
	counter frontend.Variable,
	last frontend.Variable,
) [8]uints.U64 {
	var m [16]uints.U64
	for i := range m {
		m[i] = u64api.PackLSB(block[i*8 : (i+1)*8]...)
	}
	var v [16]uints.U64
	copy(v[:8], h[:])
	for i := range blake2bIV {
		v[i+8] = uints.NewU64(blake2bIV[i])
	}
	v[12] = u64api.Xor(v[12], u64api.ValueOf(counter))
	var finalMask uints.U64
	for i := range finalMask {
		finalMask[i] = uints.U8{Val: api.Select(last, 0xff, 0)}
	}
	v[14] = u64api.Xor(v[14], finalMask)

	for round := range blake2bSigma {
		s := blake2bSigma[round]
		blake2bG(u64api, &v, 0, 4, 8, 12, m[s[0]], m[s[1]])
		blake2bG(u64api, &v, 1, 5, 9, 13, m[s[2]], m[s[3]])
		blake2bG(u64api, &v, 2, 6, 10, 14, m[s[4]], m[s[5]])
		blake2bG(u64api, &v, 3, 7, 11, 15, m[s[6]], m[s[7]])
		blake2bG(u64api, &v, 0, 5, 10, 15, m[s[8]], m[s[9]])
		blake2bG(u64api, &v, 1, 6, 11, 12, m[s[10]], m[s[11]])
		blake2bG(u64api, &v, 2, 7, 8, 13, m[s[12]], m[s[13]])
		blake2bG(u64api, &v, 3, 4, 9, 14, m[s[14]], m[s[15]])
	}
	var out [8]uints.U64
	for i := range out {
		out[i] = u64api.Xor(h[i], v[i], v[i+8])
	}
	return out
}

func blake2bG(u64api *uints.BinaryField[uints.U64], v *[16]uints.U64, a, b, c, d int, x, y uints.U64) {
	v[a] = u64api.Add(v[a], v[b], x)
	v[d] = u64api.Lrot(u64api.Xor(v[d], v[a]), -32)
	v[c] = u64api.Add(v[c], v[d])
	v[b] = u64api.Lrot(u64api.Xor(v[b], v[c]), -24)
	v[a] = u64api.Add(v[a], v[b], y)
	v[d] = u64api.Lrot(u64api.Xor(v[d], v[a]), -16)
	v[c] = u64api.Add(v[c], v[d])
	v[b] = u64api.Lrot(u64api.Xor(v[b], v[c]), -63)
}

func selectU64(api frontend.API, selector frontend.Variable, whenTrue, whenFalse uints.U64) uints.U64 {
	var out uints.U64
	for i := range out {
		out[i] = uints.U8{Val: api.Select(selector, whenTrue[i].Val, whenFalse[i].Val)}
	}
	return out
}
