package circuit

import (
	"bytes"
	"fmt"
	"testing"

	"github.com/consensys/gnark-crypto/ecc"
	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/uints"
	"github.com/consensys/gnark/test"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

// exactArtifactTestCircuit isolates the canonical V2FinalityArtifact byte
// equality from BLS pairing cost. Full message/epoch KATs separately prove
// that this equality is composed with the aggregate-signature constraints.
type exactArtifactTestCircuit struct {
	Finality FinalityWitness
}

func (c *exactArtifactTestCircuit) Define(api frontend.API) error {
	digest, err := canonicalFinalityArtifactHash(api, &c.Finality)
	if err != nil {
		return err
	}
	return assertBytesEqual(api, digest[:], c.Finality.FinalityArtifactHash[:])
}

func TestExactFinalityArtifactRejectsCanonicalByteSubstitutions(t *testing.T) {
	cfg, err := profile.ByID("sccp-final-v1-ethereum-mainnet-message")
	if err != nil {
		t.Fatal(err)
	}
	_, message, err := MessageKAT(cfg)
	if err != nil {
		t.Fatal(err)
	}
	definition := &exactArtifactTestCircuit{}
	positive := &exactArtifactTestCircuit{Finality: message.Finality}
	field := ecc.BN254.ScalarField()
	if err := test.IsSolved(definition, positive, field); err != nil {
		t.Fatalf("exact artifact positive failed: %v", err)
	}

	t.Run("self-hashed noncanonical byte", func(t *testing.T) {
		candidate := &exactArtifactTestCircuit{Finality: message.Finality}
		length := candidate.Finality.FinalityArtifactLength.(int)
		middle := length / 2
		candidate.Finality.FinalityArtifactBytes[middle].Val =
			candidate.Finality.FinalityArtifactBytes[middle].Val.(uint8) ^ 1
		refreshRawArtifactHash(&candidate.Finality)
		if err := test.IsSolved(definition, candidate, field); err == nil {
			t.Fatal("a self-hashed noncanonical finality-artifact byte was accepted")
		}
	})

	t.Run("shortened canonical prefix", func(t *testing.T) {
		candidate := &exactArtifactTestCircuit{Finality: message.Finality}
		candidate.Finality.FinalityArtifactLength =
			candidate.Finality.FinalityArtifactLength.(int) - 1
		refreshRawArtifactHash(&candidate.Finality)
		if err := test.IsSolved(definition, candidate, field); err == nil {
			t.Fatal("a self-hashed truncated finality artifact was accepted")
		}
	})

	t.Run("nonzero trailing byte", func(t *testing.T) {
		candidate := &exactArtifactTestCircuit{Finality: message.Finality}
		length := candidate.Finality.FinalityArtifactLength.(int)
		candidate.Finality.FinalityArtifactBytes[length].Val = uint8(1)
		if err := test.IsSolved(definition, candidate, field); err == nil {
			t.Fatal("nonzero padding after the canonical artifact length was accepted")
		}
	})

	for _, mutation := range []struct {
		name   string
		mutate func(*FinalityWitness)
	}{
		{
			name: "full parent CommitQC aggregate",
			mutate: func(finality *FinalityWitness) {
				finality.ParentAggregateSignature[17].Val =
					finality.ParentAggregateSignature[17].Val.(uint8) ^ 1
			},
		},
		{
			name: "full parent CommitQC signer",
			mutate: func(finality *FinalityWitness) {
				finality.ParentSignerIndices[2] = 4
			},
		},
		{
			name: "durable current-roster PoP",
			mutate: func(finality *FinalityWitness) {
				finality.ValidatorPoPs[2][31].Val =
					finality.ValidatorPoPs[2][31].Val.(uint8) ^ 1
			},
		},
		{
			name: "data-availability layout",
			mutate: func(finality *FinalityWitness) {
				finality.DALayout.ParityShards = finality.DALayout.ParityShards.(int) + 1
			},
		},
	} {
		mutation := mutation
		t.Run(mutation.name, func(t *testing.T) {
			candidate := &exactArtifactTestCircuit{Finality: message.Finality}
			mutation.mutate(&candidate.Finality)
			if err := test.IsSolved(definition, candidate, field); err == nil {
				t.Fatalf("structured %s substitution was accepted under unchanged exact bytes", mutation.name)
			}
		})
	}
}

func refreshRawArtifactHash(finality *FinalityWitness) {
	length := finality.FinalityArtifactLength.(int)
	bytes := make([]byte, length)
	for index := range bytes {
		bytes[index] = finality.FinalityArtifactBytes[index].Val.(uint8)
	}
	set32(&finality.FinalityArtifactHash, nativeIrohaHash(bytes))
}

func TestNativeFinalityEncodingCommitsEveryStructuredFieldFamily(t *testing.T) {
	messageCfg, err := profile.ByID("sccp-final-v1-ethereum-mainnet-message")
	if err != nil {
		t.Fatal(err)
	}
	_, message, err := MessageKAT(messageCfg)
	if err != nil {
		t.Fatal(err)
	}
	base := message.Finality
	baseIdentity := nativeHeightContextIdentity(base)
	baseArtifact := nativeFinalityArtifactBytes(base)
	flip := func(value *uints.U8) { value.Val = value.Val.(uint8) ^ 1 }
	mutations := []struct {
		name           string
		changesContext bool
		mutate         func(*FinalityWitness)
	}{
		{"height", true, func(value *FinalityWitness) { value.Height = value.Height.(int) + 1 }},
		{"epoch", true, func(value *FinalityWitness) { value.Epoch = value.Epoch.(int) + 1 }},
		{"epoch end", true, func(value *FinalityWitness) { value.EpochEndHeight = value.EpochEndHeight.(int) + 1 }},
		{"mode", true, func(value *FinalityWitness) { value.Mode = value.Mode.(int) + 1 }},
		{"parent context", true, func(value *FinalityWitness) { flip(&value.ParentContextID[0]) }},
		{"parent height", true, func(value *FinalityWitness) { value.ParentHeight = value.ParentHeight.(int) + 1 }},
		{"parent grandparent", true, func(value *FinalityWitness) { flip(&value.ParentSubjectParentBlockHash[0]) }},
		{"parent block", true, func(value *FinalityWitness) { flip(&value.ParentBlockHash[0]) }},
		{"parent payload", true, func(value *FinalityWitness) { flip(&value.ParentPayloadHash[0]) }},
		{"parent execution", true, func(value *FinalityWitness) { flip(&value.ParentExecution.PostStateRoot[0]) }},
		{"roster key", true, func(value *FinalityWitness) { flip(&value.ValidatorPublicKeys[0][9]) }},
		{"nexus context", true, func(value *FinalityWitness) { flip(&value.NexusAMXContextHash[0]) }},
		{"execution policy", true, func(value *FinalityWitness) { flip(&value.ExecutionPolicyHash[0]) }},
		{"DA encoding fields", true, func(value *FinalityWitness) { value.DALayout.DataShards = value.DALayout.DataShards.(int) + 1 }},
		{"leader seed", true, func(value *FinalityWitness) { flip(&value.LeaderSeed[0]) }},
		{"current subject payload", false, func(value *FinalityWitness) { flip(&value.SubjectPayloadHash[0]) }},
		{"current QC round", false, func(value *FinalityWitness) { value.RoundView = value.RoundView.(int) + 1 }},
		{"current QC proposal round", false, func(value *FinalityWitness) { value.ProposalView = value.ProposalView.(int) + 1 }},
		{"current execution", false, func(value *FinalityWitness) { flip(&value.Execution.PostStateRoot[0]) }},
		{"current QC signer", false, func(value *FinalityWitness) { value.SignerIndices[2] = 4 }},
		{"current QC aggregate", false, func(value *FinalityWitness) { flip(&value.AggregateSignature[9]) }},
		{"durable current PoP", false, func(value *FinalityWitness) { flip(&value.ValidatorPoPs[1][13]) }},
		{"full parent QC round", false, func(value *FinalityWitness) { value.ParentRoundView = value.ParentRoundView.(int) + 1 }},
		{"full parent QC proposal round", false, func(value *FinalityWitness) { value.ParentProposalView = value.ParentProposalView.(int) + 1 }},
		{"full parent QC signer", false, func(value *FinalityWitness) { value.ParentSignerIndices[2] = 4 }},
		{"full parent QC aggregate", false, func(value *FinalityWitness) { flip(&value.ParentAggregateSignature[11]) }},
	}
	for _, mutation := range mutations {
		candidate := base
		mutation.mutate(&candidate)
		if bytes.Equal(baseArtifact, nativeFinalityArtifactBytes(candidate)) {
			t.Errorf("artifact encoding omits %s", mutation.name)
		}
		if mutation.changesContext && baseIdentity == nativeHeightContextIdentity(candidate) {
			t.Errorf("HeightContext identity omits %s", mutation.name)
		}
	}

	epochCfg, err := profile.ByID("sccp-final-v1-ethereum-mainnet-epoch-anchor-update")
	if err != nil {
		t.Fatal(err)
	}
	_, epoch, err := EpochKAT(epochCfg)
	if err != nil {
		t.Fatal(err)
	}
	boundary := epoch.BoundaryFinality
	boundaryIdentity := nativeHeightContextIdentity(boundary)
	boundaryArtifact := nativeFinalityArtifactBytes(boundary)
	for _, mutation := range []struct {
		name   string
		mutate func(*FinalityWitness)
	}{
		{"next epoch", func(value *FinalityWitness) { value.NextEpochSnapshot.Epoch = value.NextEpochSnapshot.Epoch.(int) + 1 }},
		{"next epoch end", func(value *FinalityWitness) {
			value.NextEpochSnapshot.EpochEndHeight = value.NextEpochSnapshot.EpochEndHeight.(int) + 1
		}},
		{"next mode", func(value *FinalityWitness) { value.NextEpochSnapshot.Mode = value.NextEpochSnapshot.Mode.(int) + 1 }},
		{"next roster key", func(value *FinalityWitness) { flip(&value.NextEpochSnapshot.ValidatorPublicKeys[0][7]) }},
		{"next roster PoP", func(value *FinalityWitness) { flip(&value.NextEpochSnapshot.ValidatorPoPs[0][17]) }},
		{"next leader seed", func(value *FinalityWitness) { flip(&value.NextEpochSnapshot.LeaderSeed[0]) }},
	} {
		candidate := boundary
		mutation.mutate(&candidate)
		if boundaryIdentity == nativeHeightContextIdentity(candidate) {
			t.Errorf("HeightContext identity omits %s", mutation.name)
		}
		if bytes.Equal(boundaryArtifact, nativeFinalityArtifactBytes(candidate)) {
			t.Errorf("artifact encoding omits %s", mutation.name)
		}
	}
}

func TestFinalityArtifactBoundCoversEveryClosedWireShape(t *testing.T) {
	cfg, err := profile.ByID("sccp-final-v1-ethereum-mainnet-message")
	if err != nil {
		t.Fatal(err)
	}
	_, message, err := MessageKAT(cfg)
	if err != nil {
		t.Fatal(err)
	}
	maximum := 0
	maximumShape := ""
	maximumByBoundary := map[bool]int{}
	for _, count := range canonicalCommitteeSizes {
		for _, boundary := range []bool{false, true} {
			candidate := message.Finality
			candidate.ValidatorCount = count
			candidate.ParentSignerCount = 21
			if boundary {
				candidate.HasNextEpochSnapshot = 1
				candidate.EpochEndHeight = candidate.Height
				candidate.NextEpochSnapshot.Epoch = candidate.Epoch.(int) + 1
				candidate.NextEpochSnapshot.EpochEndHeight = candidate.Height.(int) + 100
				candidate.NextEpochSnapshot.Mode = sumeragiModeNPoS
				candidate.NextEpochSnapshot.ValidatorCount = 31
			} else {
				candidate.HasNextEpochSnapshot = 0
				candidate.NextEpochSnapshot.Epoch = 0
				candidate.NextEpochSnapshot.EpochEndHeight = 0
				candidate.NextEpochSnapshot.Mode = 0
				candidate.NextEpochSnapshot.ValidatorCount = 0
			}
			for _, execution := range []*ExecutionCommitmentWitness{
				&candidate.Execution,
				&candidate.ParentExecution,
			} {
				execution.HasOfflineCashTopUpRoot = 1
				execution.HasLaneFinalityManifest = 1
				execution.HasMergeCarrier = 1
			}
			encoded := nativeFinalityArtifactBytes(candidate)
			if len(encoded) > maximumByBoundary[boundary] {
				maximumByBoundary[boundary] = len(encoded)
			}
			if len(encoded) > maximum {
				maximum = len(encoded)
				maximumShape = fmt.Sprintf("roster=%d,boundary=%t", count, boundary)
			}
		}
	}
	t.Logf("largest non-boundary=%d boundary=%d", maximumByBoundary[false], maximumByBoundary[true])
	if maximum != maxFinalityArtifactBytes {
		t.Fatalf("largest closed artifact shape %s is %d bytes, exact bound %d", maximumShape, maximum, maxFinalityArtifactBytes)
	}
	t.Logf("largest closed artifact shape %s is %d bytes", maximumShape, maximum)
}
