package circuit

import (
	"fmt"
	"math/big"

	"github.com/consensys/gnark/frontend"
	"github.com/consensys/gnark/std/algebra/emulated/sw_bls12381"
	"github.com/consensys/gnark/std/algebra/emulated/sw_emulated"
	"github.com/consensys/gnark/std/conversion"
	"github.com/consensys/gnark/std/math/cmp"
	"github.com/consensys/gnark/std/math/emulated"
	"github.com/consensys/gnark/std/math/uints"

	"github.com/hyperledger-iroha/iroha/circuits/sccp/internal/profile"
)

var fixedDummyBLSMaterial = deterministicBLSKeyMaterial("inactive-slot")

// constrainBLSNormalFinality verifies the Iroha BLS-normal aggregate vote
// signature carried by every finality artifact. Only an epoch successor also
// supplies PoPs for a newly activated roster. Those PoP equations are
// randomized with a circuit-derived Fiat-Shamir scalar and checked in their
// own multi-pairing, separate from the aggregate-vote equation. The challenge
// commits the fixed profile/role, complete public SCCP statement, epoch,
// ordered roster, every compressed key and PoP, every signer bit, exact vote
// bytes, and aggregate signature. There is no witness-supplied randomness.
func constrainBLSNormalFinality(
	api frontend.API,
	finality *FinalityWitness,
	batchContext []uints.U8,
	newRosterPoPs *PoPBatchWitness,
	voteEncoding *canonicalVoteEncoding,
) error {
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return fmt.Errorf("initialize BLS byte API: %w", err)
	}
	baseField, err := emulated.NewField[sw_bls12381.BaseField](api)
	if err != nil {
		return fmt.Errorf("initialize BLS base field: %w", err)
	}
	scalarField, err := emulated.NewField[sw_bls12381.ScalarField](api)
	if err != nil {
		return fmt.Errorf("initialize BLS scalar field: %w", err)
	}
	g1, err := sw_bls12381.NewG1(api)
	if err != nil {
		return fmt.Errorf("initialize BLS G1: %w", err)
	}
	g2, err := sw_bls12381.NewG2(api)
	if err != nil {
		return fmt.Errorf("initialize BLS G2: %w", err)
	}
	curve, err := sw_emulated.New[sw_bls12381.BaseField, sw_bls12381.ScalarField](
		api,
		sw_emulated.GetBLS12381Params(),
	)
	if err != nil {
		return fmt.Errorf("initialize BLS G1 group arithmetic: %w", err)
	}
	pairing, err := sw_bls12381.NewPairing(api)
	if err != nil {
		return fmt.Errorf("initialize BLS pairing: %w", err)
	}

	voteTranscript := constants(sumeragiVoteSignatureDomain)
	voteTranscript = append(voteTranscript, finality.VoteSignaturePayload[:]...)
	var challenge *emulated.Element[sw_bls12381.ScalarField]
	if newRosterPoPs != nil {
		transcript := constants([]byte("sccp:final-v1:bls-normal:batch-challenge:v1"))
		transcript = append(transcript, batchContext...)
		countBytes, err := u32Bytes(api, finality.ValidatorCount)
		if err != nil {
			return err
		}
		transcript = append(transcript, countBytes...)
		epochBytes, err := u64Bytes(api, finality.Epoch)
		if err != nil {
			return err
		}
		epochEndBytes, err := u64Bytes(api, finality.EpochEndHeight)
		if err != nil {
			return err
		}
		transcript = append(transcript, epochBytes...)
		transcript = append(transcript, epochEndBytes...)
		for index := 0; index < MaxValidators; index++ {
			transcript = append(transcript, uints.U8{Val: finality.SignerBitmap[index]})
			transcript = append(transcript, finality.ValidatorPublicKeys[index][:]...)
			transcript = append(transcript, newRosterPoPs.ValidatorPoPs[index][:]...)
		}
		transcript = append(transcript, finality.AggregateSignature[:]...)
		voteLength, err := u32Bytes(api, finality.VoteSignaturePayloadLength)
		if err != nil {
			return err
		}
		transcript = append(transcript, voteLength...)
		transcript = append(transcript, voteTranscript...)
		challengeBytes, err := hashFixed(api, profile.SHA256Signal, transcript)
		if err != nil {
			return fmt.Errorf("derive BLS batch challenge: %w", err)
		}
		// Extract 254 uniformly distributed challenge bits. Every such value is
		// below the 255-bit BLS12-381 scalar modulus, avoiding a biased modular
		// reduction while retaining a 254-bit batch-soundness error bound.
		challengeBytes[0] = byteAPI.And(challengeBytes[0], uints.NewU8(0x3f))
		challenge, err = conversion.BytesToEmulated[sw_bls12381.ScalarField](api, challengeBytes)
		if err != nil {
			return fmt.Errorf("convert BLS batch challenge: %w", err)
		}
		api.AssertIsEqual(scalarField.IsZero(challenge), 0)
	}

	countComparator := cmp.NewBoundedComparator(api, big.NewInt(MaxValidators+1), false)
	dummyPublicKey := uints.NewU8Array(fixedDummyBLSMaterial.publicKeyBytes[:])
	zero := baseField.Zero()
	infinity := &sw_bls12381.G1Affine{X: *zero, Y: *zero}
	aggregatePublicKey := infinity
	popPairingCapacity := 0
	if newRosterPoPs != nil {
		popPairingCapacity = 2 * MaxValidators
	}
	popPairingG1 := make([]*sw_bls12381.G1Affine, 0, popPairingCapacity)
	popPairingG2 := make([]*sw_bls12381.G2Affine, 0, popPairingCapacity)
	power := scalarField.One()
	for index := 0; index < MaxValidators; index++ {
		active := countComparator.IsLess(index, finality.ValidatorCount)
		if err := canonicalActiveDigest(api, active, finality.ValidatorPublicKeys[index][:]); err != nil {
			return err
		}
		selectedPublicKey := selectBytes(
			api,
			active,
			finality.ValidatorPublicKeys[index][:],
			dummyPublicKey,
		)
		if err := assertCompressedFinitePrefix(api, byteAPI, selectedPublicKey[0]); err != nil {
			return fmt.Errorf("validator %d public key prefix: %w", index, err)
		}
		publicKey, err := g1.UnmarshalCompressed(selectedPublicKey)
		if err != nil {
			return fmt.Errorf("validator %d public key: %w", index, err)
		}
		if err := bindConditionalSHA256(
			api,
			active,
			finality.ValidatorPublicKeys[index][:],
			finality.ValidatorKeyHashes[index][:],
		); err != nil {
			return fmt.Errorf("validator %d public-key digest: %w", index, err)
		}
		if newRosterPoPs != nil {
			if err := canonicalActiveDigest(api, active, newRosterPoPs.ValidatorPoPs[index][:]); err != nil {
				return err
			}
			selectedProof := selectBytes(
				api,
				active,
				newRosterPoPs.ValidatorPoPs[index][:],
				uints.NewU8Array(fixedDummyBLSMaterial.proofBytes[:]),
			)
			if err := bindConditionalSHA256(
				api,
				active,
				newRosterPoPs.ValidatorPoPs[index][:],
				finality.ValidatorPoPHashes[index][:],
			); err != nil {
				return fmt.Errorf("validator %d PoP digest: %w", index, err)
			}
			proofPoint := &newRosterPoPs.ValidatorPoPPoints[index]
			if err := bindCompressedG2(api, byteAPI, baseField, g2, selectedProof, proofPoint); err != nil {
				return fmt.Errorf("validator %d PoP encoding: %w", index, err)
			}

			popHashInput := constants(irohaBLSPoPDomain)
			popHashInput = append(popHashInput, selectedPublicKey...)
			popMessageHash, err := irohaBlake2bHash(api, popHashInput)
			if err != nil {
				return fmt.Errorf("validator %d PoP message hash: %w", index, err)
			}
			popMessage := constants(w3fBLSNormalPrefix)
			popMessage = append(popMessage, popMessageHash[:]...)
			hashedPoPMessage, err := g2.HashToG2(popMessage, w3fBLSDST)
			if err != nil {
				return fmt.Errorf("validator %d PoP hash-to-G2: %w", index, err)
			}

			power = scalarField.Mul(power, challenge)
			weightedPublicKey := curve.ScalarMul(publicKey, power)
			weightedNegativeGenerator := curve.Neg(curve.ScalarMulBase(power))
			popPairingG1 = append(popPairingG1, weightedPublicKey, weightedNegativeGenerator)
			popPairingG2 = append(popPairingG2, hashedPoPMessage, proofPoint)
		}

		selectedSignerKey := curve.Select(finality.SignerBitmap[index], publicKey, infinity)
		aggregatePublicKey = curve.AddUnified(aggregatePublicKey, selectedSignerKey)
	}

	if err := bindSHA256(api, finality.AggregateSignature[:], finality.AggregateSignatureHash[:]); err != nil {
		return fmt.Errorf("aggregate-signature digest: %w", err)
	}
	if err := bindCompressedG2(
		api,
		byteAPI,
		baseField,
		g2,
		finality.AggregateSignature[:],
		&finality.AggregateSignaturePoint,
	); err != nil {
		return fmt.Errorf("aggregate-signature encoding: %w", err)
	}
	api.AssertIsEqual(
		api.And(
			baseField.IsZero(&aggregatePublicKey.X),
			baseField.IsZero(&aggregatePublicKey.Y),
		),
		0,
	)
	if voteEncoding == nil {
		return fmt.Errorf("canonical vote encoding is required")
	}
	var hashedVote *sw_bls12381.G2Affine
	for variant, payload := range voteEncoding.payloadVariants {
		voteMessage := constants(w3fBLSNormalPrefix)
		voteMessage = append(voteMessage, constants(sumeragiVoteSignatureDomain)...)
		voteMessage = append(voteMessage, payload...)
		candidate, err := g2.HashToG2(voteMessage, w3fBLSDST)
		if err != nil {
			return fmt.Errorf("vote variant %d hash-to-G2: %w", variant, err)
		}
		if hashedVote == nil {
			hashedVote = candidate
			continue
		}
		hashedVote = g2.Select(voteEncoding.selectors[variant], candidate, hashedVote)
	}
	negativeGenerator := curve.Neg(curve.ScalarMulBase(scalarField.One()))
	if err := pairing.PairingCheck(
		[]*sw_bls12381.G1Affine{aggregatePublicKey, negativeGenerator},
		[]*sw_bls12381.G2Affine{hashedVote, &finality.AggregateSignaturePoint},
	); err != nil {
		return fmt.Errorf("BLS-normal aggregate-vote pairing: %w", err)
	}
	if newRosterPoPs != nil {
		if err := pairing.PairingCheck(popPairingG1, popPairingG2); err != nil {
			return fmt.Errorf("BLS-normal new-roster PoP batch pairing: %w", err)
		}
	}
	return nil
}

func selectBytes(
	api frontend.API,
	selector frontend.Variable,
	whenTrue []uints.U8,
	whenFalse []uints.U8,
) []uints.U8 {
	if len(whenTrue) != len(whenFalse) {
		panic("selectBytes requires equal-width inputs")
	}
	result := make([]uints.U8, len(whenTrue))
	for index := range result {
		result[index] = uints.U8{
			Val: api.Select(selector, whenTrue[index].Val, whenFalse[index].Val),
		}
	}
	return result
}

func bindConditionalSHA256(
	api frontend.API,
	active frontend.Variable,
	input []uints.U8,
	expected []uints.U8,
) error {
	digest, err := hashFixed(api, profile.SHA256Signal, input)
	if err != nil {
		return err
	}
	byteAPI, err := uints.NewBytes(api)
	if err != nil {
		return err
	}
	for index := range digest {
		api.AssertIsEqual(
			byteAPI.Value(expected[index]),
			api.Mul(active, byteAPI.Value(digest[index])),
		)
	}
	return nil
}

func bindSHA256(api frontend.API, input, expected []uints.U8) error {
	digest, err := hashFixed(api, profile.SHA256Signal, input)
	if err != nil {
		return err
	}
	return assertBytesEqual(api, digest, expected)
}

func assertCompressedFinitePrefix(
	api frontend.API,
	byteAPI *uints.Bytes,
	first uints.U8,
) error {
	prefix := byteAPI.And(first, uints.NewU8(0xe0))
	isSmall := api.IsZero(api.Sub(byteAPI.Value(prefix), 0x80))
	isLarge := api.IsZero(api.Sub(byteAPI.Value(prefix), 0xa0))
	api.AssertIsEqual(api.Add(isSmall, isLarge), 1)
	return nil
}

func bindCompressedG2(
	api frontend.API,
	byteAPI *uints.Bytes,
	baseField *emulated.Field[sw_bls12381.BaseField],
	g2 *sw_bls12381.G2,
	compressed []uints.U8,
	point *sw_bls12381.G2Affine,
) error {
	if len(compressed) != 96 {
		return fmt.Errorf("compressed G2 point must be 96 bytes, got %d", len(compressed))
	}
	if err := assertCompressedFinitePrefix(api, byteAPI, compressed[0]); err != nil {
		return err
	}
	prefix := byteAPI.And(compressed[0], uints.NewU8(0xe0))
	isSmall := api.IsZero(api.Sub(byteAPI.Value(prefix), 0x80))
	xA1Bytes := append([]uints.U8(nil), compressed[:48]...)
	xA1Bytes[0] = byteAPI.And(xA1Bytes[0], uints.NewU8(0x1f))
	xA1, err := conversion.BytesToEmulated[sw_bls12381.BaseField](api, xA1Bytes)
	if err != nil {
		return fmt.Errorf("decode G2 x.A1: %w", err)
	}
	xA0, err := conversion.BytesToEmulated[sw_bls12381.BaseField](api, compressed[48:])
	if err != nil {
		return fmt.Errorf("decode G2 x.A0: %w", err)
	}
	baseField.AssertIsEqual(xA1, &point.P.X.A1)
	baseField.AssertIsEqual(xA0, &point.P.X.A0)
	g2.AssertIsOnG2(point)

	negativeY := g2.Ext2.Neg(&point.P.Y)
	smallY := g2.Ext2.Select(isSmall, &point.P.Y, negativeY)
	a1IsZero := baseField.IsZero(&smallY.A1)
	comparisonComponent := baseField.Select(a1IsZero, &smallY.A0, &smallY.A1)
	comparisonComponent = baseField.ReduceStrict(comparisonComponent)
	halfModulus := new(big.Int).Sub(sw_bls12381.BaseField{}.Modulus(), big.NewInt(1))
	halfModulus.Div(halfModulus, big.NewInt(2))
	baseField.AssertIsLessOrEqual(comparisonComponent, baseField.NewElement(halfModulus))
	return nil
}
