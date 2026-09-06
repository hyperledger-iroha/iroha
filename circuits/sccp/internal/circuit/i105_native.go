package circuit

import (
	"fmt"
	"math/big"

	"github.com/consensys/gnark/std/math/uints"
)

var katI105CanonicalAccount = [36]byte{
	0x02, 0x00, 0x01, 0x20,
	0x0b, 0x51, 0x3a, 0xd9, 0xb4, 0x92, 0x40, 0x15,
	0xca, 0x09, 0x02, 0xed, 0x07, 0x90, 0x44, 0xd3,
	0xac, 0x5d, 0xbe, 0xc2, 0x30, 0x6f, 0x06, 0x94,
	0x8c, 0x10, 0xda, 0x8e, 0xb6, 0xe3, 0x9f, 0x2d,
}

func nativeI105Digits(canonical []byte) []byte {
	value := new(big.Int).SetBytes(canonical)
	base := big.NewInt(i105Base)
	quotient := new(big.Int)
	remainder := new(big.Int)
	digits := make([]byte, 0, (len(canonical)*8+5)/6)
	for value.Sign() != 0 {
		quotient.QuoRem(value, base, remainder)
		digits = append(digits, byte(remainder.Uint64()))
		value.Set(quotient)
	}
	if len(digits) == 0 {
		digits = append(digits, 0)
	}
	for left, right := 0, len(digits)-1; left < right; left, right = left+1, right-1 {
		digits[left], digits[right] = digits[right], digits[left]
	}
	return digits
}

func nativeI105Checksum(canonical []byte) [i105ChecksumDigits]byte {
	step := func(checksum uint32, value byte) uint32 {
		generators := [5]uint32{0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3}
		top := checksum >> 25
		checksum = ((checksum & 0x1ff_ffff) << 5) ^ uint32(value)
		for index, generator := range generators {
			if (top>>index)&1 == 1 {
				checksum ^= generator
			}
		}
		return checksum
	}
	checksum := uint32(1)
	for _, value := range []byte("snx") {
		checksum = step(checksum, value>>5)
	}
	checksum = step(checksum, 0)
	for _, value := range []byte("snx") {
		checksum = step(checksum, value&0x1f)
	}
	var accumulator uint32
	var bitCount uint32
	for _, value := range canonical {
		accumulator = (accumulator << 8) | uint32(value)
		bitCount += 8
		for bitCount >= 5 {
			bitCount -= 5
			checksum = step(checksum, byte((accumulator>>bitCount)&0x1f))
		}
	}
	if bitCount > 0 {
		checksum = step(checksum, byte((accumulator<<(5-bitCount))&0x1f))
	}
	for range i105ChecksumDigits {
		checksum = step(checksum, 0)
	}
	polymod := checksum ^ i105Bech32mConstant
	var result [i105ChecksumDigits]byte
	for index := range result {
		shift := 5 * (i105ChecksumDigits - 1 - index)
		result[index] = byte((polymod >> shift) & 0x1f)
	}
	return result
}

func nativeTairaI105(canonical []byte) ([]byte, []byte) {
	digits := nativeI105Digits(canonical)
	checksum := nativeI105Checksum(canonical)
	digits = append(digits, checksum[:]...)
	encoded := []byte("test")
	for _, digit := range digits {
		encoded = append(encoded, []byte(i105Alphabet[digit])...)
	}
	return encoded, digits
}

func initializeI105Witness(witness *I105Witness) {
	witness.DigitCount = 0
	witness.CanonicalLength = 0
	for index := range witness.Digits {
		witness.Digits[index] = 0
	}
	zeroU8s(witness.Canonical[:])
	zeroU8s(witness.CanonicalLE[:])
	for index := range witness.Base105Trace {
		zeroU8s(witness.Base105Trace[index][:])
	}
	for index := range witness.Base105Carries {
		zeroU8s(witness.Base105Carries[index][:])
	}
}

func populateI105Witness(witness *I105Witness, canonical []byte) ([]byte, error) {
	initializeI105Witness(witness)
	if len(canonical) == 0 || len(canonical) > maxI105CanonicalBytes {
		return nil, fmt.Errorf("canonical I105 payload length %d is out of range", len(canonical))
	}
	encoded, digits := nativeTairaI105(canonical)
	if len(encoded) > maxI105SenderBytes || len(digits) > maxI105Digits {
		return nil, fmt.Errorf("encoded I105 payload exceeds the circuit bound")
	}
	witness.DigitCount = len(digits)
	for index, digit := range digits {
		witness.Digits[index] = int(digit)
	}
	witness.CanonicalLength = len(canonical)
	copyU8(witness.Canonical[:], canonical)
	for index := range canonical {
		witness.CanonicalLE[index] = uints.NewU8(canonical[len(canonical)-1-index])
	}

	dataDigits := digits[:len(digits)-i105ChecksumDigits]
	state := make([]byte, maxI105CanonicalBytes)
	copyU8(witness.Base105Trace[0][:], state)
	for digitIndex := 0; digitIndex < maxI105Digits; digitIndex++ {
		next := append([]byte(nil), state...)
		carry := 0
		if digitIndex < len(dataDigits) {
			carry = int(dataDigits[digitIndex])
			witness.Base105Carries[digitIndex][0] = uints.NewU8(byte(carry))
			for byteIndex := range state {
				value := int(state[byteIndex])*i105Base + carry
				next[byteIndex] = byte(value)
				carry = value >> 8
				witness.Base105Carries[digitIndex][byteIndex+1] = uints.NewU8(byte(carry))
			}
		}
		if carry != 0 {
			return nil, fmt.Errorf("I105 base conversion exceeds the canonical byte bound")
		}
		copyU8(witness.Base105Trace[digitIndex+1][:], next)
		state = next
	}
	return encoded, nil
}
