package circuit

import (
	"crypto/sha256"
	"encoding/binary"
	"fmt"
	"math/big"

	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/algebra/emulated/sw_bls12381"
	"github.com/consensys/gnark/std/math/cmp"
	"github.com/consensys/gnark/std/math/uints"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

const (
	// MaxCanonicalPayloadBytes is the closed first-release TON boundary and is
	// sufficient for every exact XOR launch profile.
	MaxCanonicalPayloadBytes = 374
	// MaxMerkleDepth is the final-V1 bounded SCCP message-inclusion depth.
	MaxMerkleDepth = 64
	// MaxValidators is the final-V1 Sumeragi committee bound.
	MaxValidators = 31
	// MaxHeightContextIdentityBytes bounds the exact bare-Norito
	// HeightContextIdentity v5 payload accepted by the circuit.  The largest
	// legal value contains two 31-member BLS-normal rosters and 96-byte PoPs.
	MaxHeightContextIdentityBytes = 12_288
)

var (
	tairaChainID = []byte{0xfc, 0x56, 0x98, 0x4b, 0x2b, 0xe7, 0x43, 0x1d, 0x84, 0x0e, 0x21, 0x51, 0x4d, 0x18, 0x83, 0xf0}
	// Exact genesis-derived NetworkId compiled by iroha_sccp for Taira
	// finality. It is distinct from the public UUID-shaped chain label above.
	tairaFinalityNetworkID = [32]byte{
		0x82, 0x53, 0x1c, 0xe8, 0xea, 0xe8, 0xbf, 0xf6,
		0xbe, 0xec, 0xa4, 0x69, 0x8b, 0xfd, 0x13, 0xa3,
		0xbc, 0x8b, 0xec, 0x5f, 0x0e, 0xe0, 0xd2, 0x3d,
		0x42, 0x8c, 0x97, 0xfc, 0x17, 0xab, 0x0f, 0x3b,
	}
	signalLabelsBN254 = [11]string{
		"sccp:groth16-bn254:signal:message-id:v1",
		"sccp:groth16-bn254:signal:payload-hash:v1",
		"sccp:groth16-bn254:signal:target-domain:v1",
		"sccp:groth16-bn254:signal:commitment-root:v1",
		"sccp:groth16-bn254:signal:finality-height:v1",
		"sccp:groth16-bn254:signal:finality-block-hash:v1",
		"sccp:groth16-bn254:signal:source-domain:v1",
		"sccp:groth16-bn254:signal:statement-hash:v1",
		"sccp:groth16-bn254:signal:destination-binding-hash:v1",
		"sccp:groth16-bn254:signal:route-configuration-hash:v1",
		"sccp:groth16-bn254:signal:sora-finality-anchor-hash:v1",
	}
	signalLabelsBLS12381 = [11]string{
		"sccp:groth16-bls12381:signal:message-id:v1",
		"sccp:groth16-bls12381:signal:payload-hash:v1",
		"sccp:groth16-bls12381:signal:target-domain:v1",
		"sccp:groth16-bls12381:signal:commitment-root:v1",
		"sccp:groth16-bls12381:signal:finality-height:v1",
		"sccp:groth16-bls12381:signal:finality-block-hash:v1",
		"sccp:groth16-bls12381:signal:source-domain:v1",
		"sccp:groth16-bls12381:signal:statement-hash:v1",
		"sccp:groth16-bls12381:signal:destination-binding-hash:v1",
		"sccp:groth16-bls12381:signal:route-config-hash:v1",
		"sccp:groth16-bls12381:signal:sora-finality-anchor-hash:v1",
	}
)

// AnchorWitness is the exact governed Taira checkpoint selected by a message.
type AnchorWitness struct {
	CheckpointHeight               frontend.Variable
	Epoch                          frontend.Variable
	EpochEndHeight                 frontend.Variable
	RosterCommitment               [32]uints.U8
	CheckpointBlockHash            [32]uints.U8
	CheckpointContextID            [32]uints.U8
	CheckpointFinalityArtifactHash [32]uints.U8
}

// FinalityWitness is the bounded, structured Sumeragi-v2 v4 artifact used by
// both message and epoch-transition circuits. Redundant fields are intentional:
// the circuit derives the context, vote, and artifact commitments and checks
// every parent/QC/subject linkage instead of accepting a host-side verdict bit.
//
// Raw BLS-normal key/signature material is verified by the cryptographic layer,
// while the byte-exact Norito encoder binds the complete artifact. Release
// production remains source-level fail-closed until fixed-verifier, ceremony,
// audit, resource, and destination-runtime closure is complete.
type FinalityWitness struct {
	Height                       frontend.Variable
	ContextHeight                frontend.Variable
	CertificateHeight            frontend.Variable
	Epoch                        frontend.Variable
	EpochEndHeight               frontend.Variable
	HasNextEpochSnapshot         frontend.Variable
	NextEpochSnapshot            NextEpochSnapshotWitness
	Mode                         frontend.Variable
	RoundView                    frontend.Variable
	ProposalView                 frontend.Variable
	BlockHeaderView              frontend.Variable
	ValidatorCount               frontend.Variable
	SignerBitmap                 [MaxValidators]frontend.Variable
	SignerIndices                [MaxValidators]frontend.Variable
	ValidatorPublicKeys          [MaxValidators][48]uints.U8
	ValidatorPoPs                [MaxValidators][96]uints.U8
	ValidatorKeyHashes           [MaxValidators][32]uints.U8
	ValidatorPoPHashes           [MaxValidators][32]uints.U8
	SubjectParentBlockHash       [32]uints.U8
	SubjectPayloadHash           [32]uints.U8
	Execution                    ExecutionCommitmentWitness
	ParentContextID              [32]uints.U8
	ParentHeight                 frontend.Variable
	ParentRoundView              frontend.Variable
	ParentProposalView           frontend.Variable
	ParentSignerCount            frontend.Variable
	ParentSignerIndices          [MaxValidators]frontend.Variable
	ParentAggregateSignature     [96]uints.U8
	ParentSubjectParentBlockHash [32]uints.U8
	ParentBlockHash              [32]uints.U8
	ParentPayloadHash            [32]uints.U8
	ParentExecution              ExecutionCommitmentWitness
	NexusAMXContextHash          [32]uints.U8
	ExecutionPolicyHash          [32]uints.U8
	DALayout                     DataAvailabilityLayoutWitness
	LeaderSeed                   [32]uints.U8
	AggregateSignatureHash       [32]uints.U8
	AggregateSignature           [96]uints.U8
	AggregateSignaturePoint      sw_bls12381.G2Affine
	VoteSignaturePayloadLength   frontend.Variable
	VoteSignaturePayload         [maxVoteSignaturePayloadBytes]uints.U8
	VotePreimageHash             [32]uints.U8
	HeightContextID              [32]uints.U8
	BlockHeaderHash              [32]uints.U8
	FinalityArtifactHash         [32]uints.U8
	FinalityArtifactLength       frontend.Variable
	FinalityArtifactBytes        [maxFinalityArtifactBytes]uints.U8
}

// NextEpochSnapshotWitness is the complete FinalizedNextEpochSnapshot carried
// by a boundary HeightContext.  Fixed-width BLS-normal keys and PoPs are a
// stricter, canonical subset of the Rust wire bounds; inactive slots must be
// all zero and are never serialized.
type NextEpochSnapshotWitness struct {
	Epoch               frontend.Variable
	EpochEndHeight      frontend.Variable
	Mode                frontend.Variable
	ValidatorCount      frontend.Variable
	ValidatorPublicKeys [MaxValidators][48]uints.U8
	ValidatorPoPs       [MaxValidators][96]uints.U8
	LeaderSeed          [32]uints.U8
}

// DataAvailabilityLayoutWitness is the complete revision-4 RS16 layout.  The
// quotient and remainder are explicit arithmetic witnesses for the exact
// ceil(max_payload/(chunk_size*data_shards)) capacity calculation.
type DataAvailabilityLayoutWitness struct {
	ChunkSizeBytes        frontend.Variable
	DataShards            frontend.Variable
	ParityShards          frontend.Variable
	MaxPayloadSizeBytes   frontend.Variable
	MaxChunkCount         frontend.Variable
	RequiredStripes       frontend.Variable
	LastStripePayloadSize frontend.Variable
}

// PoPBatchWitness is present only in an epoch-anchor circuit for the newly
// activated successor roster. Message and current-roster paths therefore have
// no unconstrained raw PoP inputs and rely on the governed anchor's committed
// PoP hashes.
type PoPBatchWitness struct {
	ValidatorPoPs      [MaxValidators][96]uints.U8
	ValidatorPoPPoints [MaxValidators]sw_bls12381.G2Affine
}

// MessageCircuit constrains a fixed lane's canonical transfer, message leaf,
// bounded inclusion proof, exact signed block-header projection, anchor,
// Sumeragi finality, and eleven signals.
type MessageCircuit struct {
	PublicSignals [11]frontend.Variable `gnark:",public"`
	RawSignals    [11][32]uints.U8

	Payload       [MaxCanonicalPayloadBytes]uints.U8
	PayloadLength frontend.Variable
	MerkleDepth   frontend.Variable
	Siblings      [MaxMerkleDepth][32]uints.U8
	SiblingIsLeft [MaxMerkleDepth]frontend.Variable

	Anchor      AnchorWitness
	Finality    FinalityWitness
	Statement   StatementWitness
	SenderI105  I105Witness
	BlockHeader BlockHeaderProjectionWitness

	cfg profile.Config
}

// NewMessage returns the fixed message circuit for an exact closed profile.
func NewMessage(cfg profile.Config) (*MessageCircuit, error) {
	if err := profile.ValidateClosed(cfg); err != nil {
		return nil, err
	}
	if cfg.Role != profile.Message {
		return nil, fmt.Errorf("profile %q is not a message circuit", cfg.ID)
	}
	return &MessageCircuit{cfg: cfg}, nil
}

// Define implements frontend.Circuit.
func (c *MessageCircuit) Define(api frontend.API) error {
	if err := profile.ValidateClosed(c.cfg); err != nil {
		return fmt.Errorf("message circuit profile: %w", err)
	}
	if c.cfg.Role != profile.Message {
		return fmt.Errorf("message circuit has no fixed closed profile")
	}
	if err := c.constrainRawSignalRoles(api); err != nil {
		return err
	}
	if err := c.constrainTransfer(api); err != nil {
		return err
	}
	if err := c.constrainPayloadAndMessageHashes(api); err != nil {
		return err
	}
	if err := c.constrainMerkleInclusion(api); err != nil {
		return err
	}
	if err := c.constrainAnchor(api); err != nil {
		return err
	}
	if err := constrainFinalityStructure(
		api,
		&c.Finality,
		c.finalityBatchContext("message"),
		nil,
	); err != nil {
		return err
	}
	if err := c.constrainBlockHeaderCommitment(api); err != nil {
		return err
	}
	if err := c.constrainSemanticStatement(api); err != nil {
		return err
	}
	return c.constrainPublicSignals(api)
}

// finalityBatchContext binds the pairing-batch challenge to the immutable
// circuit profile and the complete public SCCP statement. RawSignals are full
// 32-byte values and constrainPublicSignals subsequently binds each one to its
// curve-specific public field signal, so no witness-supplied batch randomness
// exists.
func (c *MessageCircuit) finalityBatchContext(role string) []uints.U8 {
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

func (c *MessageCircuit) constrainRawSignalRoles(api frontend.API) error {
	// Exact byte roles: message, payload, root, finality block, destination,
	// route, and anchor are copied from their independently constrained inputs.
	if err := assertBytesEqual(api, c.RawSignals[5][:], c.Finality.BlockHeaderHash[:]); err != nil {
		return err
	}
	for _, index := range []int{0, 1, 3, 5, 8, 9, 10} {
		if err := nonZeroBytes(api, c.RawSignals[index][:]); err != nil {
			return err
		}
	}
	for i := 0; i < 28; i++ {
		api.AssertIsEqual(c.RawSignals[2][i].Val, 0)
		api.AssertIsEqual(c.RawSignals[6][i].Val, 0)
	}
	var target [4]byte
	binary.BigEndian.PutUint32(target[:], c.cfg.TargetDomain)
	for i := range target {
		api.AssertIsEqual(c.RawSignals[2][28+i].Val, target[i])
	}
	for i := 28; i < 32; i++ { // source SCCP domain is exactly zero
		api.AssertIsEqual(c.RawSignals[6][i].Val, 0)
	}
	for i := 0; i < 24; i++ {
		api.AssertIsEqual(c.RawSignals[4][i].Val, 0)
	}
	height, err := bytesToFieldBE(api, c.RawSignals[4][24:])
	if err != nil {
		return err
	}
	api.AssertIsEqual(height, c.Finality.Height)
	return nil
}

func (c *MessageCircuit) constrainTransfer(api frontend.API) error {
	p := c.Payload[:]
	assertByte := func(index int, expected byte) {
		api.AssertIsEqual(p[index].Val, expected)
	}
	assertLE32 := func(offset int, expected uint32) {
		var bytes [4]byte
		binary.LittleEndian.PutUint32(bytes[:], expected)
		for i := range bytes {
			assertByte(offset+i, bytes[i])
		}
	}
	assertByte(0, 2) // SccpPayloadV1::Transfer discriminant
	assertByte(1, 1)
	assertLE32(2, 0) // SORA domain
	assertLE32(6, c.cfg.TargetDomain)
	if err := nonZeroBytes(api, p[18:22]); err != nil { // route revision
		return err
	}
	assertLE32(22, 0) // XOR is native to SORA
	assertByte(26, 1)
	assertLE32(27, 3)
	for i, b := range []byte("xor") {
		assertByte(31+i, b)
	}
	if err := nonZeroBytes(api, p[34:50]); err != nil { // positive u128 amount
		return err
	}
	assertByte(50, 1) // canonical-text sender
	senderLength, err := leBytesToVariable(api, p[51:55])
	if err != nil {
		return err
	}
	comparator := cmp.NewBoundedComparator(api, big.NewInt(MaxCanonicalPayloadBytes), false)
	comparator.AssertIsLess(0, senderLength)
	comparator.AssertIsLessEq(senderLength, 256)
	if err := constrainCanonicalTairaI105(
		api,
		p[55:55+maxI105SenderBytes],
		senderLength,
		&c.SenderI105,
	); err != nil {
		return err
	}

	selectors := make([]frontend.Variable, 256)
	for senderLen := 1; senderLen <= 256; senderLen++ {
		selector := api.IsZero(api.Sub(senderLength, senderLen))
		selectors[senderLen-1] = selector
		recipientOffset := 55 + senderLen
		routeOffset := recipientOffset + 1 + 4 + c.cfg.RecipientLength
		expectedLength := routeOffset + 1 + 4 + len(c.cfg.RouteID)
		api.AssertIsEqual(api.Mul(selector, api.Sub(c.PayloadLength, expectedLength)), 0)
		conditionalByteEqual(api, selector, p[recipientOffset], c.cfg.RecipientCodec)
		var recipientLength [4]byte
		binary.LittleEndian.PutUint32(recipientLength[:], uint32(c.cfg.RecipientLength))
		for i, b := range recipientLength {
			conditionalByteEqual(api, selector, p[recipientOffset+1+i], b)
		}
		recipient := p[recipientOffset+5 : recipientOffset+5+c.cfg.RecipientLength]
		if c.cfg.RecipientCodec == 5 {
			conditionalByteEqual(api, selector, recipient[0], 0x41)
		}
		if c.cfg.RecipientCodec == 7 {
			for i := 0; i < 4; i++ { // TON basechain workchain 0, big endian
				conditionalByteEqual(api, selector, recipient[i], 0)
			}
		}
		nonZeroRecipient := recipient
		if c.cfg.RecipientCodec == 5 {
			nonZeroRecipient = recipient[1:]
		} else if c.cfg.RecipientCodec == 7 {
			nonZeroRecipient = recipient[4:]
		}
		if err := conditionalNonZeroBytes(api, selector, nonZeroRecipient); err != nil {
			return err
		}
		conditionalByteEqual(api, selector, p[routeOffset], 1)
		var routeLength [4]byte
		binary.LittleEndian.PutUint32(routeLength[:], uint32(len(c.cfg.RouteID)))
		for i, b := range routeLength {
			conditionalByteEqual(api, selector, p[routeOffset+1+i], b)
		}
		for i, b := range []byte(c.cfg.RouteID) {
			conditionalByteEqual(api, selector, p[routeOffset+5+i], b)
		}
	}
	api.AssertIsEqual(sumVariables(api, selectors), 1)
	return nil
}

func (c *MessageCircuit) constrainPayloadAndMessageHashes(api frontend.API) error {
	payloadInput := append(constants([]byte("sccp:payload:v1")), c.Payload[:]...)
	payloadDigest, err := blake2b256(api, payloadInput, api.Add(len("sccp:payload:v1"), c.PayloadLength))
	if err != nil {
		return err
	}
	if err := assertBytesEqual(api, payloadDigest[:], c.RawSignals[1][:]); err != nil {
		return err
	}

	lane := canonicalLaneBytes(c.cfg)
	prefix := []byte("sccp:lane-message-id:v1")
	base := make([]byte, 0, len(prefix)+1+4+len(lane)+4)
	base = append(base, prefix...)
	base = append(base, 1)
	base = binary.LittleEndian.AppendUint32(base, uint32(len(lane)))
	base = append(base, lane...)
	messageInput := append(constants(base), make([]uints.U8, 4)...)
	u32api, err := uints.New[uints.U32](api)
	if err != nil {
		return err
	}
	payloadLengthBytes := u32api.UnpackLSB(u32api.ValueOf(c.PayloadLength))
	copy(messageInput[len(messageInput)-4:], payloadLengthBytes)
	messageInput = append(messageInput, c.Payload[:]...)
	messageDigest, err := hashVariable(api, profile.KeccakSignal, messageInput, api.Add(len(base)+4, c.PayloadLength))
	if err != nil {
		return err
	}
	return assertBytesEqual(api, messageDigest, c.RawSignals[0][:])
}

func (c *MessageCircuit) constrainMerkleInclusion(api frontend.API) error {
	lanePreimage := append([]byte("sccp:lane-id:v1"), canonicalLaneBytes(c.cfg)...)
	laneHash, err := blake2b256(api, constants(lanePreimage), len(lanePreimage))
	if err != nil {
		return err
	}
	// The authoritative hub constructor rejects role collisions. Enforce the
	// same five-way separation before admitting the commitment leaf so a proof
	// cannot reinterpret one digest as another semantic role.
	roles := [][32]uints.U8{
		laneHash,
		c.RawSignals[8],
		c.RawSignals[9],
		c.RawSignals[0],
		c.RawSignals[1],
	}
	for left := 0; left < len(roles); left++ {
		for right := left + 1; right < len(roles); right++ {
			if err := assertBytesNotEqual(api, roles[left][:], roles[right][:]); err != nil {
				return err
			}
		}
	}
	commitment := []uints.U8{
		uints.NewU8(1), uints.NewU8(5), uints.NewU8(profile.SoraNetworkTag), uints.NewU8(c.cfg.TargetNetworkTag),
	}
	commitment = append(commitment, c.RawSignals[8][:]...)
	commitment = append(commitment, c.RawSignals[9][:]...)
	commitment = append(commitment, c.RawSignals[0][:]...)
	commitment = append(commitment, c.RawSignals[1][:]...)
	leafInput := append(constants([]byte("sccp:hub:leaf:v1")), commitment...)
	current, err := blake2b256(api, leafInput, len(leafInput))
	if err != nil {
		return err
	}
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	// The bounded comparator assumes a small absolute difference. Make that
	// assumption a constraint first, so a near-modulus field value cannot be
	// interpreted as an alternative short proof length.
	_ = byteAPI.ValueOf(c.MerkleDepth)
	comparator := cmp.NewBoundedComparator(api, big.NewInt(MaxMerkleDepth+1), false)
	comparator.AssertIsLessEq(c.MerkleDepth, MaxMerkleDepth)
	for level := 0; level < MaxMerkleDepth; level++ {
		active := comparator.IsLess(level, c.MerkleDepth)
		api.AssertIsBoolean(c.SiblingIsLeft[level])
		api.AssertIsEqual(c.SiblingIsLeft[level], api.Mul(active, c.SiblingIsLeft[level]))
		for i := range c.Siblings[level] {
			api.AssertIsEqual(c.Siblings[level][i].Val, api.Mul(active, c.Siblings[level][i].Val))
		}
		left := selectDigest(api, c.SiblingIsLeft[level], c.Siblings[level], current)
		right := selectDigest(api, c.SiblingIsLeft[level], current, c.Siblings[level])
		nodeInput := append(constants([]byte("sccp:hub:node:v1")), left[:]...)
		nodeInput = append(nodeInput, right[:]...)
		next, err := blake2b256(api, nodeInput, len(nodeInput))
		if err != nil {
			return err
		}
		current = selectDigest(api, active, next, current)
	}
	return assertBytesEqual(api, current[:], c.RawSignals[3][:])
}

func (c *MessageCircuit) constrainAnchor(api frontend.API) error {
	if err := constrainAnchorHash(api, &c.Anchor, c.RawSignals[10]); err != nil {
		return err
	}
	comparator := cmp.NewBoundedComparator(api, new(big.Int).Lsh(big.NewInt(1), 64), false)
	comparator.AssertIsLessEq(c.Anchor.CheckpointHeight, c.Finality.Height)
	api.AssertIsEqual(c.Anchor.Epoch, c.Finality.Epoch)
	api.AssertIsEqual(c.Anchor.EpochEndHeight, c.Finality.EpochEndHeight)
	roster, err := finalityRosterCommitment(api, &c.Finality)
	if err != nil {
		return err
	}
	if err := assertBytesEqual(api, c.Anchor.RosterCommitment[:], roster[:]); err != nil {
		return err
	}
	return nil
}

func constrainAnchorHash(api frontend.API, anchorWitness *AnchorWitness, expectedHash [32]uints.U8) error {
	chainIDHash, err := hashFixed(api, profile.KeccakSignal, constants(tairaChainID))
	if err != nil {
		return err
	}
	if err := nonZeroBytes(api, anchorWitness.CheckpointBlockHash[:]); err != nil {
		return err
	}
	if err := nonZeroBytes(api, anchorWitness.CheckpointContextID[:]); err != nil {
		return err
	}
	if err := nonZeroBytes(api, anchorWitness.CheckpointFinalityArtifactHash[:]); err != nil {
		return err
	}
	if err := nonZeroBytes(api, anchorWitness.RosterCommitment[:]); err != nil {
		return err
	}
	comparator := cmp.NewBoundedComparator(api, new(big.Int).Lsh(big.NewInt(1), 64), false)
	comparator.AssertIsLess(0, anchorWitness.CheckpointHeight)
	comparator.AssertIsLess(0, anchorWitness.Epoch)
	comparator.AssertIsLessEq(anchorWitness.CheckpointHeight, anchorWitness.EpochEndHeight)
	u64api, err := uints.New[uints.U64](api)
	if err != nil {
		return err
	}
	anchor := []uints.U8{uints.NewU8(1), uints.NewU8(1), uints.NewU8(4), uints.NewU8(0)}
	anchor = append(anchor, chainIDHash...)
	anchor = append(anchor, u64api.UnpackLSB(u64api.ValueOf(anchorWitness.Epoch))...)
	anchor = append(anchor, u64api.UnpackLSB(u64api.ValueOf(anchorWitness.EpochEndHeight))...)
	anchor = append(anchor, anchorWitness.RosterCommitment[:]...)
	anchor = append(anchor, u64api.UnpackLSB(u64api.ValueOf(anchorWitness.CheckpointHeight))...)
	anchor = append(anchor, anchorWitness.CheckpointBlockHash[:]...)
	anchor = append(anchor, anchorWitness.CheckpointContextID[:]...)
	anchor = append(anchor, anchorWitness.CheckpointFinalityArtifactHash[:]...)
	preimage := append(constants([]byte("sccp:sora-finality-anchor:v1")), anchor...)
	digest, err := hashFixed(api, profile.KeccakSignal, preimage)
	if err != nil {
		return err
	}
	return assertBytesEqual(api, digest, expectedHash[:])
}

func (c *MessageCircuit) constrainPublicSignals(api frontend.API) error {
	labels := signalLabelsBN254
	if c.cfg.SignalHash == profile.SHA256Signal {
		labels = signalLabelsBLS12381
	}
	for i := range c.RawSignals {
		labelDigest := sha256.Sum256(nil)
		if c.cfg.SignalHash == profile.KeccakSignal {
			computed, err := hashFixed(api, c.cfg.SignalHash, constants([]byte(labels[i])))
			if err != nil {
				return err
			}
			preimage := append(computed, c.RawSignals[i][:]...)
			digest, err := hashFixed(api, c.cfg.SignalHash, preimage)
			if err != nil {
				return err
			}
			word, err := bytesToFieldBE(api, digest)
			if err != nil {
				return err
			}
			api.AssertIsEqual(word, c.PublicSignals[i])
			continue
		}
		labelDigest = sha256.Sum256([]byte(labels[i]))
		preimage := append(constants(labelDigest[:]), c.RawSignals[i][:]...)
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

func conditionalByteEqual(api frontend.API, selector frontend.Variable, actual uints.U8, expected byte) {
	api.AssertIsEqual(api.Mul(selector, api.Sub(actual.Val, expected)), 0)
}

func assertBytesNotEqual(api frontend.API, left, right []uints.U8) error {
	if len(left) != len(right) || len(left) == 0 {
		return fmt.Errorf("non-equality byte-array length mismatch: %d != %d", len(left), len(right))
	}
	uapi, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	allEqual := frontend.Variable(1)
	for index := range left {
		allEqual = api.Mul(allEqual, api.IsZero(api.Sub(uapi.Value(left[index]), uapi.Value(right[index]))))
	}
	api.AssertIsEqual(allEqual, 0)
	return nil
}

func conditionalNonZeroBytes(api frontend.API, selector frontend.Variable, bytes []uints.U8) error {
	uapi, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	values := make([]frontend.Variable, len(bytes))
	for i := range bytes {
		values[i] = uapi.Value(bytes[i])
	}
	api.AssertIsEqual(api.Mul(selector, api.IsZero(sumVariables(api, values))), 0)
	return nil
}

func canonicalActiveDigest(api frontend.API, active frontend.Variable, digest []uints.U8) error {
	uapi, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	values := make([]frontend.Variable, len(digest))
	for i := range digest {
		values[i] = uapi.Value(digest[i])
		api.AssertIsEqual(values[i], api.Mul(active, values[i]))
	}
	api.AssertIsEqual(api.Mul(active, api.IsZero(sumVariables(api, values))), 0)
	return nil
}

func assertActiveDigestsDistinct(
	api frontend.API,
	count frontend.Variable,
	digests *[MaxValidators][32]uints.U8,
) error {
	// Split each 256-bit digest into two injective 128-bit field elements; this
	// avoids accidental equality after scalar-field reduction on BN254.
	var halves [MaxValidators][2]frontend.Variable
	for index := range digests {
		for half := range halves[index] {
			value, err := bytesToFieldBE(api, digests[index][half*16:(half+1)*16])
			if err != nil {
				return err
			}
			halves[index][half] = value
		}
	}
	comparator := cmp.NewBoundedComparator(api, big.NewInt(MaxValidators+1), false)
	for right := 1; right < MaxValidators; right++ {
		active := comparator.IsLess(right, count)
		for left := 0; left < right; left++ {
			equalHigh := api.IsZero(api.Sub(halves[left][0], halves[right][0]))
			equalLow := api.IsZero(api.Sub(halves[left][1], halves[right][1]))
			api.AssertIsEqual(api.Mul(active, equalHigh, equalLow), 0)
		}
	}
	return nil
}

func selectDigest(api frontend.API, selector frontend.Variable, whenTrue, whenFalse [32]uints.U8) [32]uints.U8 {
	var out [32]uints.U8
	for i := range out {
		out[i] = uints.U8{Val: api.Select(selector, whenTrue[i].Val, whenFalse[i].Val)}
	}
	return out
}

func canonicalLaneBytes(cfg profile.Config) []byte {
	source, target := canonicalNetworkBytes(cfg)
	out := []byte{1}
	out = binary.LittleEndian.AppendUint32(out, uint32(len(source)))
	out = append(out, source...)
	out = binary.LittleEndian.AppendUint32(out, uint32(len(target)))
	out = append(out, target...)
	return out
}

func canonicalNetworkBytes(cfg profile.Config) ([]byte, []byte) {
	source := []byte{1, profile.SoraNetworkTag, 0, 0, 0, 0}
	source = append(source, tairaChainID...)
	target := []byte{1, cfg.TargetNetworkTag}
	target = binary.LittleEndian.AppendUint32(target, cfg.TargetDomain)
	switch cfg.Lane {
	case "ethereum-mainnet":
		target = binary.LittleEndian.AppendUint64(target, 1)
	case "bsc-mainnet":
		target = binary.LittleEndian.AppendUint64(target, 56)
	case "tron-mainnet":
		target = binary.LittleEndian.AppendUint32(target, 0x2b6653dc)
	case "ton-mainnet":
		target = binary.LittleEndian.AppendUint32(target, 0xffffff11)
		target = binary.LittleEndian.AppendUint32(target, ^uint32(0)) // masterchain workchain -1
		target = binary.LittleEndian.AppendUint64(target, 0x8000000000000000)
		target = binary.LittleEndian.AppendUint32(target, 0)
		target = append(target,
			0x17, 0xa3, 0xa9, 0x29, 0x92, 0xaa, 0xbe, 0xa7, 0x85, 0xa7, 0xa0, 0x90, 0x98, 0x5a, 0x26, 0x5c,
			0xd3, 0x1f, 0x32, 0x3d, 0x84, 0x9d, 0xa5, 0x12, 0x39, 0x73, 0x7e, 0x32, 0x1f, 0xb0, 0x55, 0x69,
		)
		target = append(target,
			0x5e, 0x99, 0x4f, 0xcf, 0x4d, 0x42, 0x5c, 0x0a, 0x6c, 0xe6, 0xa7, 0x92, 0x59, 0x4b, 0x71, 0x73,
			0x20, 0x5f, 0x74, 0x0a, 0x39, 0xcd, 0x56, 0xf5, 0x37, 0xde, 0xfd, 0x28, 0xb4, 0x8a, 0x0f, 0x6e,
		)
	default:
		panic("unreachable closed profile")
	}
	return source, target
}
