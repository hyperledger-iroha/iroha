package circuit

import (
	"fmt"
	"math/big"

	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/bits"
	"github.com/consensys/gnark/std/math/cmp"
	"github.com/consensys/gnark/std/math/uints"
)

const (
	// The SCCP payload codec bounds canonical text to 256 UTF-8 bytes. The
	// four-byte Taira sentinel leaves at most 252 one-byte base-105 symbols.
	maxI105SenderBytes    = 256
	maxI105Digits         = maxI105SenderBytes - len("test")
	maxI105CanonicalBytes = 208
	maxI105Base32Groups   = (maxI105CanonicalBytes*8 + 4) / 5
	i105ChecksumDigits    = 6
	i105Base              = 105
	i105Base58Digits      = 58
	i105HRPInitialPolymod = 0x04dd7d61
	i105Bech32mConstant   = 0x2bc830a3
)

var i105Alphabet = [i105Base]string{
	"1", "2", "3", "4", "5", "6", "7", "8", "9", "A", "B", "C", "D", "E", "F", "G", "H", "J", "K",
	"L", "M", "N", "P", "Q", "R", "S", "T", "U", "V", "W", "X", "Y", "Z", "a", "b", "c", "d", "e",
	"f", "g", "h", "i", "j", "k", "m", "n", "o", "p", "q", "r", "s", "t", "u", "v", "w", "x", "y",
	"z", "ｲ", "ﾛ", "ﾊ", "ﾆ", "ﾎ", "ﾍ", "ﾄ", "ﾁ", "ﾘ", "ﾇ", "ﾙ", "ｦ", "ﾜ", "ｶ", "ﾖ", "ﾀ", "ﾚ", "ｿ", "ﾂ",
	"ﾈ", "ﾅ", "ﾗ", "ﾑ", "ｳ", "ヰ", "ﾉ", "ｵ", "ｸ", "ﾔ", "ﾏ", "ｹ", "ﾌ", "ｺ", "ｴ", "ﾃ", "ｱ", "ｻ", "ｷ",
	"ﾕ", "ﾒ", "ﾐ", "ｼ", "ヱ", "ﾋ", "ﾓ", "ｾ", "ｽ",
}

// I105Witness supplies the bounded arithmetic trace used to decode an exact
// Taira AccountAddress. Every byte, carry, digit, and trace transition is
// constrained; none of these values is a host-side parser verdict.
type I105Witness struct {
	DigitCount      frontend.Variable
	Digits          [maxI105Digits]frontend.Variable
	CanonicalLength frontend.Variable
	Canonical       [maxI105CanonicalBytes]uints.U8
	CanonicalLE     [maxI105CanonicalBytes]uints.U8
	Base105Trace    [maxI105Digits + 1][maxI105CanonicalBytes]uints.U8
	Base105Carries  [maxI105Digits][maxI105CanonicalBytes + 1]uints.U8
}

// constrainCanonicalTairaI105 constrains the exact account/address.rs I105
// construction used by the closed SCCP sender grammar: literal `test` for
// discriminant 369, the repository's ordered 105-symbol UTF-8 alphabet,
// minimal base-105, and the six Bech32m checksum digits over HRP `snx`.
//
// The final-V1 SCCP sender grammar admits the canonical single-controller
// Ed25519 AccountAddress form. This is an explicit closed policy subset, not a
// permissive fallback: every other structurally valid AccountAddress form is
// rejected by the circuit.
func constrainCanonicalTairaI105(
	api frontend.API,
	encoded []uints.U8,
	encodedLength frontend.Variable,
	witness *I105Witness,
) error {
	if len(encoded) != maxI105SenderBytes {
		return fmt.Errorf("I105 encoded width is %d, expected %d", len(encoded), maxI105SenderBytes)
	}
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return fmt.Errorf("initialize I105 byte API: %w", err)
	}
	if _, err := u32Bytes(api, encodedLength); err != nil {
		return err
	}
	if _, err := u32Bytes(api, witness.DigitCount); err != nil {
		return err
	}
	if _, err := u32Bytes(api, witness.CanonicalLength); err != nil {
		return err
	}
	lengthComparator := cmp.NewBoundedComparator(api, big.NewInt(maxI105SenderBytes+1), false)
	lengthComparator.AssertIsLessEq(len("test")+i105ChecksumDigits+1, encodedLength)
	lengthComparator.AssertIsLessEq(encodedLength, maxI105SenderBytes)
	digitCountComparator := cmp.NewBoundedComparator(api, big.NewInt(int64(maxI105Digits+1)), false)
	digitCountComparator.AssertIsLessEq(i105ChecksumDigits+1, witness.DigitCount)
	digitCountComparator.AssertIsLessEq(witness.DigitCount, maxI105Digits)
	canonicalLengthComparator := cmp.NewBoundedComparator(api, big.NewInt(int64(maxI105CanonicalBytes+1)), false)
	canonicalLengthComparator.AssertIsLess(0, witness.CanonicalLength)
	canonicalLengthComparator.AssertIsLessEq(witness.CanonicalLength, maxI105CanonicalBytes)

	for index, expected := range []byte("test") {
		api.AssertIsEqual(byteAPI.Value(encoded[index]), expected)
	}

	digitComparator := cmp.NewBoundedComparator(api, big.NewInt(256), false)
	cursor := frontend.Variable(len("test"))
	for index := 0; index < maxI105Digits; index++ {
		active := digitCountComparator.IsLess(index, witness.DigitCount)
		digitByte := byteAPI.ValueOf(witness.Digits[index])
		digit := byteAPI.Value(digitByte)
		api.AssertIsEqual(digit, api.Mul(active, digit))
		digitComparator.AssertIsLess(digit, i105Base)
		isASCII := digitComparator.IsLess(digit, i105Base58Digits)
		isKana := api.Mul(active, api.Sub(1, isASCII))
		digitBits := bits.ToBinary(api, digit, bits.WithNbDigits(7))
		expectedBytes := i105DigitBytes(api, digitBits)
		activeCursor := api.Select(active, cursor, 0)
		actual0 := dynamicByte(api, byteAPI, encoded, activeCursor)
		api.AssertIsEqual(api.Mul(active, api.Sub(actual0, expectedBytes[0])), 0)
		actual1 := dynamicShiftedByte(api, byteAPI, encoded, activeCursor, 1)
		actual2 := dynamicShiftedByte(api, byteAPI, encoded, activeCursor, 2)
		api.AssertIsEqual(api.Mul(isKana, api.Sub(actual1, expectedBytes[1])), 0)
		api.AssertIsEqual(api.Mul(isKana, api.Sub(actual2, expectedBytes[2])), 0)
		cursor = api.Add(cursor, active, api.Mul(2, isKana))
	}
	api.AssertIsEqual(cursor, encodedLength)

	dataDigitCount := api.Sub(witness.DigitCount, i105ChecksumDigits)
	firstDigit := witness.Digits[0]
	api.AssertIsEqual(api.IsZero(firstDigit), 0)
	for byteIndex := 0; byteIndex < maxI105CanonicalBytes; byteIndex++ {
		api.AssertIsEqual(byteAPI.Value(witness.Base105Trace[0][byteIndex]), 0)
	}
	for digitIndex := 0; digitIndex < maxI105Digits; digitIndex++ {
		dataActive := digitCountComparator.IsLess(digitIndex, dataDigitCount)
		initialCarry := byteAPI.Value(witness.Base105Carries[digitIndex][0])
		api.AssertIsEqual(initialCarry, api.Mul(dataActive, witness.Digits[digitIndex]))
		multiplier := api.Add(1, api.Mul(i105Base-1, dataActive))
		for byteIndex := 0; byteIndex < maxI105CanonicalBytes; byteIndex++ {
			previous := byteAPI.Value(witness.Base105Trace[digitIndex][byteIndex])
			next := byteAPI.Value(witness.Base105Trace[digitIndex+1][byteIndex])
			carry := byteAPI.Value(witness.Base105Carries[digitIndex][byteIndex])
			nextCarry := byteAPI.Value(witness.Base105Carries[digitIndex][byteIndex+1])
			api.AssertIsEqual(
				api.Add(api.Mul(previous, multiplier), carry),
				api.Add(next, api.Mul(256, nextCarry)),
			)
		}
		api.AssertIsEqual(
			byteAPI.Value(witness.Base105Carries[digitIndex][maxI105CanonicalBytes]),
			0,
		)
	}

	for byteIndex := 0; byteIndex < maxI105CanonicalBytes; byteIndex++ {
		active := canonicalLengthComparator.IsLess(byteIndex, witness.CanonicalLength)
		canonical := byteAPI.Value(witness.Canonical[byteIndex])
		canonicalLE := byteAPI.Value(witness.CanonicalLE[byteIndex])
		api.AssertIsEqual(canonical, api.Mul(active, canonical))
		api.AssertIsEqual(canonicalLE, api.Mul(active, canonicalLE))
		reverseIndex := api.Select(
			active,
			api.Sub(witness.CanonicalLength, 1, byteIndex),
			0,
		)
		reversed := dynamicByte(api, byteAPI, witness.Canonical[:], reverseIndex)
		api.AssertIsEqual(canonicalLE, api.Mul(active, reversed))
		api.AssertIsEqual(
			byteAPI.Value(witness.Base105Trace[maxI105Digits][byteIndex]),
			canonicalLE,
		)
	}
	api.AssertIsEqual(api.IsZero(byteAPI.Value(witness.Canonical[0])), 0)

	if err := constrainSingleEd25519AccountAddress(api, byteAPI, witness); err != nil {
		return err
	}
	return constrainI105Checksum(api, byteAPI, digitCountComparator, witness, dataDigitCount)
}

func constrainSingleEd25519AccountAddress(
	api frontend.API,
	byteAPI *uints.Bytes,
	witness *I105Witness,
) error {
	api.AssertIsEqual(witness.CanonicalLength, 36)
	for index, expected := range []byte{0x02, 0x00, 0x01, 0x20} {
		api.AssertIsEqual(byteAPI.Value(witness.Canonical[index]), expected)
	}
	return nonZeroBytes(api, witness.Canonical[4:36])
}

func constrainI105Checksum(
	api frontend.API,
	byteAPI *uints.Bytes,
	digitCountComparator *cmp.BoundedComparator,
	witness *I105Witness,
	dataDigitCount frontend.Variable,
) error {
	canonicalBits := make([][]frontend.Variable, maxI105CanonicalBytes)
	for index := range canonicalBits {
		canonicalBits[index] = bits.ToBinary(
			api,
			byteAPI.Value(witness.Canonical[index]),
			bits.WithNbDigits(8),
		)
	}
	state := constantBits(i105HRPInitialPolymod, 30)
	bitLength := api.Mul(witness.CanonicalLength, 8)
	bitLengthComparator := cmp.NewBoundedComparator(api, big.NewInt(maxI105CanonicalBytes*8+1), false)
	for group := 0; group < maxI105Base32Groups; group++ {
		var value [5]frontend.Variable
		for bitIndex := 0; bitIndex < 5; bitIndex++ {
			streamIndex := group*5 + bitIndex
			if streamIndex >= maxI105CanonicalBytes*8 {
				value[4-bitIndex] = 0
				continue
			}
			byteIndex := streamIndex / 8
			msbIndex := streamIndex % 8
			value[4-bitIndex] = canonicalBits[byteIndex][7-msbIndex]
		}
		active := bitLengthComparator.IsLess(group*5, bitLength)
		candidate := i105PolymodStep(api, state, value[:])
		for bitIndex := range state {
			state[bitIndex] = api.Select(active, candidate[bitIndex], state[bitIndex])
		}
	}
	zero := []frontend.Variable{0, 0, 0, 0, 0}
	for range i105ChecksumDigits {
		state = i105PolymodStep(api, state, zero)
	}
	for bitIndex := range state {
		if (i105Bech32mConstant>>bitIndex)&1 == 1 {
			state[bitIndex] = api.Sub(1, state[bitIndex])
		}
	}
	for checksumIndex := 0; checksumIndex < i105ChecksumDigits; checksumIndex++ {
		shift := 5 * (i105ChecksumDigits - 1 - checksumIndex)
		expected := frontend.Variable(0)
		for bitIndex := 0; bitIndex < 5; bitIndex++ {
			expected = api.Add(expected, api.Mul(1<<bitIndex, state[shift+bitIndex]))
		}
		digitIndex := api.Add(dataDigitCount, checksumIndex)
		actual := dynamicVariable(api, witness.Digits[:], digitIndex, 8)
		api.AssertIsEqual(actual, expected)
	}
	// The checksum occupies exactly the final six active digits.
	api.AssertIsEqual(api.Add(dataDigitCount, i105ChecksumDigits), witness.DigitCount)
	_ = digitCountComparator
	return nil
}

func i105PolymodStep(
	api frontend.API,
	state []frontend.Variable,
	value []frontend.Variable,
) []frontend.Variable {
	if len(state) != 30 || len(value) != 5 {
		panic("I105 polymod uses exactly 30 state bits and five value bits")
	}
	next := make([]frontend.Variable, 30)
	copy(next[:5], value)
	copy(next[5:], state[:25])
	generators := [5]uint32{0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3}
	for generatorIndex, generator := range generators {
		selector := state[25+generatorIndex]
		for bitIndex := 0; bitIndex < 30; bitIndex++ {
			if (generator>>bitIndex)&1 == 1 {
				next[bitIndex] = xorBit(api, next[bitIndex], selector)
			}
		}
	}
	return next
}

func xorBit(api frontend.API, left, right frontend.Variable) frontend.Variable {
	return api.Sub(api.Add(left, right), api.Mul(2, left, right))
}

func constantBits(value uint32, width int) []frontend.Variable {
	result := make([]frontend.Variable, width)
	for index := range result {
		result[index] = (value >> index) & 1
	}
	return result
}

func i105DigitBytes(api frontend.API, digitBits []frontend.Variable) [3]frontend.Variable {
	var result [3]frontend.Variable
	for byteIndex := range result {
		values := make([]frontend.Variable, 128)
		for digit, symbol := range i105Alphabet {
			encoded := []byte(symbol)
			if byteIndex < len(encoded) {
				values[digit] = encoded[byteIndex]
			}
		}
		result[byteIndex] = selectByBits(api, values, digitBits)
	}
	return result
}

func dynamicShiftedByte(
	api frontend.API,
	byteAPI *uints.Bytes,
	values []uints.U8,
	index frontend.Variable,
	shift int,
) frontend.Variable {
	shifted := make([]uints.U8, len(values))
	for cursor := range shifted {
		if cursor+shift < len(values) {
			shifted[cursor] = values[cursor+shift]
		} else {
			shifted[cursor] = uints.NewU8(0)
		}
	}
	return dynamicByte(api, byteAPI, shifted, index)
}

func dynamicByte(
	api frontend.API,
	byteAPI *uints.Bytes,
	values []uints.U8,
	index frontend.Variable,
) frontend.Variable {
	variables := make([]frontend.Variable, len(values))
	for cursor := range values {
		variables[cursor] = byteAPI.Value(values[cursor])
	}
	return dynamicVariable(api, variables, index, 8)
}

func dynamicVariable(
	api frontend.API,
	values []frontend.Variable,
	index frontend.Variable,
	indexBits int,
) frontend.Variable {
	digits := bits.ToBinary(api, index, bits.WithNbDigits(indexBits))
	return selectByBits(api, values, digits)
}

func selectByBits(
	api frontend.API,
	values []frontend.Variable,
	indexBits []frontend.Variable,
) frontend.Variable {
	width := 1 << len(indexBits)
	if len(values) > width {
		panic("lookup table exceeds index width")
	}
	current := make([]frontend.Variable, width)
	for index := range current {
		current[index] = 0
	}
	copy(current, values)
	for index := range current {
		if current[index] == nil {
			current[index] = 0
		}
	}
	for bitIndex := 0; bitIndex < len(indexBits); bitIndex++ {
		next := make([]frontend.Variable, len(current)/2)
		for index := range next {
			next[index] = api.Select(indexBits[bitIndex], current[2*index+1], current[2*index])
		}
		current = next
	}
	return current[0]
}
