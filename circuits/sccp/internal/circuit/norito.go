package circuit

import (
	"encoding/binary"
	"fmt"
	"math/big"

	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/cmp"
	"github.com/consensys/gnark/std/math/uints"
)

const (
	maxVoteSignaturePayloadBytes = 524
	maxExecutedBlockWireBytes    = 256 * 1024 * 1024
	maxTopupAnchorsPerBlock      = 16
	maxNativeAMXManifestLeaves   = 1024
	maxLaneFinalityStatements    = 1024
	nativeAMXManifestVersion     = 1
	mergeCarrierVersion          = 1
)

// ExecutionCommitmentWitness is the complete canonical Sumeragi-v2
// ExecutionCommitment. Presence flags are explicit because Norito's required
// Option encoding is part of the authenticated vote bytes.
type ExecutionCommitmentWitness struct {
	ParentStateRoot                   [32]uints.U8
	PostStateRoot                     [32]uints.U8
	OrdinaryWritesRoot                [32]uints.U8
	HasTopupAnchorRoot                frontend.Variable
	TopupAnchorRoot                   [32]uints.U8
	TopupAnchorCount                  frontend.Variable
	NativeAMXApplicationManifestVer   frontend.Variable
	NativeAMXApplicationManifestRoot  [32]uints.U8
	NativeAMXApplicationManifestCount frontend.Variable
	HasLaneFinalityManifest           frontend.Variable
	LaneFinalityManifestRoot          [32]uints.U8
	LaneFinalityManifestLeafCount     frontend.Variable
	HasMergeCarrier                   frontend.Variable
	MergeCarrierVersion               frontend.Variable
	MergeCarrierEntryHash             [32]uints.U8
	ExecutedBlockWireLength           frontend.Variable
	ExecutedBlockWireHash             [32]uints.U8
}

func constrainExecutionCommitment(api frontend.API, execution *ExecutionCommitmentWitness) error {
	for _, present := range []frontend.Variable{
		execution.HasTopupAnchorRoot,
		execution.HasLaneFinalityManifest,
		execution.HasMergeCarrier,
	} {
		api.AssertIsBoolean(present)
	}
	for _, digest := range [][]uints.U8{
		execution.ParentStateRoot[:],
		execution.PostStateRoot[:],
		execution.OrdinaryWritesRoot[:],
		execution.NativeAMXApplicationManifestRoot[:],
		execution.ExecutedBlockWireHash[:],
	} {
		if err := constrainIrohaHash(api, digest); err != nil {
			return err
		}
	}
	for _, conditional := range []struct {
		present frontend.Variable
		digest  []uints.U8
	}{
		{execution.HasTopupAnchorRoot, execution.TopupAnchorRoot[:]},
		{execution.HasLaneFinalityManifest, execution.LaneFinalityManifestRoot[:]},
		{execution.HasMergeCarrier, execution.MergeCarrierEntryHash[:]},
	} {
		if err := canonicalActiveIrohaHash(api, conditional.present, conditional.digest); err != nil {
			return err
		}
	}

	if _, err := u32Bytes(api, execution.TopupAnchorCount); err != nil {
		return err
	}
	if _, err := u32Bytes(api, execution.NativeAMXApplicationManifestCount); err != nil {
		return err
	}
	if _, err := u16Bytes(api, execution.NativeAMXApplicationManifestVer); err != nil {
		return err
	}
	if _, err := u16Bytes(api, execution.MergeCarrierVersion); err != nil {
		return err
	}
	if _, err := u64Bytes(api, execution.LaneFinalityManifestLeafCount); err != nil {
		return err
	}
	if _, err := u64Bytes(api, execution.ExecutedBlockWireLength); err != nil {
		return err
	}

	comparison32 := cmp.NewBoundedComparator(api, new(big.Int).Lsh(big.NewInt(1), 32), false)
	comparison64 := cmp.NewBoundedComparator(api, new(big.Int).Lsh(big.NewInt(1), 64), false)

	topupCountIsZero := api.IsZero(execution.TopupAnchorCount)
	api.AssertIsEqual(execution.HasTopupAnchorRoot, api.Sub(1, topupCountIsZero))
	comparison32.AssertIsLessEq(execution.TopupAnchorCount, maxTopupAnchorsPerBlock)
	topupCountBytes, err := u32Bytes(api, execution.TopupAnchorCount)
	if err != nil {
		return err
	}
	topupPreimage := constants([]byte("iroha:kagemusha:v2:post-state-root"))
	topupPreimage = append(topupPreimage, uints.NewU8(0))
	topupPreimage = append(topupPreimage, topupCountBytes...)
	topupPreimage = append(topupPreimage, execution.OrdinaryWritesRoot[:]...)
	topupPreimage = append(topupPreimage, execution.TopupAnchorRoot[:]...)
	topupPostStateRoot, err := irohaBlake2bHash(api, topupPreimage)
	if err != nil {
		return err
	}
	if err := assertConditionalBytesEqual(
		api,
		execution.HasTopupAnchorRoot,
		execution.PostStateRoot[:],
		topupPostStateRoot[:],
	); err != nil {
		return err
	}

	api.AssertIsEqual(execution.NativeAMXApplicationManifestVer, nativeAMXManifestVersion)
	comparison32.AssertIsLessEq(
		execution.NativeAMXApplicationManifestCount,
		maxNativeAMXManifestLeaves,
	)
	emptyNativeAMXRoot := nativeIrohaHash(
		[]byte("iroha:sumeragi:v2:native-amx-application-manifest:v1:empty"),
	)
	nativeCountIsZero := api.IsZero(execution.NativeAMXApplicationManifestCount)
	if err := assertConditionalBytesEqual(
		api,
		nativeCountIsZero,
		execution.NativeAMXApplicationManifestRoot[:],
		constants(emptyNativeAMXRoot[:]),
	); err != nil {
		return err
	}
	nativeRootIsEmpty, err := bytesEqualIndicator(
		api,
		execution.NativeAMXApplicationManifestRoot[:],
		constants(emptyNativeAMXRoot[:]),
	)
	if err != nil {
		return err
	}
	api.AssertIsEqual(nativeRootIsEmpty, nativeCountIsZero)

	comparison64.AssertIsLessEq(
		execution.LaneFinalityManifestLeafCount,
		maxLaneFinalityStatements,
	)
	laneCountIsZero := api.IsZero(execution.LaneFinalityManifestLeafCount)
	api.AssertIsEqual(execution.HasLaneFinalityManifest, api.Sub(1, laneCountIsZero))

	api.AssertIsEqual(
		execution.MergeCarrierVersion,
		api.Mul(execution.HasMergeCarrier, mergeCarrierVersion),
	)

	comparison64.AssertIsLess(0, execution.ExecutedBlockWireLength)
	comparison64.AssertIsLessEq(execution.ExecutedBlockWireLength, maxExecutedBlockWireBytes)
	return nil
}

func canonicalActiveIrohaHash(api frontend.API, active frontend.Variable, digest []uints.U8) error {
	if err := canonicalActiveDigest(api, active, digest); err != nil {
		return err
	}
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	lastBit := api.ToBinary(byteAPI.Value(digest[len(digest)-1]), 8)[0]
	api.AssertIsEqual(lastBit, active)
	return nil
}

func assertConditionalBytesEqual(
	api frontend.API,
	condition frontend.Variable,
	left []uints.U8,
	right []uints.U8,
) error {
	if len(left) != len(right) {
		return fmt.Errorf("conditional byte length mismatch: %d != %d", len(left), len(right))
	}
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	for index := range left {
		api.AssertIsEqual(
			api.Mul(condition, api.Sub(byteAPI.Value(left[index]), byteAPI.Value(right[index]))),
			0,
		)
	}
	return nil
}

func bytesEqualIndicator(api frontend.API, left, right []uints.U8) (frontend.Variable, error) {
	if len(left) != len(right) {
		return nil, fmt.Errorf("byte equality length mismatch: %d != %d", len(left), len(right))
	}
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return nil, err
	}
	equal := frontend.Variable(1)
	for index := range left {
		equal = api.Mul(
			equal,
			api.IsZero(api.Sub(byteAPI.Value(left[index]), byteAPI.Value(right[index]))),
		)
	}
	return equal, nil
}

func noritoCompactLength(value int) []uints.U8 {
	if value < 0 {
		panic("Norito compact length cannot be negative")
	}
	encoded := make([]byte, 0, 10)
	remaining := uint64(value)
	for {
		current := byte(remaining & 0x7f)
		remaining >>= 7
		if remaining != 0 {
			current |= 0x80
		}
		encoded = append(encoded, current)
		if remaining == 0 {
			break
		}
	}
	return constants(encoded)
}

func noritoField(body []uints.U8) []uints.U8 {
	encoded := noritoCompactLength(len(body))
	return append(encoded, body...)
}

func noritoU16Field(api frontend.API, value frontend.Variable) ([]uints.U8, error) {
	encoded, err := u16Bytes(api, value)
	if err != nil {
		return nil, err
	}
	return noritoField(encoded), nil
}

func noritoU32Field(api frontend.API, value frontend.Variable) ([]uints.U8, error) {
	encoded, err := u32Bytes(api, value)
	if err != nil {
		return nil, err
	}
	return noritoField(encoded), nil
}

func noritoU64Field(api frontend.API, value frontend.Variable) ([]uints.U8, error) {
	encoded, err := u64Bytes(api, value)
	if err != nil {
		return nil, err
	}
	return noritoField(encoded), nil
}

func noritoHashField(digest []uints.U8) []uints.U8 {
	if len(digest) != 32 {
		panic("Norito hash field must contain 32 bytes")
	}
	return noritoField(digest)
}

func nativeNoritoCompactLength(value int) []byte {
	if value < 0 {
		panic("Norito compact length cannot be negative")
	}
	var scratch [10]byte
	count := binary.PutUvarint(scratch[:], uint64(value))
	return append([]byte(nil), scratch[:count]...)
}

func nativeNoritoField(body []byte) []byte {
	encoded := nativeNoritoCompactLength(len(body))
	return append(encoded, body...)
}

func nativeNoritoU16(value uint16) []byte {
	var encoded [2]byte
	binary.LittleEndian.PutUint16(encoded[:], value)
	return nativeNoritoField(encoded[:])
}

func nativeNoritoU32(value uint32) []byte {
	var encoded [4]byte
	binary.LittleEndian.PutUint32(encoded[:], value)
	return nativeNoritoField(encoded[:])
}

func nativeNoritoU64(value uint64) []byte {
	var encoded [8]byte
	binary.LittleEndian.PutUint64(encoded[:], value)
	return nativeNoritoField(encoded[:])
}

func nativeNoritoHash(value [32]byte) []byte {
	return nativeNoritoField(value[:])
}

func initializeExecutionCommitment(execution *ExecutionCommitmentWitness) {
	zeroU8s(execution.ParentStateRoot[:])
	zeroU8s(execution.PostStateRoot[:])
	zeroU8s(execution.OrdinaryWritesRoot[:])
	execution.HasTopupAnchorRoot = 0
	zeroU8s(execution.TopupAnchorRoot[:])
	execution.TopupAnchorCount = 0
	execution.NativeAMXApplicationManifestVer = 0
	zeroU8s(execution.NativeAMXApplicationManifestRoot[:])
	execution.NativeAMXApplicationManifestCount = 0
	execution.HasLaneFinalityManifest = 0
	zeroU8s(execution.LaneFinalityManifestRoot[:])
	execution.LaneFinalityManifestLeafCount = 0
	execution.HasMergeCarrier = 0
	execution.MergeCarrierVersion = 0
	zeroU8s(execution.MergeCarrierEntryHash[:])
	execution.ExecutedBlockWireLength = 0
	zeroU8s(execution.ExecutedBlockWireHash[:])
}

func copyExecutionCommitment(destination *ExecutionCommitmentWitness, source ExecutionCommitmentWitness) {
	*destination = source
}

func assertExecutionCommitmentsEqual(
	api frontend.API,
	left *ExecutionCommitmentWitness,
	right *ExecutionCommitmentWitness,
) error {
	for _, pair := range [][2][]uints.U8{
		{left.ParentStateRoot[:], right.ParentStateRoot[:]},
		{left.PostStateRoot[:], right.PostStateRoot[:]},
		{left.OrdinaryWritesRoot[:], right.OrdinaryWritesRoot[:]},
		{left.TopupAnchorRoot[:], right.TopupAnchorRoot[:]},
		{left.NativeAMXApplicationManifestRoot[:], right.NativeAMXApplicationManifestRoot[:]},
		{left.LaneFinalityManifestRoot[:], right.LaneFinalityManifestRoot[:]},
		{left.MergeCarrierEntryHash[:], right.MergeCarrierEntryHash[:]},
		{left.ExecutedBlockWireHash[:], right.ExecutedBlockWireHash[:]},
	} {
		if err := assertBytesEqual(api, pair[0], pair[1]); err != nil {
			return err
		}
	}
	for _, pair := range [][2]frontend.Variable{
		{left.HasTopupAnchorRoot, right.HasTopupAnchorRoot},
		{left.TopupAnchorCount, right.TopupAnchorCount},
		{left.NativeAMXApplicationManifestVer, right.NativeAMXApplicationManifestVer},
		{left.NativeAMXApplicationManifestCount, right.NativeAMXApplicationManifestCount},
		{left.HasLaneFinalityManifest, right.HasLaneFinalityManifest},
		{left.LaneFinalityManifestLeafCount, right.LaneFinalityManifestLeafCount},
		{left.HasMergeCarrier, right.HasMergeCarrier},
		{left.MergeCarrierVersion, right.MergeCarrierVersion},
		{left.ExecutedBlockWireLength, right.ExecutedBlockWireLength},
	} {
		api.AssertIsEqual(pair[0], pair[1])
	}
	return nil
}
