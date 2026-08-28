package circuit

import (
	"fmt"
	"math/big"
	"sort"

	"github.com/consensys/gnark/constraint/solver"
	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/math/cmp"
	"github.com/consensys/gnark/std/math/uints"
)

const (
	blsNormalAlgorithmTag      = 2
	maxDAChunkSizeBytes        = 256 * 1024
	maxDADataShards            = 16
	maxDAParityShards          = 16
	maxDAStripeWidth           = 32
	maxDAPayloadSizeBytes      = 16 * 1024 * 1024
	maxDAEncodedPayloadBytes   = 32 * 1024 * 1024
	maxDAChunkCount            = 1024
	canonicalBLSNormalPoPBytes = 96
)

var canonicalCommitteeSizes = [...]int{4, 7, 10, 13, 16, 19, 22, 25, 28, 31}

func init() {
	solver.RegisterHint(noritoIdentityHint)
}

func noritoIdentityHint(_ *big.Int, inputs, outputs []*big.Int) error {
	if len(inputs) != 1 || len(outputs) != 1 {
		return fmt.Errorf("Norito identity hint expects one input and one output")
	}
	outputs[0].Set(inputs[0])
	return nil
}

func materializeNoritoVariable(api frontend.API, value frontend.Variable) frontend.Variable {
	if _, constant := api.Compiler().ConstantValue(value); constant {
		return value
	}
	output, err := api.Compiler().NewHint(noritoIdentityHint, 1, value)
	if err != nil {
		panic(fmt.Sprintf("materialize Norito byte expression: %v", err))
	}
	api.AssertIsEqual(output[0], value)
	return output[0]
}

type selectedNoritoBytes struct {
	selector frontend.Variable
	bytes    []uints.U8
}

// noritoAssembler builds one variable-length byte string from circuit-derived
// one-hot choices.  Every non-selected path contributes zero, so the returned
// padded buffer is canonical and contains no caller-controlled padding.
type noritoAssembler struct {
	api     frontend.API
	byteAPI *uints.Bytes
	max     int
	values  []frontend.Variable
	states  map[int]frontend.Variable
}

func sortedNoritoOffsets(states map[int]frontend.Variable) []int {
	offsets := make([]int, 0, len(states))
	for offset := range states {
		offsets = append(offsets, offset)
	}
	sort.Ints(offsets)
	return offsets
}

func newNoritoAssembler(api frontend.API, maximum int) (*noritoAssembler, error) {
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return nil, err
	}
	return &noritoAssembler{
		api:     api,
		byteAPI: byteAPI,
		max:     maximum,
		values:  make([]frontend.Variable, maximum),
		states:  map[int]frontend.Variable{0: 1},
	}, nil
}

func (a *noritoAssembler) appendFixed(value []uints.U8) error {
	return a.appendChoices([]selectedNoritoBytes{{selector: 1, bytes: value}})
}

func (a *noritoAssembler) appendChoices(choices []selectedNoritoBytes) error {
	if len(choices) == 0 {
		return fmt.Errorf("Norito assembly choice set is empty")
	}
	next := make(map[int]frontend.Variable, len(a.states)*len(choices))
	for _, offset := range sortedNoritoOffsets(a.states) {
		stateSelector := a.states[offset]
		for _, choice := range choices {
			end := offset + len(choice.bytes)
			if end > a.max {
				return fmt.Errorf("Norito assembly length %d exceeds maximum %d", end, a.max)
			}
			pathSelector := a.api.Mul(stateSelector, choice.selector)
			for index := range choice.bytes {
				contribution := a.api.Mul(pathSelector, a.byteAPI.Value(choice.bytes[index]))
				if a.values[offset+index] == nil {
					a.values[offset+index] = contribution
				} else {
					a.values[offset+index] = a.api.Add(a.values[offset+index], contribution)
				}
			}
			if existing, ok := next[end]; ok {
				next[end] = a.api.Add(existing, pathSelector)
			} else {
				next[end] = pathSelector
			}
		}
	}
	for _, offset := range sortedNoritoOffsets(next) {
		selector := next[offset]
		next[offset] = materializeNoritoVariable(a.api, selector)
	}
	a.states = next
	return nil
}

func (a *noritoAssembler) finish() ([]uints.U8, frontend.Variable, error) {
	selectors := make([]frontend.Variable, 0, len(a.states))
	length := frontend.Variable(0)
	for _, offset := range sortedNoritoOffsets(a.states) {
		selector := a.states[offset]
		selectors = append(selectors, selector)
		length = a.api.Add(length, a.api.Mul(selector, offset))
	}
	a.api.AssertIsEqual(sumVariables(a.api, selectors), 1)
	encoded := make([]uints.U8, a.max)
	for index := range encoded {
		value := a.values[index]
		if value == nil {
			value = 0
		}
		value = materializeNoritoVariable(a.api, value)
		encoded[index] = a.byteAPI.ValueOf(value)
	}
	return encoded, length, nil
}

func committeeSizeSelectors(
	api frontend.API,
	count frontend.Variable,
) []frontend.Variable {
	selectors := make([]frontend.Variable, len(canonicalCommitteeSizes))
	for index, size := range canonicalCommitteeSizes {
		selectors[index] = api.IsZero(api.Sub(count, size))
	}
	api.AssertIsEqual(sumVariables(api, selectors), 1)
	return selectors
}

func constrainCanonicalCurrentRoster(api frontend.API, finality *FinalityWitness) error {
	countComparator := cmp.NewBoundedComparator(api, big.NewInt(MaxValidators+1), false)
	byteComparator := cmp.NewBoundedComparator(api, big.NewInt(256), false)
	for index := 0; index < MaxValidators; index++ {
		active := countComparator.IsLess(index, finality.ValidatorCount)
		if err := canonicalActiveDigest(api, active, finality.ValidatorPoPs[index][:]); err != nil {
			return err
		}
		if err := bindConditionalSHA256(
			api,
			active,
			finality.ValidatorPoPs[index][:],
			finality.ValidatorPoPHashes[index][:],
		); err != nil {
			return fmt.Errorf("validator %d durable PoP digest: %w", index, err)
		}
		if index == 0 {
			continue
		}
		// PeerId ordering is PublicKey ordering. All admitted keys use the same
		// BLS-normal algorithm tag, so lexicographic compressed-key order is the
		// exact order used by ValidatorPower/PeerId.
		less := frontend.Variable(0)
		equalPrefix := frontend.Variable(1)
		for byteIndex := 0; byteIndex < 48; byteIndex++ {
			left := finality.ValidatorPublicKeys[index-1][byteIndex].Val
			right := finality.ValidatorPublicKeys[index][byteIndex].Val
			byteLess := byteComparator.IsLess(left, right)
			less = api.Add(less, api.Mul(equalPrefix, byteLess))
			equalPrefix = api.Mul(equalPrefix, api.IsZero(api.Sub(left, right)))
		}
		api.AssertIsEqual(api.Mul(active, api.Sub(less, 1)), 0)
	}
	return nil
}

func constrainDataAvailabilityLayout(
	api frontend.API,
	layout *DataAvailabilityLayoutWitness,
) error {
	if _, err := u32Bytes(api, layout.ChunkSizeBytes); err != nil {
		return err
	}
	if _, err := u16Bytes(api, layout.DataShards); err != nil {
		return err
	}
	if _, err := u16Bytes(api, layout.ParityShards); err != nil {
		return err
	}
	if _, err := u64Bytes(api, layout.MaxPayloadSizeBytes); err != nil {
		return err
	}
	if _, err := u32Bytes(api, layout.MaxChunkCount); err != nil {
		return err
	}
	if _, err := u64Bytes(api, layout.RequiredStripes); err != nil {
		return err
	}
	if _, err := u64Bytes(api, layout.LastStripePayloadSize); err != nil {
		return err
	}

	comparison64 := cmp.NewBoundedComparator(api, new(big.Int).Lsh(big.NewInt(1), 64), false)
	comparison64.AssertIsLess(0, layout.ChunkSizeBytes)
	comparison64.AssertIsLessEq(layout.ChunkSizeBytes, maxDAChunkSizeBytes)
	// The protocol requires an even chunk size.
	chunkBytes, err := u32Bytes(api, layout.ChunkSizeBytes)
	if err != nil {
		return err
	}
	api.AssertIsEqual(api.ToBinary(chunkBytes[0].Val, 8)[0], 0)
	comparison64.AssertIsLess(0, layout.DataShards)
	comparison64.AssertIsLessEq(layout.DataShards, maxDADataShards)
	comparison64.AssertIsLess(0, layout.ParityShards)
	comparison64.AssertIsLessEq(layout.ParityShards, maxDAParityShards)
	comparison64.AssertIsLessEq(api.Add(layout.DataShards, layout.ParityShards), maxDAStripeWidth)
	comparison64.AssertIsLess(0, layout.MaxPayloadSizeBytes)
	comparison64.AssertIsLessEq(layout.MaxPayloadSizeBytes, maxDAPayloadSizeBytes)
	comparison64.AssertIsLess(0, layout.MaxChunkCount)
	comparison64.AssertIsLessEq(layout.MaxChunkCount, maxDAChunkCount)
	comparison64.AssertIsLess(0, layout.RequiredStripes)
	comparison64.AssertIsLess(0, layout.LastStripePayloadSize)

	stripePayload := api.Mul(layout.ChunkSizeBytes, layout.DataShards)
	comparison64.AssertIsLessEq(layout.LastStripePayloadSize, stripePayload)
	api.AssertIsEqual(
		layout.MaxPayloadSizeBytes,
		api.Add(
			api.Mul(api.Sub(layout.RequiredStripes, 1), stripePayload),
			layout.LastStripePayloadSize,
		),
	)
	requiredChunks := api.Mul(
		layout.RequiredStripes,
		api.Add(layout.DataShards, layout.ParityShards),
	)
	comparison64.AssertIsLessEq(requiredChunks, layout.MaxChunkCount)
	comparison64.AssertIsLessEq(
		api.Mul(requiredChunks, layout.ChunkSizeBytes),
		maxDAEncodedPayloadBytes,
	)
	return nil
}

func constrainNextEpochSnapshot(api frontend.API, finality *FinalityWitness) error {
	snapshot := &finality.NextEpochSnapshot
	active := finality.HasNextEpochSnapshot
	api.AssertIsBoolean(active)
	for _, value := range []frontend.Variable{
		snapshot.Epoch,
		snapshot.EpochEndHeight,
		snapshot.Mode,
		snapshot.ValidatorCount,
	} {
		api.AssertIsEqual(value, api.Mul(active, value))
	}
	if _, err := u64Bytes(api, snapshot.Epoch); err != nil {
		return err
	}
	if _, err := u64Bytes(api, snapshot.EpochEndHeight); err != nil {
		return err
	}
	if _, err := u32Bytes(api, snapshot.Mode); err != nil {
		return err
	}
	if _, err := u32Bytes(api, snapshot.ValidatorCount); err != nil {
		return err
	}
	api.AssertIsEqual(snapshot.Epoch, api.Mul(active, api.Add(finality.Epoch, 1)))
	api.AssertIsEqual(snapshot.Mode, api.Mul(active, finality.Mode))
	comparison64 := cmp.NewBoundedComparator(api, new(big.Int).Lsh(big.NewInt(1), 64), false)
	validEnd := comparison64.IsLessEq(api.Add(finality.Height, 1), snapshot.EpochEndHeight)
	api.AssertIsEqual(api.Mul(active, api.Sub(validEnd, 1)), 0)

	countSelectors := make([]frontend.Variable, len(canonicalCommitteeSizes))
	for index, size := range canonicalCommitteeSizes {
		countSelectors[index] = api.Mul(active, api.IsZero(api.Sub(snapshot.ValidatorCount, size)))
	}
	api.AssertIsEqual(
		api.Add(api.Sub(1, active), sumVariables(api, countSelectors)),
		1,
	)
	countComparator := cmp.NewBoundedComparator(api, big.NewInt(MaxValidators+1), false)
	for index := 0; index < MaxValidators; index++ {
		slotActive := countComparator.IsLess(index, snapshot.ValidatorCount)
		if err := canonicalActiveDigest(api, slotActive, snapshot.ValidatorPublicKeys[index][:]); err != nil {
			return err
		}
		if err := canonicalActiveDigest(api, slotActive, snapshot.ValidatorPoPs[index][:]); err != nil {
			return err
		}
	}
	if err := canonicalActiveDigest(api, active, snapshot.LeaderSeed[:]); err != nil {
		return err
	}
	return nil
}

func canonicalPublicKeyBody(publicKey []uints.U8) []uints.U8 {
	if len(publicKey) != 48 {
		panic("BLS-normal public key must contain 48 bytes")
	}
	// PublicKeyCompact stores ConstVec<u8>{algorithm || compressed key}. V1
	// uses the unpacked-sequence layout: a fixed u64 count and one compact
	// length-prefixed payload per byte.
	body := constants([]byte{49, 0, 0, 0, 0, 0, 0, 0})
	body = append(body, noritoField(constants([]byte{blsNormalAlgorithmTag}))...)
	for index := range publicKey {
		body = append(body, noritoField(publicKey[index:index+1])...)
	}
	return body
}

func canonicalValidatorPowerBody(api frontend.API, publicKey []uints.U8) ([]uints.U8, error) {
	peerBody := noritoField(canonicalPublicKeyBody(publicKey))
	body := noritoField(peerBody)
	power, err := noritoU64Field(api, 1)
	if err != nil {
		return nil, err
	}
	return append(body, power...), nil
}

func canonicalRosterVectorBody(
	api frontend.API,
	publicKeys *[MaxValidators][48]uints.U8,
	count int,
) ([]uints.U8, error) {
	body := constants([]byte{
		byte(count), 0, 0, 0, 0, 0, 0, 0,
	})
	for index := 0; index < count; index++ {
		validator, err := canonicalValidatorPowerBody(api, publicKeys[index][:])
		if err != nil {
			return nil, err
		}
		body = append(body, noritoField(validator)...)
	}
	return body, nil
}

func canonicalPoPVectorBody(
	proofs *[MaxValidators][canonicalBLSNormalPoPBytes]uints.U8,
	count int,
) []uints.U8 {
	body := constants([]byte{byte(count), 0, 0, 0, 0, 0, 0, 0})
	for index := 0; index < count; index++ {
		proof := constants([]byte{canonicalBLSNormalPoPBytes, 0, 0, 0, 0, 0, 0, 0})
		proof = append(proof, proofs[index][:]...)
		body = append(body, noritoField(proof)...)
	}
	return body
}

func canonicalDualQuorumBody(api frontend.API, count int) ([]uints.U8, error) {
	minimum := 2*((count-1)/3) + 1
	body, err := noritoU32Field(api, minimum)
	if err != nil {
		return nil, err
	}
	total, err := noritoU64Field(api, count)
	if err != nil {
		return nil, err
	}
	return append(body, total...), nil
}

func canonicalNextEpochSnapshotBody(
	api frontend.API,
	snapshot *NextEpochSnapshotWitness,
	count int,
) ([]uints.U8, error) {
	epoch, err := noritoU64Field(api, snapshot.Epoch)
	if err != nil {
		return nil, err
	}
	epochEnd, err := noritoU64Field(api, snapshot.EpochEndHeight)
	if err != nil {
		return nil, err
	}
	mode, err := noritoU32Field(api, snapshot.Mode)
	if err != nil {
		return nil, err
	}
	roster, err := canonicalRosterVectorBody(api, &snapshot.ValidatorPublicKeys, count)
	if err != nil {
		return nil, err
	}
	quorum, err := canonicalDualQuorumBody(api, count)
	if err != nil {
		return nil, err
	}
	body := append([]uints.U8(nil), epoch...)
	body = append(body, epochEnd...)
	body = append(body, mode...)
	body = append(body, noritoField(roster)...)
	body = append(body, noritoField(canonicalPoPVectorBody(&snapshot.ValidatorPoPs, count))...)
	body = append(body, noritoField(quorum)...)
	body = append(body, noritoField(snapshot.LeaderSeed[:])...)
	return body, nil
}

func canonicalParentCommitIdentityBody(
	api frontend.API,
	finality *FinalityWitness,
	executionVariant int,
) ([]uints.U8, error) {
	contextID := noritoField(noritoHashField(finality.ParentContextID[:]))
	height, err := noritoU64Field(api, finality.ParentHeight)
	if err != nil {
		return nil, err
	}
	phase, err := noritoU32Field(api, 2)
	if err != nil {
		return nil, err
	}
	parentOption := append(
		constants([]byte{1}),
		noritoHashField(finality.ParentSubjectParentBlockHash[:])...,
	)
	subject := noritoField(parentOption)
	subject = append(subject, noritoHashField(finality.ParentBlockHash[:])...)
	subject = append(subject, noritoHashField(finality.ParentPayloadHash[:])...)
	execution, err := canonicalExecutionCommitmentBody(api, &finality.ParentExecution, executionVariant)
	if err != nil {
		return nil, err
	}
	body := append([]uints.U8(nil), contextID...)
	body = append(body, height...)
	body = append(body, phase...)
	body = append(body, noritoField(subject)...)
	body = append(body, noritoField(execution)...)
	return body, nil
}

func canonicalDataAvailabilityLayoutBody(
	api frontend.API,
	layout *DataAvailabilityLayoutWitness,
) ([]uints.U8, error) {
	encoding, err := noritoU32Field(api, 0)
	if err != nil {
		return nil, err
	}
	chunk, err := noritoU32Field(api, layout.ChunkSizeBytes)
	if err != nil {
		return nil, err
	}
	data, err := noritoU16Field(api, layout.DataShards)
	if err != nil {
		return nil, err
	}
	parity, err := noritoU16Field(api, layout.ParityShards)
	if err != nil {
		return nil, err
	}
	maximumPayload, err := noritoU64Field(api, layout.MaxPayloadSizeBytes)
	if err != nil {
		return nil, err
	}
	maximumChunks, err := noritoU32Field(api, layout.MaxChunkCount)
	if err != nil {
		return nil, err
	}
	body := append([]uints.U8(nil), encoding...)
	body = append(body, chunk...)
	body = append(body, data...)
	body = append(body, parity...)
	body = append(body, maximumPayload...)
	body = append(body, maximumChunks...)
	return body, nil
}

// canonicalHeightContextIdentity constructs the exact bare-Norito bytes hashed
// by HeightContext::id() in account/block consensus_v2 revision 4.  This is not
// a semantic projection: every field and wrapper in HeightContextIdentity v5
// is present, including the full next snapshot and parent Commit decision.
func canonicalHeightContextIdentity(
	api frontend.API,
	finality *FinalityWitness,
) ([32]uints.U8, error) {
	assembler, err := newNoritoAssembler(api, MaxHeightContextIdentityBytes)
	if err != nil {
		return [32]uints.U8{}, err
	}
	identityVersion, err := noritoU16Field(api, heightContextIdentityVersion)
	if err != nil {
		return [32]uints.U8{}, err
	}
	protocol, err := noritoU16Field(api, sumeragiProtocolVersion)
	if err != nil {
		return [32]uints.U8{}, err
	}
	height, err := noritoU64Field(api, finality.Height)
	if err != nil {
		return [32]uints.U8{}, err
	}
	epoch, err := noritoU64Field(api, finality.Epoch)
	if err != nil {
		return [32]uints.U8{}, err
	}
	epochEnd, err := noritoU64Field(api, finality.EpochEndHeight)
	if err != nil {
		return [32]uints.U8{}, err
	}
	mode, err := noritoU32Field(api, finality.Mode)
	if err != nil {
		return [32]uints.U8{}, err
	}

	if err := assembler.appendFixed(identityVersion); err != nil {
		return [32]uints.U8{}, err
	}
	if err := assembler.appendFixed(noritoHashField(constants(tairaFinalityNetworkID[:]))); err != nil {
		return [32]uints.U8{}, err
	}
	for _, value := range [][]uints.U8{protocol, height, epoch, epochEnd} {
		if err := assembler.appendFixed(value); err != nil {
			return [32]uints.U8{}, err
		}
	}

	snapshotChoices := []selectedNoritoBytes{{
		selector: api.Sub(1, finality.HasNextEpochSnapshot),
		bytes:    noritoField(constants([]byte{0})),
	}}
	for _, count := range canonicalCommitteeSizes {
		body, err := canonicalNextEpochSnapshotBody(api, &finality.NextEpochSnapshot, count)
		if err != nil {
			return [32]uints.U8{}, err
		}
		option := append(constants([]byte{1}), noritoField(body)...)
		snapshotChoices = append(snapshotChoices, selectedNoritoBytes{
			selector: api.Mul(
				finality.HasNextEpochSnapshot,
				api.IsZero(api.Sub(finality.NextEpochSnapshot.ValidatorCount, count)),
			),
			bytes: noritoField(option),
		})
	}
	if err := assembler.appendChoices(snapshotChoices); err != nil {
		return [32]uints.U8{}, err
	}
	if err := assembler.appendFixed(mode); err != nil {
		return [32]uints.U8{}, err
	}

	parentChoices := make([]selectedNoritoBytes, 8)
	for variant := 0; variant < 8; variant++ {
		parentBody, err := canonicalParentCommitIdentityBody(api, finality, variant)
		if err != nil {
			return [32]uints.U8{}, err
		}
		option := append(constants([]byte{1}), noritoField(parentBody)...)
		parentChoices[variant] = selectedNoritoBytes{
			selector: optionVariantSelector(api, &finality.ParentExecution, variant),
			bytes:    noritoField(option),
		}
	}
	if err := assembler.appendChoices(parentChoices); err != nil {
		return [32]uints.U8{}, err
	}
	// SCCP finality never crosses an audited legacy snapshot bootstrap. Every
	// admitted proof has a native revision-4 parent CommitQC.
	if err := assembler.appendFixed(noritoField(constants([]byte{0}))); err != nil {
		return [32]uints.U8{}, err
	}

	countSelectors := committeeSizeSelectors(api, finality.ValidatorCount)
	rosterChoices := make([]selectedNoritoBytes, len(canonicalCommitteeSizes))
	for index, count := range canonicalCommitteeSizes {
		roster, err := canonicalRosterVectorBody(api, &finality.ValidatorPublicKeys, count)
		if err != nil {
			return [32]uints.U8{}, err
		}
		rosterChoices[index] = selectedNoritoBytes{
			selector: countSelectors[index],
			bytes:    noritoField(roster),
		}
	}
	if err := assembler.appendChoices(rosterChoices); err != nil {
		return [32]uints.U8{}, err
	}
	quorumChoices := make([]selectedNoritoBytes, len(canonicalCommitteeSizes))
	for index, count := range canonicalCommitteeSizes {
		quorum, err := canonicalDualQuorumBody(api, count)
		if err != nil {
			return [32]uints.U8{}, err
		}
		quorumChoices[index] = selectedNoritoBytes{
			selector: countSelectors[index],
			bytes:    noritoField(quorum),
		}
	}
	if err := assembler.appendChoices(quorumChoices); err != nil {
		return [32]uints.U8{}, err
	}
	for _, digest := range [][32]uints.U8{
		finality.NexusAMXContextHash,
		finality.ExecutionPolicyHash,
	} {
		if err := assembler.appendFixed(noritoHashField(digest[:])); err != nil {
			return [32]uints.U8{}, err
		}
	}
	daLayout, err := canonicalDataAvailabilityLayoutBody(api, &finality.DALayout)
	if err != nil {
		return [32]uints.U8{}, err
	}
	if err := assembler.appendFixed(noritoField(daLayout)); err != nil {
		return [32]uints.U8{}, err
	}
	if err := assembler.appendFixed(noritoField(finality.LeaderSeed[:])); err != nil {
		return [32]uints.U8{}, err
	}
	encoded, length, err := assembler.finish()
	if err != nil {
		return [32]uints.U8{}, err
	}
	digest, err := blake2b256(api, encoded, length)
	if err != nil {
		return [32]uints.U8{}, fmt.Errorf("hash exact HeightContextIdentity v5: %w", err)
	}
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return [32]uints.U8{}, err
	}
	digest[31] = byteAPI.Or(digest[31], uints.NewU8(1))
	return digest, nil
}
