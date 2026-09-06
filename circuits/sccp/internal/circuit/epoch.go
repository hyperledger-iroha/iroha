package circuit

import (
	"crypto/sha256"
	"encoding/binary"
	"fmt"
	"math/big"

	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/cmp"
	"github.com/consensys/gnark/std/math/uints"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

var epochSignalRoles = [11]string{
	"current-anchor-hash",
	"next-anchor-hash",
	"next-snapshot-hash",
	"next-context-id",
	"activation-height",
	"transition-block-hash",
	"taira-chain-id-hash",
	"transition-finality-artifact-hash",
	"next-roster-hash",
	"deployment-policy-hash",
	"independent-circuit-key-id",
}

// EpochSnapshotWitness is the exact snapshot activated by an anchor transition.
type EpochSnapshotWitness struct {
	CurrentEpoch       frontend.Variable
	NextEpoch          frontend.Variable
	NextEpochEndHeight frontend.Variable
	Mode               frontend.Variable
	ValidatorCount     frontend.Variable
	ValidatorKeyHashes [MaxValidators][32]uints.U8
	ValidatorPoPHashes [MaxValidators][32]uints.U8
	LeaderSeed         [32]uints.U8
}

// EpochAnchorCircuit constrains one authenticated monotonic anchor and
// validator-snapshot transition for a fixed destination profile.
type EpochAnchorCircuit struct {
	PublicSignals [11]frontend.Variable `gnark:",public"`
	RawSignals    [11][32]uints.U8

	CurrentAnchor AnchorWitness
	NextAnchor    AnchorWitness
	// CurrentAnchor authorizes the exact current epoch and roster.
	// BoundaryFinality independently authenticates that roster's epoch-ending
	// context carrying the next snapshot. Finality authenticates its exact
	// successor and becomes the retained anchor for the following advance.
	BoundaryFinality FinalityWitness
	Finality         FinalityWitness
	NextRosterPoPs   PoPBatchWitness
	Snapshot         EpochSnapshotWitness

	cfg profile.Config
}

// NewEpochAnchor returns the fixed epoch-anchor circuit for a closed profile.
func NewEpochAnchor(cfg profile.Config) (*EpochAnchorCircuit, error) {
	if err := profile.ValidateClosed(cfg); err != nil {
		return nil, err
	}
	if cfg.Role != profile.EpochAnchorUpdate {
		return nil, fmt.Errorf("profile %q is not an epoch-anchor-update circuit", cfg.ID)
	}
	return &EpochAnchorCircuit{cfg: cfg}, nil
}

// Define implements frontend.Circuit.
func (c *EpochAnchorCircuit) Define(api frontend.API) error {
	if err := profile.ValidateClosed(c.cfg); err != nil {
		return fmt.Errorf("epoch-anchor circuit profile: %w", err)
	}
	if c.cfg.Role != profile.EpochAnchorUpdate {
		return fmt.Errorf("epoch-anchor circuit has no fixed closed profile")
	}
	if err := constrainAnchorHash(api, &c.CurrentAnchor, c.RawSignals[0]); err != nil {
		return err
	}
	if err := constrainAnchorHash(api, &c.NextAnchor, c.RawSignals[1]); err != nil {
		return err
	}
	if err := constrainFinalityStructure(
		api,
		&c.BoundaryFinality,
		c.finalityBatchContext("epoch-boundary"),
		nil,
	); err != nil {
		return err
	}
	if err := constrainFinalityStructure(
		api,
		&c.Finality,
		c.finalityBatchContext("epoch-successor"),
		&c.NextRosterPoPs,
	); err != nil {
		return err
	}
	if err := c.constrainTransition(api); err != nil {
		return err
	}
	if err := c.constrainSnapshot(api); err != nil {
		return err
	}
	return c.constrainPublicSignals(api)
}

func (c *EpochAnchorCircuit) finalityBatchContext(role string) []uints.U8 {
	context := constants([]byte("sccp:final-v1:bls-batch-context:" + role + ":v1"))
	context = appendConstantVec(context, []byte(c.cfg.ID))
	context = append(context, uints.NewU8(c.cfg.BackendTag), uints.NewU8(c.cfg.TargetNetworkTag))
	var target [4]byte
	binary.LittleEndian.PutUint32(target[:], c.cfg.TargetDomain)
	context = append(context, constants(target[:])...)
	for index := range c.RawSignals {
		context = append(context, c.RawSignals[index][:]...)
	}
	return context
}

func (c *EpochAnchorCircuit) constrainTransition(api frontend.API) error {
	if err := constrainCurrentAnchorAuthorization(
		api,
		&c.CurrentAnchor,
		&c.BoundaryFinality,
	); err != nil {
		return err
	}
	api.AssertIsEqual(
		api.Add(c.BoundaryFinality.Height, 1),
		c.NextAnchor.CheckpointHeight,
	)
	api.AssertIsEqual(c.BoundaryFinality.HasNextEpochSnapshot, 1)
	api.AssertIsEqual(c.NextAnchor.CheckpointHeight, c.Finality.Height)
	api.AssertIsEqual(c.NextAnchor.Epoch, c.Finality.Epoch)
	api.AssertIsEqual(c.NextAnchor.EpochEndHeight, c.Finality.EpochEndHeight)
	nextRoster, err := finalityRosterCommitment(api, &c.Finality)
	if err != nil {
		return err
	}
	if err := assertBytesEqual(api, c.NextAnchor.RosterCommitment[:], nextRoster[:]); err != nil {
		return err
	}
	if err := assertBytesEqual(api, c.NextAnchor.CheckpointBlockHash[:], c.Finality.BlockHeaderHash[:]); err != nil {
		return err
	}
	if err := assertBytesEqual(api, c.NextAnchor.CheckpointContextID[:], c.Finality.HeightContextID[:]); err != nil {
		return err
	}
	if err := assertBytesEqual(api, c.NextAnchor.CheckpointFinalityArtifactHash[:], c.Finality.FinalityArtifactHash[:]); err != nil {
		return err
	}
	// The successor HeightContext carries the exact parent CommitQC finalized by
	// BoundaryFinality. HeightContext::id() separately hashes the canonical v5
	// ParentCommitIdentity projection, which intentionally omits replaceable
	// signer/signature evidence; the V2FinalityArtifact byte binding below still
	// authenticates that full evidence without weakening the stable context ID.
	api.AssertIsEqual(c.Finality.ParentHeight, c.BoundaryFinality.Height)
	api.AssertIsEqual(c.Finality.ParentRoundView, c.BoundaryFinality.RoundView)
	api.AssertIsEqual(c.Finality.ParentProposalView, c.BoundaryFinality.ProposalView)
	// The parent QC evidence embedded in the successor HeightContext is the
	// exact CommitQC just verified for BoundaryFinality.
	boundarySignerCount := frontend.Variable(0)
	for _, bit := range c.BoundaryFinality.SignerBitmap {
		boundarySignerCount = api.Add(boundarySignerCount, bit)
	}
	api.AssertIsEqual(c.Finality.ParentSignerCount, boundarySignerCount)
	for index := 0; index < MaxValidators; index++ {
		api.AssertIsEqual(
			c.Finality.ParentSignerIndices[index],
			c.BoundaryFinality.SignerIndices[index],
		)
	}
	for _, pair := range [][2][]uints.U8{
		{c.Finality.ParentContextID[:], c.BoundaryFinality.HeightContextID[:]},
		{c.Finality.ParentSubjectParentBlockHash[:], c.BoundaryFinality.SubjectParentBlockHash[:]},
		{c.Finality.ParentBlockHash[:], c.BoundaryFinality.BlockHeaderHash[:]},
		{c.Finality.SubjectParentBlockHash[:], c.BoundaryFinality.BlockHeaderHash[:]},
		{c.Finality.ParentPayloadHash[:], c.BoundaryFinality.SubjectPayloadHash[:]},
		{c.Finality.ParentAggregateSignature[:], c.BoundaryFinality.AggregateSignature[:]},
	} {
		if err := assertBytesEqual(api, pair[0], pair[1]); err != nil {
			return err
		}
	}
	if err := assertExecutionCommitmentsEqual(
		api,
		&c.Finality.ParentExecution,
		&c.BoundaryFinality.Execution,
	); err != nil {
		return err
	}
	for i := 0; i < 24; i++ {
		api.AssertIsEqual(c.RawSignals[4][i].Val, 0)
	}
	height, err := bytesToFieldBE(api, c.RawSignals[4][24:])
	if err != nil {
		return err
	}
	api.AssertIsEqual(height, c.NextAnchor.CheckpointHeight)
	for _, pair := range [][2][]uints.U8{
		{c.RawSignals[3][:], c.Finality.HeightContextID[:]},
		{c.RawSignals[5][:], c.Finality.BlockHeaderHash[:]},
		{c.RawSignals[7][:], c.Finality.FinalityArtifactHash[:]},
	} {
		if err := assertBytesEqual(api, pair[0], pair[1]); err != nil {
			return err
		}
	}
	chainIDHash, err := hashFixed(api, profile.KeccakSignal, constants(tairaChainID))
	if err != nil {
		return err
	}
	if err := assertBytesEqual(api, c.RawSignals[6][:], chainIDHash); err != nil {
		return err
	}
	for i, b := range c.cfg.IndependentKeyID {
		api.AssertIsEqual(c.RawSignals[10][i].Val, b)
	}
	if err := nonZeroBytes(api, c.RawSignals[9][:]); err != nil {
		return err
	}
	return nil
}

// constrainCurrentAnchorAuthorization treats the retained anchor as the
// authenticated authorization for one exact epoch and ordered roster. The
// anchor may have been emitted at the first block of that epoch, so its
// checkpoint is not required to alias the later boundary block. The boundary
// QC is independently verified by constrainFinalityStructure under the roster
// committed here. If an externally installed anchor already names the
// boundary height, its complete block/context/artifact identity must agree.
func constrainCurrentAnchorAuthorization(
	api frontend.API,
	current *AnchorWitness,
	boundary *FinalityWitness,
) error {
	comparator := cmp.NewBoundedComparator(api, new(big.Int).Lsh(big.NewInt(1), 64), false)
	comparator.AssertIsLessEq(current.CheckpointHeight, boundary.Height)
	api.AssertIsEqual(current.Epoch, boundary.Epoch)
	api.AssertIsEqual(current.EpochEndHeight, boundary.EpochEndHeight)
	api.AssertIsEqual(boundary.Height, boundary.EpochEndHeight)

	currentRoster, err := finalityRosterCommitment(api, boundary)
	if err != nil {
		return err
	}
	if err := assertBytesEqual(api, current.RosterCommitment[:], currentRoster[:]); err != nil {
		return err
	}

	sameCheckpoint := api.IsZero(api.Sub(current.CheckpointHeight, boundary.Height))
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	for _, pair := range [][2][]uints.U8{
		{current.CheckpointBlockHash[:], boundary.BlockHeaderHash[:]},
		{current.CheckpointContextID[:], boundary.HeightContextID[:]},
		{current.CheckpointFinalityArtifactHash[:], boundary.FinalityArtifactHash[:]},
	} {
		for index := range pair[0] {
			api.AssertIsEqual(
				api.Mul(
					sameCheckpoint,
					api.Sub(byteAPI.Value(pair[0][index]), byteAPI.Value(pair[1][index])),
				),
				0,
			)
		}
	}
	return nil
}

func (c *EpochAnchorCircuit) constrainSnapshot(api frontend.API) error {
	api.AssertIsEqual(c.Snapshot.NextEpoch, api.Add(c.Snapshot.CurrentEpoch, 1))
	api.AssertIsEqual(c.Snapshot.CurrentEpoch, c.BoundaryFinality.Epoch)
	api.AssertIsEqual(c.Snapshot.NextEpoch, c.Finality.Epoch)
	api.AssertIsEqual(c.Snapshot.NextEpochEndHeight, c.Finality.EpochEndHeight)
	api.AssertIsEqual(c.Snapshot.Mode, sumeragiModeNPoS)
	api.AssertIsEqual(c.Finality.Mode, c.Snapshot.Mode)
	api.AssertIsEqual(c.Snapshot.ValidatorCount, c.Finality.ValidatorCount)
	api.AssertIsEqual(c.BoundaryFinality.NextEpochSnapshot.Epoch, c.Snapshot.NextEpoch)
	api.AssertIsEqual(
		c.BoundaryFinality.NextEpochSnapshot.EpochEndHeight,
		c.Snapshot.NextEpochEndHeight,
	)
	api.AssertIsEqual(c.BoundaryFinality.NextEpochSnapshot.Mode, c.Snapshot.Mode)
	api.AssertIsEqual(
		c.BoundaryFinality.NextEpochSnapshot.ValidatorCount,
		c.Snapshot.ValidatorCount,
	)
	comparator := cmp.NewBoundedComparator(api, new(big.Int).Lsh(big.NewInt(1), 64), false)
	comparator.AssertIsLessEq(c.NextAnchor.CheckpointHeight, c.Snapshot.NextEpochEndHeight)
	countComparator := cmp.NewBoundedComparator(api, big.NewInt(MaxValidators+1), false)
	countComparator.AssertIsLessEq(4, c.Snapshot.ValidatorCount)
	countComparator.AssertIsLessEq(c.Snapshot.ValidatorCount, MaxValidators)
	allowedCounts := []int{4, 7, 10, 13, 16, 19, 22, 25, 28, 31}
	selectors := make([]frontend.Variable, len(allowedCounts))
	for i, count := range allowedCounts {
		selectors[i] = api.IsZero(api.Sub(c.Snapshot.ValidatorCount, count))
	}
	api.AssertIsEqual(sumVariables(api, selectors), 1)
	for i := 0; i < MaxValidators; i++ {
		active := countComparator.IsLess(i, c.Snapshot.ValidatorCount)
		if err := canonicalActiveDigest(api, active, c.Snapshot.ValidatorKeyHashes[i][:]); err != nil {
			return err
		}
		if err := canonicalActiveDigest(api, active, c.Snapshot.ValidatorPoPHashes[i][:]); err != nil {
			return err
		}
		if err := assertBytesEqual(
			api,
			c.Snapshot.ValidatorKeyHashes[i][:],
			c.Finality.ValidatorKeyHashes[i][:],
		); err != nil {
			return err
		}
		if err := assertBytesEqual(
			api,
			c.Snapshot.ValidatorPoPHashes[i][:],
			c.Finality.ValidatorPoPHashes[i][:],
		); err != nil {
			return err
		}
		if err := assertBytesEqual(
			api,
			c.BoundaryFinality.NextEpochSnapshot.ValidatorPublicKeys[i][:],
			c.Finality.ValidatorPublicKeys[i][:],
		); err != nil {
			return err
		}
		if err := assertBytesEqual(
			api,
			c.BoundaryFinality.NextEpochSnapshot.ValidatorPoPs[i][:],
			c.NextRosterPoPs.ValidatorPoPs[i][:],
		); err != nil {
			return err
		}
	}
	if err := assertActiveDigestsDistinct(api, c.Snapshot.ValidatorCount, &c.Snapshot.ValidatorKeyHashes); err != nil {
		return err
	}
	if err := nonZeroBytes(api, c.Snapshot.LeaderSeed[:]); err != nil {
		return err
	}
	if err := assertBytesEqual(api, c.Snapshot.LeaderSeed[:], c.Finality.LeaderSeed[:]); err != nil {
		return err
	}
	if err := assertBytesEqual(
		api,
		c.BoundaryFinality.NextEpochSnapshot.LeaderSeed[:],
		c.Snapshot.LeaderSeed[:],
	); err != nil {
		return err
	}
	u64api, err := uints.New[uints.U64](api)
	if err != nil {
		return err
	}
	u32api, err := uints.New[uints.U32](api)
	if err != nil {
		return err
	}
	roster := constants([]byte("sccp:epoch-anchor:roster:v1"))
	roster = append(roster, u32api.UnpackLSB(u32api.ValueOf(c.Snapshot.ValidatorCount))...)
	for i := 0; i < MaxValidators; i++ {
		roster = append(roster, c.Snapshot.ValidatorKeyHashes[i][:]...)
		roster = append(roster, c.Snapshot.ValidatorPoPHashes[i][:]...)
	}
	rosterBase := len("sccp:epoch-anchor:roster:v1") + 4
	rosterDigest, err := hashVariable(api, profile.SHA256Signal, roster, api.Add(rosterBase, api.Mul(c.Snapshot.ValidatorCount, 64)))
	if err != nil {
		return err
	}
	if err := assertBytesEqual(api, c.RawSignals[8][:], rosterDigest); err != nil {
		return err
	}
	snapshot := constants([]byte("sccp:epoch-anchor:snapshot:v1"))
	snapshot = append(snapshot, uints.NewU8(1), uints.U8{Val: c.Snapshot.Mode})
	snapshot = append(snapshot, u64api.UnpackLSB(u64api.ValueOf(c.Snapshot.NextEpoch))...)
	snapshot = append(snapshot, u64api.UnpackLSB(u64api.ValueOf(c.Snapshot.NextEpochEndHeight))...)
	snapshot = append(snapshot, rosterDigest...)
	snapshot = append(snapshot, c.Snapshot.LeaderSeed[:]...)
	snapshotDigest, err := hashFixed(api, profile.SHA256Signal, snapshot)
	if err != nil {
		return err
	}
	return assertBytesEqual(api, c.RawSignals[2][:], snapshotDigest)
}

func (c *EpochAnchorCircuit) constrainPublicSignals(api frontend.API) error {
	for i, role := range epochSignalRoles {
		label := fmt.Sprintf("sccp:groth16:%s:epoch-anchor:signal:%s:v1", c.cfg.Curve, role)
		var labelDigest []uints.U8
		if c.cfg.SignalHash == profile.KeccakSignal {
			computed, err := hashFixed(api, c.cfg.SignalHash, constants([]byte(label)))
			if err != nil {
				return err
			}
			labelDigest = computed
		} else {
			native := sha256.Sum256([]byte(label))
			labelDigest = constants(native[:])
		}
		preimage := append(labelDigest, c.RawSignals[i][:]...)
		digest, err := hashFixed(api, c.cfg.SignalHash, preimage)
		if err != nil {
			return err
		}
		word, err := bytesToFieldBE(api, digest)
		if err != nil {
			return err
		}
		api.AssertIsEqual(word, c.PublicSignals[i])
	}
	return nil
}

// encodeUint64BEWord returns the public-signal raw value for a u64.
func encodeUint64BEWord(value uint64) [32]byte {
	var out [32]byte
	binary.BigEndian.PutUint64(out[24:], value)
	return out
}
