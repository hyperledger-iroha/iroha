package circuit

import (
	"fmt"

	"github.com/consensys/gnark/frontend"
	zkhash "github.com/consensys/gnark/std/hash"
	"github.com/consensys/gnark/std/hash/sha2"
	"github.com/consensys/gnark/std/hash/sha3"
	"github.com/consensys/gnark/std/math/uints"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

func hashFixed(api frontend.API, kind profile.SignalHash, input []uints.U8) ([]uints.U8, error) {
	var (
		h   zkhash.BinaryFixedLengthHasher
		err error
	)
	switch kind {
	case profile.KeccakSignal:
		h, err = sha3.NewLegacyKeccak256(api)
	case profile.SHA256Signal:
		h, err = sha2.New(api)
	default:
		return nil, fmt.Errorf("unsupported fixed hash %q", kind)
	}
	if err != nil {
		return nil, err
	}
	h.Write(input)
	return h.Sum(), nil
}

func hashVariable(api frontend.API, kind profile.SignalHash, input []uints.U8, length frontend.Variable) ([]uints.U8, error) {
	var (
		h   zkhash.BinaryFixedLengthHasher
		err error
	)
	switch kind {
	case profile.KeccakSignal:
		h, err = sha3.NewLegacyKeccak256(api)
	case profile.SHA256Signal:
		h, err = sha2.New(api)
	default:
		return nil, fmt.Errorf("unsupported variable hash %q", kind)
	}
	if err != nil {
		return nil, err
	}
	h.Write(input)
	return h.FixedLengthSum(length), nil
}

func assertBytesEqual(api frontend.API, left, right []uints.U8) error {
	if len(left) != len(right) {
		return fmt.Errorf("byte-array length mismatch: %d != %d", len(left), len(right))
	}
	uapi, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	for i := range left {
		uapi.AssertIsEqual(left[i], right[i])
	}
	return nil
}

func bytesToFieldBE(api frontend.API, bytes []uints.U8) (frontend.Variable, error) {
	uapi, err := uints.NewBytes(api)
	if err != nil {
		return nil, err
	}
	var value frontend.Variable = 0
	for _, b := range bytes {
		value = api.Add(api.Mul(value, 256), uapi.Value(b))
	}
	return value, nil
}

func constants(value []byte) []uints.U8 {
	return uints.NewU8Array(value)
}

func nonZeroBytes(api frontend.API, bytes []uints.U8) error {
	uapi, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	values := make([]frontend.Variable, len(bytes))
	for i := range bytes {
		values[i] = uapi.Value(bytes[i])
	}
	api.AssertIsEqual(api.IsZero(sumVariables(api, values)), 0)
	return nil
}

func sumVariables(api frontend.API, values []frontend.Variable) frontend.Variable {
	switch len(values) {
	case 0:
		return 0
	case 1:
		return values[0]
	default:
		return api.Add(values[0], values[1], values[2:]...)
	}
}

func leBytesToVariable(api frontend.API, bytes []uints.U8) (frontend.Variable, error) {
	uapi, err := uints.NewBytes(api)
	if err != nil {
		return nil, err
	}
	var value frontend.Variable = 0
	coefficient := 1
	for _, b := range bytes {
		value = api.Add(value, api.Mul(uapi.Value(b), coefficient))
		coefficient *= 256
	}
	return value, nil
}
