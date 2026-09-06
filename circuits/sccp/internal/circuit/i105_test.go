package circuit

import (
	"testing"

	"github.com/consensys/gnark-crypto/ecc"
	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/uints"
	"github.com/consensys/gnark/test"
)

type i105FixtureCircuit struct {
	Encoded       [maxI105SenderBytes]uints.U8
	EncodedLength frontend.Variable
	I105          I105Witness
}

func (c *i105FixtureCircuit) Define(api frontend.API) error {
	return constrainCanonicalTairaI105(api, c.Encoded[:], c.EncodedLength, &c.I105)
}

func i105FixtureAssignment(canonical []byte) (*i105FixtureCircuit, []byte, error) {
	witness := &i105FixtureCircuit{}
	zeroU8s(witness.Encoded[:])
	encoded, err := populateI105Witness(&witness.I105, canonical)
	if err != nil {
		return nil, nil, err
	}
	copyU8(witness.Encoded[:], encoded)
	witness.EncodedLength = len(encoded)
	return witness, encoded, nil
}

func TestTairaI105NativeKATMatchesRustAccountAddressVector(t *testing.T) {
	_, encoded, err := i105FixtureAssignment(katI105CanonicalAccount[:])
	if err != nil {
		t.Fatal(err)
	}
	expected := "testuﾛ1NkﾍﾒRAﾌﾎzLsﾉaPg53ﾊﾐp6SﾏﾅﾏcgNJsﾇzkjﾃUｽAﾗMUUXV1"
	if string(encoded) != expected {
		t.Fatalf("Taira I105 mismatch\n got: %s\nwant: %s", encoded, expected)
	}
	if len(encoded) == len([]rune(string(encoded))) {
		t.Fatal("I105 KAT must exercise a multibyte half-width-kana digit")
	}
}

func TestTairaI105CircuitKATAndMutationNegatives(t *testing.T) {
	definition := &i105FixtureCircuit{}
	positive, encoded, err := i105FixtureAssignment(katI105CanonicalAccount[:])
	if err != nil {
		t.Fatal(err)
	}
	if err := test.IsSolved(definition, positive, ecc.BN254.ScalarField()); err != nil {
		t.Fatalf("canonical Taira I105 KAT failed: %v", err)
	}

	t.Run("test sentinel", func(t *testing.T) {
		changed, _, err := i105FixtureAssignment(katI105CanonicalAccount[:])
		if err != nil {
			t.Fatal(err)
		}
		changed.Encoded[0] = uints.NewU8('s')
		assertI105Rejected(t, definition, changed)
	})

	t.Run("alphabet byte", func(t *testing.T) {
		changed, _, err := i105FixtureAssignment(katI105CanonicalAccount[:])
		if err != nil {
			t.Fatal(err)
		}
		changed.Encoded[4] = uints.NewU8('0')
		assertI105Rejected(t, definition, changed)
	})

	t.Run("kana continuation", func(t *testing.T) {
		changed, _, err := i105FixtureAssignment(katI105CanonicalAccount[:])
		if err != nil {
			t.Fatal(err)
		}
		kana := -1
		for index, value := range encoded {
			if value >= 0x80 {
				kana = index
				break
			}
		}
		if kana < 0 || kana+1 >= len(encoded) {
			t.Fatal("fixture contains no multibyte I105 digit")
		}
		changed.Encoded[kana+1] = uints.NewU8(byteValue(changed.Encoded[kana+1]) ^ 1)
		assertI105Rejected(t, definition, changed)
	})

	t.Run("checksum substitution with coherent digit", func(t *testing.T) {
		changed, current, err := i105FixtureAssignment(katI105CanonicalAccount[:])
		if err != nil {
			t.Fatal(err)
		}
		last := int(changed.I105.DigitCount.(int)) - 1
		changed.I105.Digits[last] = 1
		if current[len(current)-1] != '1' {
			t.Fatalf("fixture checksum terminator changed: %q", current[len(current)-1])
		}
		changed.Encoded[len(current)-1] = uints.NewU8('2')
		assertI105Rejected(t, definition, changed)
	})

	t.Run("nonminimal leading base105 zero", func(t *testing.T) {
		changed, _, err := i105FixtureAssignment(katI105CanonicalAccount[:])
		if err != nil {
			t.Fatal(err)
		}
		changed.I105.Digits[0] = 0
		changed.Encoded[4] = uints.NewU8('1')
		assertI105Rejected(t, definition, changed)
	})

	t.Run("canonical account structure", func(t *testing.T) {
		malformed := append([]byte(nil), katI105CanonicalAccount[:]...)
		malformed[0] = 0x0a
		changed, _, err := i105FixtureAssignment(malformed)
		if err != nil {
			t.Fatal(err)
		}
		assertI105Rejected(t, definition, changed)
	})

	t.Run("base105 arithmetic trace", func(t *testing.T) {
		changed, _, err := i105FixtureAssignment(katI105CanonicalAccount[:])
		if err != nil {
			t.Fatal(err)
		}
		changed.I105.Base105Trace[1][0] = uints.NewU8(byteValue(changed.I105.Base105Trace[1][0]) ^ 1)
		assertI105Rejected(t, definition, changed)
	})
}

func assertI105Rejected(t *testing.T, definition, witness frontend.Circuit) {
	t.Helper()
	if err := test.IsSolved(definition, witness, ecc.BN254.ScalarField()); err == nil {
		t.Fatal("malformed I105 assignment was accepted")
	}
}

func byteValue(value uints.U8) byte {
	switch typed := value.Val.(type) {
	case uint8:
		return typed
	case int:
		return byte(typed)
	default:
		panic("unexpected concrete byte witness type")
	}
}
