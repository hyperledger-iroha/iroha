package circuit

import (
	"encoding/binary"
	"encoding/hex"
	"os"
	"strings"
	"testing"

	"github.com/consensys/gnark-crypto/ecc"
	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/test"
)

type canonicalVoteFixtureCircuit struct {
	Finality FinalityWitness
}

func (c *canonicalVoteFixtureCircuit) Define(api frontend.API) error {
	_, err := constrainCanonicalCommitVote(api, &c.Finality)
	return err
}

func TestCanonicalCommitVoteMatchesRustNoritoFixture(t *testing.T) {
	payload := readRustCommitVotePayload(t)
	witness := voteFixtureAssignment(t, payload)
	actual := nativeCanonicalVoteSignaturePayload(witness.Finality)
	if string(actual) != string(payload) {
		t.Fatalf("canonical Go vote payload differs from Rust Encode::encode fixture\n got: %x\nwant: %x", actual, payload)
	}
	if len(actual) != 411 {
		t.Fatalf("base canonical vote payload length = %d, expected 411", len(actual))
	}
	definition := &canonicalVoteFixtureCircuit{}
	if err := test.IsSolved(definition, witness, ecc.BN254.ScalarField()); err != nil {
		t.Fatalf("Rust-generated canonical commit vote failed circuit constraints: %v", err)
	}
}

func TestCanonicalCommitVoteRejectsMalformedNorito(t *testing.T) {
	payload := readRustCommitVotePayload(t)
	definition := &canonicalVoteFixtureCircuit{}
	tests := []struct {
		name   string
		mutate func(*canonicalVoteFixtureCircuit)
	}{
		{
			name: "wrong round field length",
			mutate: func(w *canonicalVoteFixtureCircuit) {
				w.Finality.VoteSignaturePayload[3].Val = uint8(0x35)
			},
		},
		{
			name: "overlong compact length",
			mutate: func(w *canonicalVoteFixtureCircuit) {
				w.Finality.VoteSignaturePayload[3].Val = uint8(0xb4)
				w.Finality.VoteSignaturePayload[4].Val = uint8(0)
			},
		},
		{
			name: "wrong phase discriminant",
			mutate: func(w *canonicalVoteFixtureCircuit) {
				w.Finality.VoteSignaturePayload[110].Val = uint8(1)
			},
		},
		{
			name: "wrong required parent option flag",
			mutate: func(w *canonicalVoteFixtureCircuit) {
				w.Finality.VoteSignaturePayload[116].Val = uint8(0)
			},
		},
		{
			name: "wrong execution option flag",
			mutate: func(w *canonicalVoteFixtureCircuit) {
				w.Finality.VoteSignaturePayload[318].Val = uint8(1)
			},
		},
		{
			name: "trailing byte",
			mutate: func(w *canonicalVoteFixtureCircuit) {
				w.Finality.VoteSignaturePayload[len(payload)].Val = uint8(1)
			},
		},
		{
			name: "wrong declared length",
			mutate: func(w *canonicalVoteFixtureCircuit) {
				w.Finality.VoteSignaturePayloadLength = len(payload) + 1
			},
		},
	}
	for _, testCase := range tests {
		t.Run(testCase.name, func(t *testing.T) {
			witness := voteFixtureAssignment(t, payload)
			testCase.mutate(witness)
			if err := test.IsSolved(definition, witness, ecc.BN254.ScalarField()); err == nil {
				t.Fatal("malformed canonical vote encoding was accepted")
			}
		})
	}
}

func TestCanonicalCommitVoteCoversEveryExecutionOptionShape(t *testing.T) {
	definition := &canonicalVoteFixtureCircuit{}
	for variant := 0; variant < 8; variant++ {
		variant := variant
		t.Run(optionVariantName(variant), func(t *testing.T) {
			witness := genericVoteFixtureAssignment(t, variant)
			if err := test.IsSolved(definition, witness, ecc.BN254.ScalarField()); err != nil {
				t.Fatalf("canonical execution option variant %d failed: %v", variant, err)
			}
		})
	}
}

func readRustCommitVotePayload(t *testing.T) []byte {
	t.Helper()
	raw, err := os.ReadFile("testdata/norito/sumeragi_v4_commit_vote_signature_payload.hex")
	if err != nil {
		t.Fatal(err)
	}
	payload, err := hex.DecodeString(strings.TrimSpace(string(raw)))
	if err != nil {
		t.Fatal(err)
	}
	return payload
}

func voteFixtureAssignment(t *testing.T, payload []byte) *canonicalVoteFixtureCircuit {
	t.Helper()
	if len(payload) != 411 {
		t.Fatalf("Rust vote payload is %d bytes, expected 411", len(payload))
	}
	witness := &canonicalVoteFixtureCircuit{}
	initializeFinality(&witness.Finality)
	witness.Finality.Height = 1
	witness.Finality.RoundView = 9
	witness.Finality.ProposalView = 9
	copyU8(witness.Finality.HeightContextID[:], payload[6:38])
	copyU8(witness.Finality.SubjectParentBlockHash[:], payload[118:150])
	copyU8(witness.Finality.BlockHeaderHash[:], payload[151:183])
	copyU8(witness.Finality.SubjectPayloadHash[:], payload[184:216])
	execution := &witness.Finality.Execution
	copyU8(execution.ParentStateRoot[:], payload[219:251])
	copyU8(execution.PostStateRoot[:], payload[252:284])
	copyU8(execution.OrdinaryWritesRoot[:], payload[285:317])
	execution.NativeAMXApplicationManifestVer = int(binary.LittleEndian.Uint16(payload[325:327]))
	copyU8(execution.NativeAMXApplicationManifestRoot[:], payload[328:360])
	execution.ExecutedBlockWireLength = int(binary.LittleEndian.Uint64(payload[370:378]))
	copyU8(execution.ExecutedBlockWireHash[:], payload[379:411])
	setCanonicalVoteBytes(&witness.Finality, payload)
	return witness
}

func genericVoteFixtureAssignment(t *testing.T, variant int) *canonicalVoteFixtureCircuit {
	t.Helper()
	witness := &canonicalVoteFixtureCircuit{}
	initializeFinality(&witness.Finality)
	witness.Finality.Height = 17
	witness.Finality.RoundView = 3
	witness.Finality.ProposalView = 3
	set32(&witness.Finality.HeightContextID, nativeIrohaHash([]byte("generic context")))
	set32(&witness.Finality.SubjectParentBlockHash, nativeIrohaHash([]byte("generic parent")))
	set32(&witness.Finality.BlockHeaderHash, nativeIrohaHash([]byte("generic block")))
	set32(&witness.Finality.SubjectPayloadHash, nativeIrohaHash([]byte("generic payload")))
	populateKATExecutionCommitment(&witness.Finality.Execution, "generic execution")
	execution := &witness.Finality.Execution
	if variant&1 != 0 {
		execution.HasKagemushaTopUpRoot = 1
		execution.KagemushaTopUpCount = 1_000
		set32(&execution.KagemushaTopUpRoot, nativeIrohaHash([]byte("generic KAGEMUSHA top-up root")))
		count := make([]byte, 4)
		binary.LittleEndian.PutUint32(count, 1_000)
		ordinary := u8Array32(execution.OrdinaryWritesRoot)
		kagemushaTopUp := u8Array32(execution.KagemushaTopUpRoot)
		preimage := append([]byte("iroha:kagemusha:v1:post-state-root"), 0)
		preimage = append(preimage, count...)
		preimage = append(preimage, ordinary[:]...)
		preimage = append(preimage, kagemushaTopUp[:]...)
		set32(&execution.PostStateRoot, nativeIrohaHash(preimage))
	}
	if variant&2 != 0 {
		execution.HasLaneFinalityManifest = 1
		execution.LaneFinalityManifestLeafCount = 1
		set32(&execution.LaneFinalityManifestRoot, nativeIrohaHash([]byte("generic lane root")))
	}
	if variant&4 != 0 {
		execution.HasMergeCarrier = 1
		execution.MergeCarrierVersion = mergeCarrierVersion
		set32(&execution.MergeCarrierEntryHash, nativeIrohaHash([]byte("generic merge entry")))
	}
	payload := nativeCanonicalVoteSignaturePayload(witness.Finality)
	setCanonicalVoteBytes(&witness.Finality, payload)
	return witness
}

func setCanonicalVoteBytes(finality *FinalityWitness, payload []byte) {
	finality.VoteSignaturePayloadLength = len(payload)
	zeroU8s(finality.VoteSignaturePayload[:])
	copyU8(finality.VoteSignaturePayload[:], payload)
	preimage := append(append([]byte(nil), sumeragiVoteSignatureDomain...), payload...)
	set32(&finality.VotePreimageHash, nativeIrohaHash(preimage))
}

func optionVariantName(variant int) string {
	parts := make([]string, 0, 3)
	for bit, name := range []string{"kagemusha_top_up", "lane", "merge"} {
		if variant&(1<<bit) != 0 {
			parts = append(parts, name)
		}
	}
	if len(parts) == 0 {
		return "all-absent"
	}
	return strings.Join(parts, "+")
}
