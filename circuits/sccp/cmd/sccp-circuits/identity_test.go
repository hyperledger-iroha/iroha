package main

import (
	"bytes"
	"crypto/sha256"
	"errors"
	"fmt"
	"io"
	"testing"

	"github.com/consensys/gnark-crypto/ecc"
	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/frontend/cs/r1cs"
)

type identityTestCircuit struct {
	Secret frontend.Variable
	Public frontend.Variable `gnark:",public"`
}

func (c *identityTestCircuit) Define(api frontend.API) error {
	api.AssertIsEqual(api.Mul(c.Secret, c.Secret), c.Public)
	return nil
}

func TestR1CSIdentityMatchesCanonicalBytesInBothFields(t *testing.T) {
	for _, curve := range []ecc.ID{ecc.BLS12_381, ecc.BN254} {
		t.Run(curve.String(), func(t *testing.T) {
			system, err := frontend.Compile(curve.ScalarField(), r1cs.NewBuilder, &identityTestCircuit{})
			if err != nil {
				t.Fatal(err)
			}
			var canonical bytes.Buffer
			reported, err := system.WriteTo(&canonical)
			if err != nil {
				t.Fatal(err)
			}
			wantHash := fmt.Sprintf("%x", sha256.Sum256(canonical.Bytes()))
			for iteration := 0; iteration < 2; iteration++ {
				if iteration != 0 {
					system, err = frontend.Compile(curve.ScalarField(), r1cs.NewBuilder, &identityTestCircuit{})
					if err != nil {
						t.Fatal(err)
					}
				}
				size, digest, err := r1csIdentity(system)
				if err != nil {
					t.Fatal(err)
				}
				if size != int64(canonical.Len()) || size != reported || digest != wantHash {
					t.Fatalf("identity differs from canonical bytes: size=%d digest=%s", size, digest)
				}
			}
		})
	}
}

type identityWriterTo func(io.Writer) (int64, error)

func (write identityWriterTo) WriteTo(writer io.Writer) (int64, error) {
	return write(writer)
}

func TestR1CSIdentityRejectsFailedEmptyAndMiscountedSerialization(t *testing.T) {
	serializationError := errors.New("serialization failure")
	for _, candidate := range []struct {
		name  string
		write identityWriterTo
	}{
		{"failure", func(writer io.Writer) (int64, error) {
			_, _ = writer.Write([]byte("partial"))
			return 7, serializationError
		}},
		{"empty", func(io.Writer) (int64, error) { return 0, nil }},
		{"miscounted", func(writer io.Writer) (int64, error) {
			_, _ = writer.Write([]byte("serialized"))
			return 11, nil
		}},
	} {
		t.Run(candidate.name, func(t *testing.T) {
			size, digest, err := r1csIdentity(candidate.write)
			if err == nil || size != 0 || digest != "" {
				t.Fatalf("invalid serialization emitted an identity: size=%d digest=%s error=%v", size, digest, err)
			}
			if candidate.name == "failure" && !errors.Is(err, serializationError) {
				t.Fatalf("serialization error was not preserved: %v", err)
			}
		})
	}
}
