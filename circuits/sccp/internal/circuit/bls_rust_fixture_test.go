package circuit

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"io"
	"math/big"
	"os"
	"slices"
	"testing"

	"github.com/consensys/gnark-crypto/ecc"
	bls12381 "github.com/consensys/gnark-crypto/ecc/bls12-381"
	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/algebra/emulated/sw_bls12381"
	"github.com/consensys/gnark/test"
)

const rustBLSVotePayloadLength = 378

type rustBLSNormalQCKAT struct {
	Finality FinalityWitness
	PoPs     PoPBatchWitness
}

func (c *rustBLSNormalQCKAT) Define(api frontend.API) error {
	api.AssertIsEqual(c.Finality.ValidatorCount, 4)
	api.AssertIsEqual(c.Finality.VoteSignaturePayloadLength, rustBLSVotePayloadLength)
	for index := range c.Finality.SignerBitmap {
		expected := 0
		if index < 3 {
			expected = 1
		}
		api.AssertIsEqual(c.Finality.SignerBitmap[index], expected)
	}
	for index := rustBLSVotePayloadLength; index < len(c.Finality.VoteSignaturePayload); index++ {
		api.AssertIsEqual(c.Finality.VoteSignaturePayload[index].Val, 0)
	}
	var encoding canonicalVoteEncoding
	for variant := range encoding.payloadVariants {
		encoding.payloadVariants[variant] = c.Finality.VoteSignaturePayload[:rustBLSVotePayloadLength]
		encoding.selectors[variant] = 0
	}
	encoding.selectors[0] = 1
	return constrainBLSNormalFinality(
		api,
		&c.Finality,
		constants([]byte("sccp:rust-sumeragi-v4-bls-normal-qc-kat:v1")),
		&c.PoPs,
		&encoding,
	)
}

type rustBLSNormalQCFixture struct {
	Schema                string   `json:"schema"`
	Source                string   `json:"source"`
	VotePreimageHex       string   `json:"vote_preimage_hex"`
	PublicKeysHex         []string `json:"public_keys_hex"`
	ProofsOfPossessionHex []string `json:"proofs_of_possession_hex"`
	SignerIndices         []int    `json:"signer_indices"`
	AggregateSignatureHex string   `json:"aggregate_signature_hex"`
}

func TestRustSumeragiBLSNormalQCKATInBothOuterFields(t *testing.T) {
	assignment := rustBLSNormalFixtureAssignment(t)
	definition := &rustBLSNormalQCKAT{}
	for _, outer := range []struct {
		name  string
		field *big.Int
	}{
		{name: "bn254", field: ecc.BN254.ScalarField()},
		{name: "bls12-381", field: ecc.BLS12_381.ScalarField()},
	} {
		outer := outer
		t.Run(outer.name, func(t *testing.T) {
			if err := test.IsSolved(definition, assignment, outer.field); err != nil {
				t.Fatalf("Rust Sumeragi BLS-normal QC failed in %s outer field: %v", outer.name, err)
			}
		})
	}
}

func rustBLSNormalFixtureAssignment(t *testing.T) *rustBLSNormalQCKAT {
	t.Helper()
	encoded, err := os.ReadFile("testdata/norito/sumeragi_v4_bls_normal_qc.json")
	if err != nil {
		t.Fatal(err)
	}
	var fixture rustBLSNormalQCFixture
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&fixture); err != nil {
		t.Fatal(err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		t.Fatalf("Rust BLS fixture has trailing content: %v", err)
	}
	if fixture.Schema != "sccp-rust-sumeragi-v4-bls-normal-qc-v1" ||
		fixture.Source != "iroha_sccp::sccp_exact_outbound_test_fixture_v1" ||
		len(fixture.PublicKeysHex) != 4 || len(fixture.ProofsOfPossessionHex) != 4 ||
		!slices.Equal(fixture.SignerIndices, []int{0, 1, 2}) {
		t.Fatalf("Rust BLS fixture header/shape mismatch: %#v", fixture)
	}
	votePreimage := decodeExactHex(t, fixture.VotePreimageHex, len(sumeragiVoteSignatureDomain)+rustBLSVotePayloadLength)
	if !bytes.Equal(votePreimage[:len(sumeragiVoteSignatureDomain)], sumeragiVoteSignatureDomain) {
		t.Fatal("Rust BLS fixture uses another vote signature domain")
	}

	assignment := &rustBLSNormalQCKAT{}
	initializeFinality(&assignment.Finality)
	initializePoPBatch(&assignment.PoPs)
	assignment.Finality.ValidatorCount = 4
	assignment.Finality.Epoch = 1
	assignment.Finality.EpochEndHeight = 10
	assignment.Finality.VoteSignaturePayloadLength = rustBLSVotePayloadLength
	copyU8(assignment.Finality.VoteSignaturePayload[:], votePreimage[len(sumeragiVoteSignatureDomain):])
	for index := 0; index < 3; index++ {
		assignment.Finality.SignerBitmap[index] = 1
	}
	for index := 0; index < 4; index++ {
		key := decodeExactHex(t, fixture.PublicKeysHex[index], 48)
		proof := decodeExactHex(t, fixture.ProofsOfPossessionHex[index], 96)
		copyU8(assignment.Finality.ValidatorPublicKeys[index][:], key)
		copyU8(assignment.PoPs.ValidatorPoPs[index][:], proof)
		set32(&assignment.Finality.ValidatorKeyHashes[index], sha256.Sum256(key))
		set32(&assignment.Finality.ValidatorPoPHashes[index], sha256.Sum256(proof))
		var proofPoint bls12381.G2Affine
		if consumed, err := proofPoint.SetBytes(proof); err != nil || consumed != len(proof) {
			t.Fatalf("decode Rust PoP %d: consumed=%d err=%v", index, consumed, err)
		}
		assignment.PoPs.ValidatorPoPPoints[index] = sw_bls12381.NewG2Affine(proofPoint)
	}
	aggregate := decodeExactHex(t, fixture.AggregateSignatureHex, 96)
	copyU8(assignment.Finality.AggregateSignature[:], aggregate)
	set32(&assignment.Finality.AggregateSignatureHash, sha256.Sum256(aggregate))
	var aggregatePoint bls12381.G2Affine
	if consumed, err := aggregatePoint.SetBytes(aggregate); err != nil || consumed != len(aggregate) {
		t.Fatalf("decode Rust aggregate: consumed=%d err=%v", consumed, err)
	}
	assignment.Finality.AggregateSignaturePoint = sw_bls12381.NewG2Affine(aggregatePoint)
	return assignment
}

func decodeExactHex(t *testing.T, value string, length int) []byte {
	t.Helper()
	decoded, err := hex.DecodeString(value)
	if err != nil {
		t.Fatal(err)
	}
	if len(decoded) != length {
		t.Fatalf("decoded fixture length = %d, want %d", len(decoded), length)
	}
	return decoded
}
