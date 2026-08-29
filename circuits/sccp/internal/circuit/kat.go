package circuit

import (
	"crypto/sha256"
	"encoding/binary"
	"encoding/hex"
	"fmt"
	"math/big"

	"github.com/consensys/gnark-crypto/ecc"
	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/algebra/emulated/sw_bls12381"
	"github.com/consensys/gnark/std/math/uints"
	"golang.org/x/crypto/blake2b"
	"golang.org/x/crypto/sha3"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

// KnownAnswerVector is a public-only deterministic circuit KAT. The private
// assignment is reconstructed by MessageKAT or EpochKAT and checked by tests.
type KnownAnswerVector struct {
	Schema           string        `json:"schema"`
	Version          int           `json:"version"`
	Profile          string        `json:"profile"`
	Role             profile.Role  `json:"role"`
	Curve            profile.Curve `json:"curve"`
	IndependentKeyID string        `json:"independent_key_id"`
	RawSignals       []string      `json:"raw_signals"`
	PublicSignals    []string      `json:"public_signals"`
}

// PublicKAT returns the unique deterministic public KAT for one closed profile.
func PublicKAT(cfg profile.Config) (KnownAnswerVector, error) {
	var raw [11][32]uints.U8
	var public [11]frontend.Variable
	switch cfg.Role {
	case profile.Message:
		_, witness, err := MessageKAT(cfg)
		if err != nil {
			return KnownAnswerVector{}, err
		}
		raw = witness.RawSignals
		public = witness.PublicSignals
	case profile.EpochAnchorUpdate:
		_, witness, err := EpochKAT(cfg)
		if err != nil {
			return KnownAnswerVector{}, err
		}
		raw = witness.RawSignals
		public = witness.PublicSignals
	default:
		return KnownAnswerVector{}, fmt.Errorf("unsupported SCCP KAT role %q", cfg.Role)
	}

	modulus := ecc.BN254.ScalarField()
	if cfg.Curve == profile.BLS12381 {
		modulus = ecc.BLS12_381.ScalarField()
	}
	vector := KnownAnswerVector{
		Schema:           "sccp-circuit-kat-final-v1",
		Version:          1,
		Profile:          cfg.ID,
		Role:             cfg.Role,
		Curve:            cfg.Curve,
		IndependentKeyID: hex.EncodeToString(cfg.IndependentKeyID[:]),
		RawSignals:       make([]string, len(raw)),
		PublicSignals:    make([]string, len(public)),
	}
	for index := range raw {
		word := u8Array32(raw[index])
		vector.RawSignals[index] = hex.EncodeToString(word[:])
		value, ok := public[index].(*big.Int)
		if !ok {
			return KnownAnswerVector{}, fmt.Errorf("KAT public signal %d has non-integer type %T", index, public[index])
		}
		reduced := new(big.Int).Mod(new(big.Int).Set(value), modulus)
		vector.PublicSignals[index] = fmt.Sprintf("%064x", reduced)
	}
	return vector, nil
}

// MessageKAT constructs a deterministic positive known-answer assignment for
// one message profile. It creates no proving or verification key.
func MessageKAT(cfg profile.Config) (*MessageCircuit, *MessageCircuit, error) {
	definition, err := NewMessage(cfg)
	if err != nil {
		return nil, nil, err
	}
	witness := &MessageCircuit{cfg: cfg}
	initializeMessageWitness(witness)

	sender, err := populateI105Witness(&witness.SenderI105, katI105CanonicalAccount[:])
	if err != nil {
		return nil, nil, err
	}
	payload := canonicalKATPayload(cfg, sender)
	copyU8(witness.Payload[:], payload)
	witness.PayloadLength = len(payload)
	witness.MerkleDepth = 0

	payloadHash := nativeBlake2b([]byte("sccp:payload:v1"), payload)
	set32(&witness.RawSignals[1], payloadHash)
	lane := canonicalLaneBytes(cfg)
	messagePreimage := []byte("sccp:lane-message-id:v1")
	messagePreimage = append(messagePreimage, 1)
	messagePreimage = binary.LittleEndian.AppendUint32(messagePreimage, uint32(len(lane)))
	messagePreimage = append(messagePreimage, lane...)
	messagePreimage = binary.LittleEndian.AppendUint32(messagePreimage, uint32(len(payload)))
	messagePreimage = append(messagePreimage, payload...)
	messageID := nativeKeccak(messagePreimage)
	set32(&witness.RawSignals[0], messageID)

	destinationBinding := sha256.Sum256([]byte(cfg.ID + ":destination"))
	routeConfiguration := sha256.Sum256([]byte(cfg.ID + ":route"))
	set32(&witness.RawSignals[8], destinationBinding)
	set32(&witness.RawSignals[9], routeConfiguration)
	semanticProfile := sha256.Sum256([]byte(cfg.ID + ":semantic-profile"))
	set32(&witness.Statement.SemanticProofProfileHash, semanticProfile)
	if cfg.Curve == profile.BLS12381 {
		verifierCircuit := sha256.Sum256([]byte(cfg.ID + ":verifier-circuit"))
		proofProfile := sha256.Sum256([]byte(cfg.ID + ":proof-profile"))
		set32(&witness.Statement.VerifierCircuitHash, verifierCircuit)
		set32(&witness.Statement.ProofProfileCommitment, proofProfile)
	}
	commitment := []byte{1, 5, profile.SoraNetworkTag, cfg.TargetNetworkTag}
	commitment = append(commitment, destinationBinding[:]...)
	commitment = append(commitment, routeConfiguration[:]...)
	commitment = append(commitment, messageID[:]...)
	commitment = append(commitment, payloadHash[:]...)
	root := nativeBlake2b([]byte("sccp:hub:leaf:v1"), commitment)
	set32(&witness.RawSignals[3], root)

	populateKATFinality(&witness.Finality, nil, cfg.ID+":message", 100, 7, 150, nil)
	populateKATBlockHeaderProjection(&witness.BlockHeader, witness.Finality, cfg.ID+":message-header")
	commitmentRoot := u8Array32(witness.RawSignals[3])
	set32(
		&witness.Finality.BlockHeaderHash,
		nativeBlockHeaderConsensusHash(witness.BlockHeader, commitmentRoot),
	)
	refreshKATFinalityCommitments(&witness.Finality, cfg.ID+":message")
	blockHash := u8Array32(witness.Finality.BlockHeaderHash)
	set32(&witness.RawSignals[5], blockHash)

	witness.Anchor.CheckpointHeight = 90
	witness.Anchor.Epoch = witness.Finality.Epoch
	witness.Anchor.EpochEndHeight = witness.Finality.EpochEndHeight
	set32(&witness.Anchor.RosterCommitment, nativeFinalityRosterCommitment(witness.Finality))
	anchorBlock := sha256.Sum256([]byte(cfg.ID + ":anchor-block"))
	anchorContext := sha256.Sum256([]byte(cfg.ID + ":anchor-context"))
	anchorArtifact := sha256.Sum256([]byte(cfg.ID + ":anchor-artifact"))
	set32(&witness.Anchor.CheckpointBlockHash, anchorBlock)
	set32(&witness.Anchor.CheckpointContextID, anchorContext)
	set32(&witness.Anchor.CheckpointFinalityArtifactHash, anchorArtifact)
	anchorHash := nativeAnchorHash(
		90,
		7,
		150,
		nativeFinalityRosterCommitment(witness.Finality),
		anchorBlock,
		anchorContext,
		anchorArtifact,
	)
	set32(&witness.RawSignals[10], anchorHash)

	var targetWord [32]byte
	binary.BigEndian.PutUint32(targetWord[28:], cfg.TargetDomain)
	set32(&witness.RawSignals[2], targetWord)
	heightWord := encodeUint64BEWord(100)
	set32(&witness.RawSignals[4], heightWord)
	set32(&witness.RawSignals[6], [32]byte{})

	statementHash := nativeSemanticStatement(cfg, witness, payload)
	set32(&witness.RawSignals[7], statementHash)

	labels := signalLabelsBN254
	if cfg.SignalHash == profile.SHA256Signal {
		labels = signalLabelsBLS12381
	}
	for i := range witness.RawSignals {
		raw := u8Array32(witness.RawSignals[i])
		digest := nativeSignalHash(cfg.SignalHash, labels[i], raw[:])
		witness.PublicSignals[i] = new(big.Int).SetBytes(digest[:])
	}
	return definition, witness, nil
}

// EpochKAT constructs a deterministic positive known-answer assignment for
// one epoch-anchor-update profile without generating trusted-setup material.
func EpochKAT(cfg profile.Config) (*EpochAnchorCircuit, *EpochAnchorCircuit, error) {
	definition, err := NewEpochAnchor(cfg)
	if err != nil {
		return nil, nil, err
	}
	witness := &EpochAnchorCircuit{cfg: cfg}
	initializeEpochWitness(witness)

	witness.Snapshot.CurrentEpoch = 8
	witness.Snapshot.NextEpoch = 9
	witness.Snapshot.NextEpochEndHeight = 200
	witness.Snapshot.Mode = sumeragiModeNPoS
	witness.Snapshot.ValidatorCount = 4
	populateKATFinality(
		&witness.Finality,
		&witness.NextRosterPoPs,
		cfg.ID+":successor",
		100,
		9,
		200,
		nil,
	)
	refreshKATEpochDerived(cfg, witness)
	return definition, witness, nil
}

type katEpochTransition struct {
	boundaryHeight uint64
	boundaryEpoch  uint64
	boundaryEnd    uint64
	boundaryScope  string
	successorScope string
	currentAnchor  *AnchorWitness
}

// refreshKATEpochDerived rebuilds every commitment downstream of the
// successor roster. Adversarial tests use it after modifying a PoP or roster
// field so the negative reaches the intended cryptographic or anchor binding
// instead of failing on an unrelated stale helper value.
func refreshKATEpochDerived(cfg profile.Config, witness *EpochAnchorCircuit) {
	refreshKATEpochTransition(cfg, witness, katEpochTransition{
		boundaryHeight: 99,
		boundaryEpoch:  8,
		boundaryEnd:    99,
		boundaryScope:  cfg.ID + ":boundary",
		successorScope: cfg.ID + ":successor",
	})
}

func refreshKATEpochTransition(
	cfg profile.Config,
	witness *EpochAnchorCircuit,
	transition katEpochTransition,
) {
	for index := 0; index < MaxValidators; index++ {
		witness.Snapshot.ValidatorKeyHashes[index] = witness.Finality.ValidatorKeyHashes[index]
		witness.Snapshot.ValidatorPoPHashes[index] = witness.Finality.ValidatorPoPHashes[index]
	}
	witness.Snapshot.LeaderSeed = witness.Finality.LeaderSeed

	roster := []byte("sccp:epoch-anchor:roster:v1")
	validatorCount := concreteKATUint64(witness.Snapshot.ValidatorCount)
	roster = binary.LittleEndian.AppendUint32(roster, uint32(validatorCount))
	for i := 0; i < int(validatorCount); i++ {
		key := u8Array32(witness.Snapshot.ValidatorKeyHashes[i])
		pop := u8Array32(witness.Snapshot.ValidatorPoPHashes[i])
		roster = append(roster, key[:]...)
		roster = append(roster, pop[:]...)
	}
	rosterHash := sha256.Sum256(roster)
	set32(&witness.RawSignals[8], rosterHash)
	snapshot := []byte("sccp:epoch-anchor:snapshot:v1")
	snapshot = append(snapshot, 1, byte(concreteKATUint64(witness.Snapshot.Mode)))
	snapshot = binary.LittleEndian.AppendUint64(
		snapshot,
		concreteKATUint64(witness.Snapshot.NextEpoch),
	)
	snapshot = binary.LittleEndian.AppendUint64(
		snapshot,
		concreteKATUint64(witness.Snapshot.NextEpochEndHeight),
	)
	snapshot = append(snapshot, rosterHash[:]...)
	leaderSeed := u8Array32(witness.Snapshot.LeaderSeed)
	snapshot = append(snapshot, leaderSeed[:]...)
	snapshotHash := sha256.Sum256(snapshot)
	set32(&witness.RawSignals[2], snapshotHash)

	populateKATFinality(
		&witness.BoundaryFinality,
		nil,
		transition.boundaryScope,
		transition.boundaryHeight,
		transition.boundaryEpoch,
		transition.boundaryEnd,
		&snapshotHash,
	)
	witness.BoundaryFinality.NextEpochSnapshot.Epoch = witness.Snapshot.NextEpoch
	witness.BoundaryFinality.NextEpochSnapshot.EpochEndHeight = witness.Snapshot.NextEpochEndHeight
	witness.BoundaryFinality.NextEpochSnapshot.Mode = witness.Snapshot.Mode
	witness.BoundaryFinality.NextEpochSnapshot.ValidatorCount = witness.Snapshot.ValidatorCount
	for index := 0; index < MaxValidators; index++ {
		witness.BoundaryFinality.NextEpochSnapshot.ValidatorPublicKeys[index] =
			witness.Finality.ValidatorPublicKeys[index]
		witness.BoundaryFinality.NextEpochSnapshot.ValidatorPoPs[index] =
			witness.NextRosterPoPs.ValidatorPoPs[index]
	}
	witness.BoundaryFinality.NextEpochSnapshot.LeaderSeed = witness.Snapshot.LeaderSeed
	refreshKATFinalityCommitments(&witness.BoundaryFinality, transition.boundaryScope)
	witness.Finality.ParentContextID = witness.BoundaryFinality.HeightContextID
	witness.Finality.ParentHeight = witness.BoundaryFinality.Height
	witness.Finality.ParentRoundView = witness.BoundaryFinality.RoundView
	witness.Finality.ParentProposalView = witness.BoundaryFinality.ProposalView
	witness.Finality.ParentSignerCount = 3
	for index := 0; index < MaxValidators; index++ {
		witness.Finality.ParentSignerIndices[index] = witness.BoundaryFinality.SignerIndices[index]
	}
	witness.Finality.ParentAggregateSignature = witness.BoundaryFinality.AggregateSignature
	witness.Finality.ParentSubjectParentBlockHash = witness.BoundaryFinality.SubjectParentBlockHash
	witness.Finality.ParentBlockHash = witness.BoundaryFinality.BlockHeaderHash
	witness.Finality.SubjectParentBlockHash = witness.BoundaryFinality.BlockHeaderHash
	witness.Finality.ParentPayloadHash = witness.BoundaryFinality.SubjectPayloadHash
	copyExecutionCommitment(&witness.Finality.ParentExecution, witness.BoundaryFinality.Execution)
	refreshKATFinalityCommitments(&witness.Finality, transition.successorScope)

	if transition.currentAnchor == nil {
		currentBlock := u8Array32(witness.BoundaryFinality.BlockHeaderHash)
		currentContext := u8Array32(witness.BoundaryFinality.HeightContextID)
		currentArtifact := u8Array32(witness.BoundaryFinality.FinalityArtifactHash)
		witness.CurrentAnchor.CheckpointHeight = int(transition.boundaryHeight)
		witness.CurrentAnchor.Epoch = int(transition.boundaryEpoch)
		witness.CurrentAnchor.EpochEndHeight = int(transition.boundaryEnd)
		currentRoster := nativeFinalityRosterCommitment(witness.BoundaryFinality)
		set32(&witness.CurrentAnchor.RosterCommitment, currentRoster)
		set32(&witness.CurrentAnchor.CheckpointBlockHash, currentBlock)
		set32(&witness.CurrentAnchor.CheckpointContextID, currentContext)
		set32(&witness.CurrentAnchor.CheckpointFinalityArtifactHash, currentArtifact)
	} else {
		witness.CurrentAnchor = *transition.currentAnchor
	}
	currentRoster := u8Array32(witness.CurrentAnchor.RosterCommitment)
	currentBlock := u8Array32(witness.CurrentAnchor.CheckpointBlockHash)
	currentContext := u8Array32(witness.CurrentAnchor.CheckpointContextID)
	currentArtifact := u8Array32(witness.CurrentAnchor.CheckpointFinalityArtifactHash)
	set32(
		&witness.RawSignals[0],
		nativeAnchorHash(
			concreteKATUint64(witness.CurrentAnchor.CheckpointHeight),
			concreteKATUint64(witness.CurrentAnchor.Epoch),
			concreteKATUint64(witness.CurrentAnchor.EpochEndHeight),
			currentRoster,
			currentBlock,
			currentContext,
			currentArtifact,
		),
	)

	nextBlock := u8Array32(witness.Finality.BlockHeaderHash)
	nextContext := u8Array32(witness.Finality.HeightContextID)
	nextArtifact := u8Array32(witness.Finality.FinalityArtifactHash)
	witness.NextAnchor.CheckpointHeight = witness.Finality.Height
	witness.NextAnchor.Epoch = witness.Finality.Epoch
	witness.NextAnchor.EpochEndHeight = witness.Finality.EpochEndHeight
	nextRoster := nativeFinalityRosterCommitment(witness.Finality)
	set32(&witness.NextAnchor.RosterCommitment, nextRoster)
	set32(&witness.NextAnchor.CheckpointBlockHash, nextBlock)
	set32(&witness.NextAnchor.CheckpointContextID, nextContext)
	set32(&witness.NextAnchor.CheckpointFinalityArtifactHash, nextArtifact)
	set32(
		&witness.RawSignals[1],
		nativeAnchorHash(
			concreteKATUint64(witness.NextAnchor.CheckpointHeight),
			concreteKATUint64(witness.NextAnchor.Epoch),
			concreteKATUint64(witness.NextAnchor.EpochEndHeight),
			nextRoster,
			nextBlock,
			nextContext,
			nextArtifact,
		),
	)

	set32(&witness.RawSignals[3], nextContext)
	set32(
		&witness.RawSignals[4],
		encodeUint64BEWord(concreteKATUint64(witness.NextAnchor.CheckpointHeight)),
	)
	set32(&witness.RawSignals[5], nextBlock)
	set32(&witness.RawSignals[6], nativeKeccak(tairaChainID))
	set32(&witness.RawSignals[7], nextArtifact)
	policyHash := sha256.Sum256([]byte(cfg.ID + ":deployment-policy"))
	set32(&witness.RawSignals[9], policyHash)
	set32(&witness.RawSignals[10], cfg.IndependentKeyID)

	for i, role := range epochSignalRoles {
		label := fmt.Sprintf("sccp:groth16:%s:epoch-anchor:signal:%s:v1", cfg.Curve, role)
		raw := u8Array32(witness.RawSignals[i])
		digest := nativeSignalHash(cfg.SignalHash, label, raw[:])
		witness.PublicSignals[i] = new(big.Int).SetBytes(digest[:])
	}
}

func concreteKATUint64(value frontend.Variable) uint64 {
	switch typed := value.(type) {
	case int:
		return uint64(typed)
	case uint64:
		return typed
	case uint32:
		return uint64(typed)
	default:
		panic(fmt.Sprintf("unexpected concrete KAT integer %T", value))
	}
}

func initializeMessageWitness(w *MessageCircuit) {
	zeroU8s(w.Payload[:])
	for i := range w.RawSignals {
		zeroU8s(w.RawSignals[i][:])
		w.PublicSignals[i] = 0
	}
	for i := range w.Siblings {
		zeroU8s(w.Siblings[i][:])
		w.SiblingIsLeft[i] = 0
	}
	initializeAnchor(&w.Anchor)
	initializeFinality(&w.Finality)
	zeroU8s(w.Statement.SemanticProofProfileHash[:])
	zeroU8s(w.Statement.VerifierCircuitHash[:])
	zeroU8s(w.Statement.ProofProfileCommitment[:])
	initializeI105Witness(&w.SenderI105)
	initializeBlockHeaderProjection(&w.BlockHeader)
	w.PayloadLength = 0
	w.MerkleDepth = 0
}

func initializeEpochWitness(w *EpochAnchorCircuit) {
	for i := range w.RawSignals {
		zeroU8s(w.RawSignals[i][:])
		w.PublicSignals[i] = 0
	}
	initializeAnchor(&w.CurrentAnchor)
	initializeAnchor(&w.NextAnchor)
	initializeFinality(&w.BoundaryFinality)
	initializeFinality(&w.Finality)
	initializePoPBatch(&w.NextRosterPoPs)
	w.Snapshot.CurrentEpoch = 0
	w.Snapshot.NextEpoch = 0
	w.Snapshot.NextEpochEndHeight = 0
	w.Snapshot.Mode = 0
	w.Snapshot.ValidatorCount = 0
	for i := range w.Snapshot.ValidatorKeyHashes {
		zeroU8s(w.Snapshot.ValidatorKeyHashes[i][:])
		zeroU8s(w.Snapshot.ValidatorPoPHashes[i][:])
	}
	zeroU8s(w.Snapshot.LeaderSeed[:])
}

func initializeAnchor(anchor *AnchorWitness) {
	anchor.CheckpointHeight = 0
	anchor.Epoch = 0
	anchor.EpochEndHeight = 0
	zeroU8s(anchor.RosterCommitment[:])
	zeroU8s(anchor.CheckpointBlockHash[:])
	zeroU8s(anchor.CheckpointContextID[:])
	zeroU8s(anchor.CheckpointFinalityArtifactHash[:])
}

func initializeFinality(finality *FinalityWitness) {
	finality.Height = 0
	finality.ContextHeight = 0
	finality.CertificateHeight = 0
	finality.Epoch = 0
	finality.EpochEndHeight = 0
	finality.HasNextEpochSnapshot = 0
	finality.Mode = 0
	finality.RoundView = 0
	finality.ProposalView = 0
	finality.BlockHeaderView = 0
	finality.ParentHeight = 0
	finality.ParentRoundView = 0
	finality.ParentProposalView = 0
	finality.ParentSignerCount = 0
	finality.ValidatorCount = 0
	for i := range finality.SignerBitmap {
		finality.SignerBitmap[i] = 0
		finality.SignerIndices[i] = 0
		finality.ParentSignerIndices[i] = 0
		zeroU8s(finality.ValidatorPublicKeys[i][:])
		zeroU8s(finality.ValidatorPoPs[i][:])
		zeroU8s(finality.ValidatorKeyHashes[i][:])
		zeroU8s(finality.ValidatorPoPHashes[i][:])
		zeroU8s(finality.NextEpochSnapshot.ValidatorPublicKeys[i][:])
		zeroU8s(finality.NextEpochSnapshot.ValidatorPoPs[i][:])
	}
	finality.NextEpochSnapshot.Epoch = 0
	finality.NextEpochSnapshot.EpochEndHeight = 0
	finality.NextEpochSnapshot.Mode = 0
	finality.NextEpochSnapshot.ValidatorCount = 0
	zeroU8s(finality.NextEpochSnapshot.LeaderSeed[:])
	zeroU8s(finality.SubjectParentBlockHash[:])
	zeroU8s(finality.SubjectPayloadHash[:])
	initializeExecutionCommitment(&finality.Execution)
	zeroU8s(finality.ParentContextID[:])
	zeroU8s(finality.ParentAggregateSignature[:])
	zeroU8s(finality.ParentSubjectParentBlockHash[:])
	zeroU8s(finality.ParentBlockHash[:])
	zeroU8s(finality.ParentPayloadHash[:])
	initializeExecutionCommitment(&finality.ParentExecution)
	zeroU8s(finality.NexusAMXContextHash[:])
	zeroU8s(finality.ExecutionPolicyHash[:])
	finality.DALayout.ChunkSizeBytes = 0
	finality.DALayout.DataShards = 0
	finality.DALayout.ParityShards = 0
	finality.DALayout.MaxPayloadSizeBytes = 0
	finality.DALayout.MaxChunkCount = 0
	finality.DALayout.RequiredStripes = 0
	finality.DALayout.LastStripePayloadSize = 0
	zeroU8s(finality.LeaderSeed[:])
	zeroU8s(finality.AggregateSignatureHash[:])
	zeroU8s(finality.AggregateSignature[:])
	finality.AggregateSignaturePoint = sw_bls12381.NewG2Affine(fixedDummyBLSMaterial.proof)
	finality.VoteSignaturePayloadLength = 0
	zeroU8s(finality.VoteSignaturePayload[:])
	zeroU8s(finality.VotePreimageHash[:])
	zeroU8s(finality.HeightContextID[:])
	zeroU8s(finality.BlockHeaderHash[:])
	zeroU8s(finality.FinalityArtifactHash[:])
	finality.FinalityArtifactLength = 0
	zeroU8s(finality.FinalityArtifactBytes[:])
}

func initializePoPBatch(batch *PoPBatchWitness) {
	for index := range batch.ValidatorPoPs {
		zeroU8s(batch.ValidatorPoPs[index][:])
		batch.ValidatorPoPPoints[index] = sw_bls12381.NewG2Affine(fixedDummyBLSMaterial.proof)
	}
}

func canonicalKATPayload(cfg profile.Config, sender []byte) []byte {
	recipient := make([]byte, cfg.RecipientLength)
	for i := range recipient {
		recipient[i] = byte(i + 1)
	}
	if cfg.RecipientCodec == 5 {
		recipient[0] = 0x41
	}
	if cfg.RecipientCodec == 7 {
		for i := 0; i < 4; i++ {
			recipient[i] = 0
		}
	}
	out := []byte{2, 1}
	out = binary.LittleEndian.AppendUint32(out, 0)
	out = binary.LittleEndian.AppendUint32(out, cfg.TargetDomain)
	out = binary.LittleEndian.AppendUint64(out, 7)
	out = binary.LittleEndian.AppendUint32(out, 1)
	out = binary.LittleEndian.AppendUint32(out, 0)
	out = append(out, 1)
	out = binary.LittleEndian.AppendUint32(out, 3)
	out = append(out, "xor"...)
	amount := make([]byte, 16)
	binary.LittleEndian.PutUint64(amount, 11)
	out = append(out, amount...)
	out = append(out, 1)
	out = binary.LittleEndian.AppendUint32(out, uint32(len(sender)))
	out = append(out, sender...)
	out = append(out, cfg.RecipientCodec)
	out = binary.LittleEndian.AppendUint32(out, uint32(len(recipient)))
	out = append(out, recipient...)
	out = append(out, 1)
	out = binary.LittleEndian.AppendUint32(out, uint32(len(cfg.RouteID)))
	out = append(out, cfg.RouteID...)
	return out
}

func nativeAnchorHash(
	height uint64,
	epoch uint64,
	epochEndHeight uint64,
	rosterCommitment [32]byte,
	blockHash,
	contextID,
	artifactHash [32]byte,
) [32]byte {
	chainIDHash := nativeKeccak(tairaChainID)
	preimage := []byte("sccp:sora-finality-anchor:v1")
	preimage = append(preimage, 1, 1, 4, 0)
	preimage = append(preimage, chainIDHash[:]...)
	preimage = binary.LittleEndian.AppendUint64(preimage, epoch)
	preimage = binary.LittleEndian.AppendUint64(preimage, epochEndHeight)
	preimage = append(preimage, rosterCommitment[:]...)
	preimage = binary.LittleEndian.AppendUint64(preimage, height)
	preimage = append(preimage, blockHash[:]...)
	preimage = append(preimage, contextID[:]...)
	preimage = append(preimage, artifactHash[:]...)
	return nativeKeccak(preimage)
}

func nativeSignalHash(kind profile.SignalHash, label string, raw []byte) [32]byte {
	if kind == profile.KeccakSignal {
		labelHash := nativeKeccak([]byte(label))
		return nativeKeccak(append(labelHash[:], raw...))
	}
	labelHash := sha256.Sum256([]byte(label))
	return sha256.Sum256(append(labelHash[:], raw...))
}

func nativeSemanticStatement(cfg profile.Config, witness *MessageCircuit, _ []byte) [32]byte {
	source, target := canonicalNetworkBytes(cfg)
	destination := u8Array32(witness.RawSignals[8])
	route := u8Array32(witness.RawSignals[9])
	semanticProfile := u8Array32(witness.Statement.SemanticProofProfileHash)
	anchor := u8Array32(witness.RawSignals[10])
	verifierCircuit := u8Array32(witness.Statement.VerifierCircuitHash)
	proofProfile := u8Array32(witness.Statement.ProofProfileCommitment)
	messageID := u8Array32(witness.RawSignals[0])
	payloadHash := u8Array32(witness.RawSignals[1])
	root := u8Array32(witness.RawSignals[3])
	blockHash := u8Array32(witness.RawSignals[5])
	preimage := []byte(semanticStatementDomainFinalV1)
	preimage = append(preimage, 1, cfg.BackendTag)
	preimage = appendNativeVec(preimage, source)
	preimage = appendNativeVec(preimage, target)
	preimage = append(preimage, destination[:]...)
	preimage = append(preimage, route[:]...)
	preimage = append(preimage, semanticProfile[:]...)
	preimage = append(preimage, anchor[:]...)
	preimage = append(preimage, verifierCircuit[:]...)
	preimage = append(preimage, proofProfile[:]...)
	preimage = append(preimage, 1)
	preimage = append(preimage, messageID[:]...)
	preimage = append(preimage, payloadHash[:]...)
	preimage = binary.LittleEndian.AppendUint32(preimage, cfg.TargetDomain)
	preimage = append(preimage, root[:]...)
	preimage = binary.LittleEndian.AppendUint64(preimage, uint64(witness.Finality.Height.(int)))
	preimage = append(preimage, blockHash[:]...)
	preimage = append(preimage, payloadHash[:]...)
	bundle := nativeSemanticBundle(cfg, witness)
	preimage = append(preimage, bundle[:]...)
	if cfg.SignalHash == profile.KeccakSignal {
		return nativeKeccak(preimage)
	}
	return sha256.Sum256(preimage)
}

func nativeSemanticBundle(cfg profile.Config, witness *MessageCircuit) [32]byte {
	destination := u8Array32(witness.RawSignals[8])
	route := u8Array32(witness.RawSignals[9])
	messageID := u8Array32(witness.RawSignals[0])
	payloadHash := u8Array32(witness.RawSignals[1])
	root := u8Array32(witness.RawSignals[3])
	commitment := []byte{1, 5, profile.SoraNetworkTag, cfg.TargetNetworkTag}
	commitment = append(commitment, destination[:]...)
	commitment = append(commitment, route[:]...)
	commitment = append(commitment, messageID[:]...)
	commitment = append(commitment, payloadHash[:]...)
	proof := binary.LittleEndian.AppendUint32(nil, uint32(witness.MerkleDepth.(int)))
	proofDigest := sha256.Sum256(proof)
	finality := u8Array32(witness.Finality.FinalityArtifactHash)
	preimage := []byte("sccp:semantic-bundle:final-v1")
	preimage = append(preimage, 1)
	preimage = append(preimage, root[:]...)
	preimage = append(preimage, commitment...)
	preimage = append(preimage, proofDigest[:]...)
	preimage = append(preimage, payloadHash[:]...)
	preimage = append(preimage, finality[:]...)
	return sha256.Sum256(preimage)
}

func populateKATFinality(
	finality *FinalityWitness,
	newRosterPoPs *PoPBatchWitness,
	scope string,
	height uint64,
	epoch uint64,
	epochEnd uint64,
	nextSnapshot *[32]byte,
) {
	finality.Height = int(height)
	finality.ContextHeight = int(height)
	finality.CertificateHeight = int(height)
	finality.Epoch = int(epoch)
	finality.EpochEndHeight = int(epochEnd)
	finality.Mode = sumeragiModeNPoS
	finality.RoundView = 3
	finality.ProposalView = 3
	finality.BlockHeaderView = 2
	finality.ParentHeight = int(height - 1)
	finality.ParentRoundView = 2
	finality.ParentProposalView = 2
	finality.ParentSignerCount = 3
	if nextSnapshot != nil {
		finality.HasNextEpochSnapshot = 1
	}
	finality.ValidatorCount = 4
	for index := 0; index < 3; index++ {
		finality.SignerBitmap[index] = 1
		finality.SignerIndices[index] = index
		finality.ParentSignerIndices[index] = index
	}
	materials := deterministicBLSRoster(scope, 4)
	for index := 0; index < 4; index++ {
		material := materials[index]
		copyU8(finality.ValidatorPublicKeys[index][:], material.publicKeyBytes[:])
		copyU8(finality.ValidatorPoPs[index][:], material.proofBytes[:])
		if newRosterPoPs != nil {
			copyU8(newRosterPoPs.ValidatorPoPs[index][:], material.proofBytes[:])
			newRosterPoPs.ValidatorPoPPoints[index] = sw_bls12381.NewG2Affine(material.proof)
		}
		keyHash := sha256.Sum256(material.publicKeyBytes[:])
		popHash := sha256.Sum256(material.proofBytes[:])
		set32(&finality.ValidatorKeyHashes[index], keyHash)
		set32(&finality.ValidatorPoPHashes[index], popHash)
	}
	setMarked := func(destination *[32]uints.U8, label string) {
		set32(destination, nativeIrohaHash([]byte(scope+":"+label)))
	}
	setMarked(&finality.ParentContextID, "parent-context")
	parentAggregateMaterial := deterministicBLSKeyMaterial(scope + ":parent-aggregate")
	copyU8(finality.ParentAggregateSignature[:], parentAggregateMaterial.proofBytes[:])
	setMarked(&finality.ParentSubjectParentBlockHash, "parent-subject-parent")
	setMarked(&finality.ParentBlockHash, "parent-block")
	finality.SubjectParentBlockHash = finality.ParentBlockHash
	setMarked(&finality.ParentPayloadHash, "parent-payload")
	setMarked(&finality.SubjectPayloadHash, "subject-payload")
	populateKATExecutionCommitment(&finality.ParentExecution, scope+":parent-execution")
	populateKATExecutionCommitment(&finality.Execution, scope+":execution")
	setMarked(&finality.NexusAMXContextHash, "nexus-amx")
	setMarked(&finality.ExecutionPolicyHash, "execution-policy")
	finality.DALayout.ChunkSizeBytes = maxDAChunkSizeBytes
	finality.DALayout.DataShards = 4
	finality.DALayout.ParityShards = 2
	finality.DALayout.MaxPayloadSizeBytes = maxDAPayloadSizeBytes
	finality.DALayout.MaxChunkCount = maxDAChunkCount
	finality.DALayout.RequiredStripes = 16
	finality.DALayout.LastStripePayloadSize = maxDAChunkSizeBytes * 4
	setMarked(&finality.BlockHeaderHash, "block")
	leader := sha256.Sum256([]byte(scope + ":leader-seed"))
	aggregate := sha256.Sum256([]byte(scope + ":aggregate-signature"))
	set32(&finality.LeaderSeed, leader)
	set32(&finality.AggregateSignatureHash, aggregate)
	if nextSnapshot == nil {
		refreshKATFinalityCommitments(finality, scope)
	}
}

func refreshKATFinalityCommitments(finality *FinalityWitness, scope string) {
	set32(&finality.HeightContextID, nativeHeightContextIdentity(*finality))
	votePayload := nativeCanonicalVoteSignaturePayload(*finality)
	finality.VoteSignaturePayloadLength = len(votePayload)
	zeroU8s(finality.VoteSignaturePayload[:])
	copyU8(finality.VoteSignaturePayload[:], votePayload)
	votePreimage := append(append([]byte(nil), sumeragiVoteSignatureDomain...), votePayload...)
	set32(&finality.VotePreimageHash, nativeIrohaHash(votePreimage))
	keys := deterministicBLSRoster(scope, int(finality.ValidatorCount.(int)))
	aggregate := nativeBLSAggregateSignature(keys, []int{0, 1, 2}, votePreimage)
	aggregateBytes := aggregate.Bytes()
	copyU8(finality.AggregateSignature[:], aggregateBytes[:])
	finality.AggregateSignaturePoint = sw_bls12381.NewG2Affine(aggregate)
	aggregateHash := sha256.Sum256(aggregateBytes[:])
	set32(&finality.AggregateSignatureHash, aggregateHash)
	artifactBytes := nativeFinalityArtifactBytes(*finality)
	if len(artifactBytes) > maxFinalityArtifactBytes {
		panic("canonical SCCP finality artifact exceeds circuit bound")
	}
	finality.FinalityArtifactLength = len(artifactBytes)
	zeroU8s(finality.FinalityArtifactBytes[:])
	copyU8(finality.FinalityArtifactBytes[:], artifactBytes)
	set32(&finality.FinalityArtifactHash, nativeIrohaHash(artifactBytes))
}

func populateKATExecutionCommitment(execution *ExecutionCommitmentWitness, scope string) {
	initializeExecutionCommitment(execution)
	setMarked := func(destination *[32]uints.U8, label string) {
		set32(destination, nativeIrohaHash([]byte(scope+":"+label)))
	}
	setMarked(&execution.ParentStateRoot, "parent-state-root")
	setMarked(&execution.PostStateRoot, "post-state-root")
	setMarked(&execution.OrdinaryWritesRoot, "ordinary-writes-root")
	execution.NativeAMXApplicationManifestVer = nativeAMXManifestVersion
	set32(
		&execution.NativeAMXApplicationManifestRoot,
		nativeIrohaHash([]byte("iroha:sumeragi:v2:native-amx-application-manifest:v1:empty")),
	)
	execution.ExecutedBlockWireLength = 1
	setMarked(&execution.ExecutedBlockWireHash, "executed-block-wire")
}

func nativeFinalityRosterCommitment(finality FinalityWitness) [32]byte {
	preimage := []byte("iroha:sumeragi:v2:roster-semantic:final-v1")
	preimage = append(preimage, sumeragiProtocolVersion)
	preimage = binary.LittleEndian.AppendUint32(preimage, uint32(finality.ValidatorCount.(int)))
	for index := 0; index < MaxValidators; index++ {
		key := u8Array32(finality.ValidatorKeyHashes[index])
		pop := u8Array32(finality.ValidatorPoPHashes[index])
		preimage = append(preimage, key[:]...)
		preimage = append(preimage, pop[:]...)
	}
	return nativeIrohaHash(preimage)
}

func nativeIrohaHash(input []byte) [32]byte {
	digest := nativeBlake2b(nil, input)
	digest[len(digest)-1] |= 1
	return digest
}

func appendNativeVec(destination, value []byte) []byte {
	destination = binary.LittleEndian.AppendUint32(destination, uint32(len(value)))
	return append(destination, value...)
}

func nativeKeccak(input []byte) [32]byte {
	h := sha3.NewLegacyKeccak256()
	_, _ = h.Write(input)
	var out [32]byte
	copy(out[:], h.Sum(nil))
	return out
}

func nativeBlake2b(prefix, input []byte) [32]byte {
	h, err := blake2b.New256(nil)
	if err != nil {
		panic(err)
	}
	_, _ = h.Write(prefix)
	_, _ = h.Write(input)
	var out [32]byte
	copy(out[:], h.Sum(nil))
	return out
}

func zeroU8s(destination []uints.U8) {
	copy(destination, uints.NewU8Array(make([]byte, len(destination))))
}

func copyU8(destination []uints.U8, source []byte) {
	copy(destination, uints.NewU8Array(source))
}

func set32(destination *[32]uints.U8, source [32]byte) {
	copy(destination[:], uints.NewU8Array(source[:]))
}

func u8Array32(source [32]uints.U8) [32]byte {
	var out [32]byte
	for i := range source {
		value, ok := source[i].Val.(uint8)
		if !ok {
			if integer, ok := source[i].Val.(int); ok {
				out[i] = byte(integer)
				continue
			}
			panic(fmt.Sprintf("KAT byte %d has unexpected type %T", i, source[i].Val))
		}
		out[i] = value
	}
	return out
}
