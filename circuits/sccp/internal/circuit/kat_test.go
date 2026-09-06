package circuit

import (
	"bytes"
	"crypto/sha256"
	"encoding/json"
	"fmt"
	"io"
	"math/big"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"github.com/consensys/gnark-crypto/ecc"
	bls12381 "github.com/consensys/gnark-crypto/ecc/bls12-381"
	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/algebra/emulated/sw_bls12381"
	"github.com/consensys/gnark/std/math/uints"
	"github.com/consensys/gnark/test"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

func TestCheckedInPublicKATs(t *testing.T) {
	for _, cfg := range profile.All() {
		cfg := cfg
		t.Run(cfg.ID, func(t *testing.T) {
			expected, err := PublicKAT(cfg)
			if err != nil {
				t.Fatal(err)
			}
			path := filepath.Join("testdata", "kats", cfg.ID+".json")
			encoded, err := os.ReadFile(path)
			if err != nil {
				t.Fatal(err)
			}
			var actual KnownAnswerVector
			decoder := json.NewDecoder(bytes.NewReader(encoded))
			decoder.DisallowUnknownFields()
			if err := decoder.Decode(&actual); err != nil {
				t.Fatal(err)
			}
			if err := decoder.Decode(&struct{}{}); err != io.EOF {
				t.Fatalf("KAT %s has trailing JSON content: %v", path, err)
			}
			if !reflect.DeepEqual(actual, expected) {
				t.Fatalf("checked-in KAT %s is stale", path)
			}
		})
	}
}

func TestCheckedInKATInventoryAuthenticatesEveryVector(t *testing.T) {
	type entry struct {
		Profile string `json:"profile"`
		Path    string `json:"path"`
		SHA256  string `json:"sha256"`
	}
	var inventory struct {
		Schema  string  `json:"schema"`
		Version int     `json:"version"`
		Vectors []entry `json:"vectors"`
	}
	encoded, err := os.ReadFile(filepath.Join("..", "..", "manifests", "kat-inventory-final-v1.json"))
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&inventory); err != nil {
		t.Fatal(err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		t.Fatalf("KAT inventory has trailing JSON content: %v", err)
	}
	configs := profile.All()
	if inventory.Schema != "sccp-circuit-kat-inventory-final-v1" ||
		inventory.Version != 1 || len(inventory.Vectors) != len(configs) {
		t.Fatalf("KAT inventory header/length mismatch: %#v", inventory)
	}
	for index, cfg := range configs {
		vector := inventory.Vectors[index]
		expectedPath := filepath.ToSlash(filepath.Join("internal", "circuit", "testdata", "kats", cfg.ID+".json"))
		if vector.Profile != cfg.ID || vector.Path != expectedPath {
			t.Fatalf("KAT inventory entry %d does not match profile %q: %#v", index, cfg.ID, vector)
		}
		contents, err := os.ReadFile(filepath.Join("..", "..", filepath.FromSlash(vector.Path)))
		if err != nil {
			t.Fatal(err)
		}
		digest := sha256.Sum256(contents)
		if vector.SHA256 != fmt.Sprintf("%x", digest) {
			t.Fatalf("KAT inventory digest mismatch for %q", cfg.ID)
		}
	}
}

func TestCheckedInConstraintCountInventoryCoversEveryProfile(t *testing.T) {
	type toolchain struct {
		Go         string `json:"go"`
		Gnark      string `json:"gnark"`
		Builder    string `json:"builder"`
		ModuleMode string `json:"module_mode"`
		Network    string `json:"network"`
	}
	type entry struct {
		Profile     string        `json:"profile"`
		Role        profile.Role  `json:"role"`
		OuterCurve  profile.Curve `json:"outer_curve"`
		Constraints int           `json:"constraints"`
		KATSHA256   string        `json:"kat_sha256"`
	}
	type artifactState struct {
		R1CSIdentitiesCurrent      bool     `json:"r1cs_identities_current"`
		InvalidatedArtifactRoles   []string `json:"invalidated_artifact_roles"`
		FreshClosureRequired       []string `json:"fresh_closure_required"`
		ProfilesRequiringFreshR1CS []string `json:"profiles_requiring_fresh_r1cs"`
		Reason                     string   `json:"reason"`
	}
	var inventory struct {
		Schema               string        `json:"schema"`
		Version              int           `json:"version"`
		Toolchain            toolchain     `json:"toolchain"`
		DefinitionState      string        `json:"definition_state"`
		Profiles             []entry       `json:"profiles"`
		ArtifactState        artifactState `json:"artifact_state"`
		ProductionAdmissible bool          `json:"production_admissible"`
		Note                 string        `json:"note"`
	}
	encoded, err := os.ReadFile(filepath.Join("..", "..", "manifests", "constraint-counts-final-v1.json"))
	if err != nil {
		t.Fatal(err)
	}
	decoder := json.NewDecoder(bytes.NewReader(encoded))
	decoder.DisallowUnknownFields()
	if err := decoder.Decode(&inventory); err != nil {
		t.Fatal(err)
	}
	if err := decoder.Decode(&struct{}{}); err != io.EOF {
		t.Fatalf("constraint-count inventory has trailing JSON content: %v", err)
	}
	configs := profile.All()
	if inventory.Schema != "sccp-circuit-constraint-counts-final-v1" ||
		inventory.Version != 1 || len(inventory.Profiles) != len(configs) ||
		inventory.ProductionAdmissible || inventory.Note == "" {
		t.Fatalf("constraint-count inventory header/length mismatch: %#v", inventory)
	}
	if inventory.Toolchain != (toolchain{
		Go:         "go1.25.7",
		Gnark:      "v0.16.3",
		Builder:    "frontend.Compile+r1cs.NewBuilder",
		ModuleMode: "vendor",
		Network:    "none",
	}) {
		t.Fatalf("constraint-count inventory toolchain drift: %#v", inventory.Toolchain)
	}
	if inventory.DefinitionState != "final-v1-wire-identifiers-aligned" {
		t.Fatalf("unexpected constraint-count definition state %q", inventory.DefinitionState)
	}
	state := inventory.ArtifactState
	expectedInvalidatedRoles := []string{
		"r1cs",
		"phase2_transcript",
		"proving_key",
		"verifying_key",
		"fixed_key_verifier",
		"destination_deployment",
	}
	expectedFreshClosure := []string{
		"circuit_specific_phase2_mpc",
		"semantic_cryptographic_audit",
		"reproducibility_ceremony_audit",
		"destination_integration_audit",
	}
	expectedFreshR1CS := []string{
		"sccp-final-v1-bsc-mainnet-message",
		"sccp-final-v1-ethereum-mainnet-message",
		"sccp-final-v1-ton-mainnet-epoch-anchor-update",
		"sccp-final-v1-ton-mainnet-message",
		"sccp-final-v1-tron-mainnet-epoch-anchor-update",
		"sccp-final-v1-tron-mainnet-message",
	}
	if state.R1CSIdentitiesCurrent || state.Reason == "" ||
		!reflect.DeepEqual(state.InvalidatedArtifactRoles, expectedInvalidatedRoles) ||
		!reflect.DeepEqual(state.FreshClosureRequired, expectedFreshClosure) ||
		!reflect.DeepEqual(state.ProfilesRequiringFreshR1CS, expectedFreshR1CS) {
		t.Fatalf("wire-alignment invalidation policy drift: %#v", state)
	}
	for index, cfg := range configs {
		entry := inventory.Profiles[index]
		if entry.Profile != cfg.ID || entry.Role != cfg.Role || entry.OuterCurve != cfg.Curve {
			t.Fatalf("constraint-count entry %d does not match profile %q: %#v", index, cfg.ID, entry)
		}
		if entry.Constraints <= 0 {
			t.Fatalf("constraint-count entry %q is not positive", cfg.ID)
		}
		contents, err := os.ReadFile(filepath.Join("testdata", "kats", cfg.ID+".json"))
		if err != nil {
			t.Fatal(err)
		}
		digest := sha256.Sum256(contents)
		if entry.KATSHA256 != fmt.Sprintf("%x", digest) {
			t.Fatalf("constraint-count inventory KAT digest mismatch for %q", cfg.ID)
		}
	}
}

func TestEightProfileKATsAndPublicMutationNegatives(t *testing.T) {
	for _, cfg := range profile.All() {
		cfg := cfg
		t.Run(cfg.ID, func(t *testing.T) {
			var definition, witness frontend.Circuit
			var err error
			if cfg.Role == profile.Message {
				definition, witness, err = MessageKAT(cfg)
			} else {
				definition, witness, err = EpochKAT(cfg)
			}
			if err != nil {
				t.Fatal(err)
			}
			field := ecc.BN254.ScalarField()
			if cfg.Curve == profile.BLS12381 {
				field = ecc.BLS12_381.ScalarField()
			}
			if err := test.IsSolved(definition, witness, field); err != nil {
				t.Fatalf("positive KAT failed: %v", err)
			}
			// Every public role is independently constrained. Mutating any one of
			// the eleven fields must invalidate the witness.
			for signal := 0; signal < 11; signal++ {
				var changed frontend.Circuit
				if cfg.Role == profile.Message {
					_, candidate, err := MessageKAT(cfg)
					if err != nil {
						t.Fatal(err)
					}
					candidate.PublicSignals[signal] = new(big.Int).Add(candidate.PublicSignals[signal].(*big.Int), big.NewInt(1))
					changed = candidate
				} else {
					_, candidate, err := EpochKAT(cfg)
					if err != nil {
						t.Fatal(err)
					}
					candidate.PublicSignals[signal] = new(big.Int).Add(candidate.PublicSignals[signal].(*big.Int), big.NewInt(1))
					changed = candidate
				}
				if err := test.IsSolved(definition, changed, field); err == nil {
					t.Fatalf("public signal %d mutation was accepted", signal)
				}
			}
		})
	}
}

func TestStructuredWitnessesRejectDuplicateValidatorKeys(t *testing.T) {
	messageConfig, err := profile.ByID("sccp-final-v1-ethereum-mainnet-message")
	if err != nil {
		t.Fatal(err)
	}
	messageDefinition, messageWitness, err := MessageKAT(messageConfig)
	if err != nil {
		t.Fatal(err)
	}
	messageWitness.Finality.ValidatorKeyHashes[1] = messageWitness.Finality.ValidatorKeyHashes[0]
	if err := test.IsSolved(messageDefinition, messageWitness, ecc.BN254.ScalarField()); err == nil {
		t.Fatal("message finality accepted duplicate active validator keys")
	}

	epochConfig, err := profile.ByID("sccp-final-v1-ton-mainnet-epoch-anchor-update")
	if err != nil {
		t.Fatal(err)
	}
	epochDefinition, epochWitness, err := EpochKAT(epochConfig)
	if err != nil {
		t.Fatal(err)
	}
	epochWitness.Snapshot.ValidatorKeyHashes[1] = epochWitness.Snapshot.ValidatorKeyHashes[0]
	if err := test.IsSolved(epochDefinition, epochWitness, ecc.BLS12_381.ScalarField()); err == nil {
		t.Fatal("epoch snapshot accepted duplicate active validator keys")
	}
}

func TestMessageRejectsNearModulusMerkleDepth(t *testing.T) {
	cfg, err := profile.ByID("sccp-final-v1-ethereum-mainnet-message")
	if err != nil {
		t.Fatal(err)
	}
	definition, witness, err := MessageKAT(cfg)
	if err != nil {
		t.Fatal(err)
	}
	witness.MerkleDepth = -1
	if err := test.IsSolved(definition, witness, ecc.BN254.ScalarField()); err == nil {
		t.Fatal("near-modulus Merkle depth bypassed the canonical proof-length bound")
	}
}

type currentAnchorAuthorizationTestCircuit struct {
	Current      AnchorWitness
	Boundary     FinalityWitness
	ExpectedHash [32]uints.U8
}

func (c *currentAnchorAuthorizationTestCircuit) Define(api frontend.API) error {
	if err := constrainAnchorHash(api, &c.Current, c.ExpectedHash); err != nil {
		return err
	}
	return constrainCurrentAnchorAuthorization(api, &c.Current, &c.Boundary)
}

func TestEpochAnchorSupportsConsecutiveAuthenticatedAdvances(t *testing.T) {
	cfg, err := profile.ByID("sccp-final-v1-ethereum-mainnet-epoch-anchor-update")
	if err != nil {
		t.Fatal(err)
	}
	definition, first, err := EpochKAT(cfg)
	if err != nil {
		t.Fatal(err)
	}
	field := ecc.BN254.ScalarField()
	if err := test.IsSolved(definition, first, field); err != nil {
		t.Fatalf("first authenticated epoch advance failed: %v", err)
	}

	second := secondEpochKATAssignment(cfg, &first.NextAnchor)
	if first.RawSignals[1] != second.RawSignals[0] {
		t.Fatal("first next-anchor output is not the second current-anchor input")
	}
	if err := test.IsSolved(definition, second, field); err != nil {
		t.Fatalf("second authenticated epoch advance failed: %v", err)
	}
}

func secondEpochKATAssignment(cfg profile.Config, current *AnchorWitness) *EpochAnchorCircuit {
	second := &EpochAnchorCircuit{cfg: cfg}
	initializeEpochWitness(second)
	second.Snapshot.CurrentEpoch = 9
	second.Snapshot.NextEpoch = 10
	second.Snapshot.NextEpochEndHeight = 300
	second.Snapshot.Mode = sumeragiModeNPoS
	second.Snapshot.ValidatorCount = 4
	populateKATFinality(
		&second.Finality,
		&second.NextRosterPoPs,
		cfg.ID+":successor-2",
		201,
		10,
		300,
		nil,
	)
	refreshKATEpochTransition(cfg, second, katEpochTransition{
		boundaryHeight: 200,
		boundaryEpoch:  9,
		boundaryEnd:    200,
		// The first successor roster is the second boundary's exact current
		// roster. Its aggregate QC is independently re-signed for height 200.
		boundaryScope:  cfg.ID + ":successor",
		successorScope: cfg.ID + ":successor-2",
		currentAnchor:  current,
	})
	return second
}

func TestCurrentAnchorAuthorizationRejectsStaleAndMismatchedTransitions(t *testing.T) {
	cfg, err := profile.ByID("sccp-final-v1-ethereum-mainnet-epoch-anchor-update")
	if err != nil {
		t.Fatal(err)
	}
	_, first, err := EpochKAT(cfg)
	if err != nil {
		t.Fatal(err)
	}
	second := secondEpochKATAssignment(cfg, &first.NextAnchor)
	field := ecc.BN254.ScalarField()
	base := &currentAnchorAuthorizationTestCircuit{
		Current:      second.CurrentAnchor,
		Boundary:     second.BoundaryFinality,
		ExpectedHash: second.RawSignals[0],
	}
	authorizationDefinition := &currentAnchorAuthorizationTestCircuit{}
	if err := test.IsSolved(authorizationDefinition, base, field); err != nil {
		t.Fatalf("retained successor anchor did not authorize its epoch boundary: %v", err)
	}

	for _, mutation := range []struct {
		name   string
		mutate func(*currentAnchorAuthorizationTestCircuit)
	}{
		{
			name: "stale anchor after boundary",
			mutate: func(candidate *currentAnchorAuthorizationTestCircuit) {
				candidate.Current.CheckpointHeight = 201
				refreshAuthorizationAnchorHash(candidate)
			},
		},
		{
			name: "wrong authorized roster",
			mutate: func(candidate *currentAnchorAuthorizationTestCircuit) {
				candidate.Current.RosterCommitment[0].Val =
					candidate.Current.RosterCommitment[0].Val.(uint8) ^ 1
				refreshAuthorizationAnchorHash(candidate)
			},
		},
		{
			name: "wrong boundary epoch",
			mutate: func(candidate *currentAnchorAuthorizationTestCircuit) {
				candidate.Boundary.Epoch = candidate.Boundary.Epoch.(int) + 1
			},
		},
		{
			name: "non-boundary height",
			mutate: func(candidate *currentAnchorAuthorizationTestCircuit) {
				candidate.Boundary.Height = candidate.Boundary.Height.(int) - 1
			},
		},
		{
			name: "same-height equivocation",
			mutate: func(candidate *currentAnchorAuthorizationTestCircuit) {
				candidate.Current.CheckpointHeight = candidate.Boundary.Height
				refreshAuthorizationAnchorHash(candidate)
			},
		},
	} {
		mutation := mutation
		t.Run(mutation.name, func(t *testing.T) {
			candidate := *base
			mutation.mutate(&candidate)
			if err := test.IsSolved(authorizationDefinition, &candidate, field); err == nil {
				t.Fatalf("%s was accepted", mutation.name)
			}
		})
	}
}

func refreshAuthorizationAnchorHash(candidate *currentAnchorAuthorizationTestCircuit) {
	roster := u8Array32(candidate.Current.RosterCommitment)
	block := u8Array32(candidate.Current.CheckpointBlockHash)
	context := u8Array32(candidate.Current.CheckpointContextID)
	artifact := u8Array32(candidate.Current.CheckpointFinalityArtifactHash)
	digest := nativeAnchorHash(
		concreteKATUint64(candidate.Current.CheckpointHeight),
		concreteKATUint64(candidate.Current.Epoch),
		concreteKATUint64(candidate.Current.EpochEndHeight),
		roster,
		block,
		context,
		artifact,
	)
	set32(&candidate.ExpectedHash, digest)
}

func TestBLSBatchRejectsCancellationAndRosterAmbiguity(t *testing.T) {
	epochConfig, err := profile.ByID("sccp-final-v1-ethereum-mainnet-epoch-anchor-update")
	if err != nil {
		t.Fatal(err)
	}
	field := ecc.BN254.ScalarField()

	t.Run("offsetting forged pops", func(t *testing.T) {
		definition, witness, err := EpochKAT(epochConfig)
		if err != nil {
			t.Fatal(err)
		}
		scope := epochConfig.ID + ":successor"
		keys := deterministicBLSRoster(scope, 4)
		delta := nativeBLSSignPoint(big.NewInt(17), []byte("sccp:forged-pop-cancellation-negative:v1"))
		proof0 := keys[0].proof
		proof0.Add(&proof0, &delta)
		proof1 := keys[1].proof
		proof1.Sub(&proof1, &delta)
		setFinalityPoP(&witness.Finality, &witness.NextRosterPoPs, 0, proof0)
		setFinalityPoP(&witness.Finality, &witness.NextRosterPoPs, 1, proof1)
		refreshKATEpochDerived(epochConfig, witness)
		if err := test.IsSolved(definition, witness, field); err == nil {
			t.Fatal("transcript-randomized batch accepted two individually invalid PoPs whose unweighted errors cancel")
		}
	})

	t.Run("pops reordered independently of keys", func(t *testing.T) {
		definition, witness, err := EpochKAT(epochConfig)
		if err != nil {
			t.Fatal(err)
		}
		witness.NextRosterPoPs.ValidatorPoPs[0], witness.NextRosterPoPs.ValidatorPoPs[1] =
			witness.NextRosterPoPs.ValidatorPoPs[1], witness.NextRosterPoPs.ValidatorPoPs[0]
		witness.NextRosterPoPs.ValidatorPoPPoints[0], witness.NextRosterPoPs.ValidatorPoPPoints[1] =
			witness.NextRosterPoPs.ValidatorPoPPoints[1], witness.NextRosterPoPs.ValidatorPoPPoints[0]
		witness.Finality.ValidatorPoPs[0], witness.Finality.ValidatorPoPs[1] =
			witness.Finality.ValidatorPoPs[1], witness.Finality.ValidatorPoPs[0]
		witness.Finality.ValidatorPoPHashes[0], witness.Finality.ValidatorPoPHashes[1] =
			witness.Finality.ValidatorPoPHashes[1], witness.Finality.ValidatorPoPHashes[0]
		refreshKATEpochDerived(epochConfig, witness)
		if err := test.IsSolved(definition, witness, field); err == nil {
			t.Fatal("BLS PoPs were accepted after reordering them independently of their public keys")
		}
	})

	messageConfig, err := profile.ByID("sccp-final-v1-ethereum-mainnet-message")
	if err != nil {
		t.Fatal(err)
	}
	t.Run("ordered roster bound to approved anchor", func(t *testing.T) {
		definition, witness, err := MessageKAT(messageConfig)
		if err != nil {
			t.Fatal(err)
		}
		witness.Finality.ValidatorPublicKeys[0], witness.Finality.ValidatorPublicKeys[1] =
			witness.Finality.ValidatorPublicKeys[1], witness.Finality.ValidatorPublicKeys[0]
		witness.Finality.ValidatorKeyHashes[0], witness.Finality.ValidatorKeyHashes[1] =
			witness.Finality.ValidatorKeyHashes[1], witness.Finality.ValidatorKeyHashes[0]
		witness.Finality.ValidatorPoPs[0], witness.Finality.ValidatorPoPs[1] =
			witness.Finality.ValidatorPoPs[1], witness.Finality.ValidatorPoPs[0]
		witness.Finality.ValidatorPoPHashes[0], witness.Finality.ValidatorPoPHashes[1] =
			witness.Finality.ValidatorPoPHashes[1], witness.Finality.ValidatorPoPHashes[0]
		refreshKATFinalityCommitments(&witness.Finality, messageConfig.ID+":message")
		refreshKATMessageStatement(messageConfig, witness)
		// The signer set and aggregate remain valid under this 0/1 permutation,
		// and every downstream finality digest was refreshed. The unchanged
		// governed anchor must reject it because roster order is identity.
		if err := test.IsSolved(definition, witness, field); err == nil {
			t.Fatal("a reordered roster was accepted under the original approved anchor")
		}
	})

	t.Run("pop hash substitution bound to approved anchor", func(t *testing.T) {
		definition, witness, err := MessageKAT(messageConfig)
		if err != nil {
			t.Fatal(err)
		}
		changedPoPHash := u8Array32(witness.Finality.ValidatorPoPHashes[0])
		changedPoPHash[0] ^= 1
		set32(&witness.Finality.ValidatorPoPHashes[0], changedPoPHash)
		refreshKATFinalityCommitments(&witness.Finality, messageConfig.ID+":message")
		refreshKATMessageStatement(messageConfig, witness)
		if err := test.IsSolved(definition, witness, field); err == nil {
			t.Fatal("a substituted PoP digest was accepted under the original approved anchor")
		}
	})
}

func setFinalityPoP(
	finality *FinalityWitness,
	batch *PoPBatchWitness,
	index int,
	proof bls12381.G2Affine,
) {
	bytes := proof.Bytes()
	copyU8(batch.ValidatorPoPs[index][:], bytes[:])
	batch.ValidatorPoPPoints[index] = sw_bls12381.NewG2Affine(proof)
	copyU8(finality.ValidatorPoPs[index][:], bytes[:])
	digest := sha256.Sum256(bytes[:])
	set32(&finality.ValidatorPoPHashes[index], digest)
}

func refreshKATMessageStatement(cfg profile.Config, witness *MessageCircuit) {
	statement := nativeSemanticStatement(cfg, witness, nil)
	set32(&witness.RawSignals[7], statement)
	labels := signalLabelsBN254
	if cfg.SignalHash == profile.SHA256Signal {
		labels = signalLabelsBLS12381
	}
	raw := u8Array32(witness.RawSignals[7])
	digest := nativeSignalHash(cfg.SignalHash, labels[7], raw[:])
	witness.PublicSignals[7] = new(big.Int).SetBytes(digest[:])
}
