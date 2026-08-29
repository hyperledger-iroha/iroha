package circuit

import (
	"testing"

	"github.com/consensys/gnark-crypto/ecc"
	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/uints"
	"github.com/consensys/gnark/test"
	"golang.org/x/crypto/blake2b"
)

type blake2bCircuit struct {
	Input    [260]uints.U8
	Length   frontend.Variable
	Expected [32]uints.U8
}

func (c *blake2bCircuit) Define(api frontend.API) error {
	digest, err := blake2b256(api, c.Input[:], c.Length)
	if err != nil {
		return err
	}
	uapi, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	for i := range digest {
		uapi.AssertIsEqual(digest[i], c.Expected[i])
	}
	return nil
}

func TestBlake2b256MatchesNative(t *testing.T) {
	for _, input := range [][]byte{
		[]byte("sccp"),
		make([]byte, 128),
		make([]byte, 129),
		make([]byte, 259),
	} {
		var witness blake2bCircuit
		copy(witness.Input[:], uints.NewU8Array(make([]byte, len(witness.Input))))
		copy(witness.Input[:], uints.NewU8Array(input))
		witness.Length = len(input)
		digest := blake2b.Sum256(input)
		copy(witness.Expected[:], uints.NewU8Array(digest[:]))
		if err := test.IsSolved(&blake2bCircuit{}, &witness, ecc.BN254.ScalarField()); err != nil {
			t.Fatalf("length %d: %v", len(input), err)
		}
	}
}

func TestBlake2b256RejectsNearModulusLength(t *testing.T) {
	var witness blake2bCircuit
	copy(witness.Input[:], uints.NewU8Array(make([]byte, len(witness.Input))))
	witness.Length = -1
	copy(witness.Expected[:], uints.NewU8Array(make([]byte, len(witness.Expected))))
	if err := test.IsSolved(&blake2bCircuit{}, &witness, ecc.BN254.ScalarField()); err == nil {
		t.Fatal("near-modulus BLAKE2b length bypassed the canonical buffer bound")
	}
}
