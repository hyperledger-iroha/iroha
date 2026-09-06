package circuit

import (
	"bytes"
	"crypto/sha256"
	"fmt"
	"math/big"
	"sort"

	bls12381 "github.com/consensys/gnark-crypto/ecc/bls12-381"
	fr_bls12381 "github.com/consensys/gnark-crypto/ecc/bls12-381/fr"
)

var (
	w3fBLSNormalPrefix = []byte(
		"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_NUL_for signing messages",
	)
	w3fBLSDST         = []byte{1}
	irohaBLSPoPDomain = []byte("iroha:bls:pop:v1")
)

type nativeBLSKeyMaterial struct {
	secret         *big.Int
	publicKey      bls12381.G1Affine
	publicKeyBytes [bls12381.SizeOfG1AffineCompressed]byte
	proof          bls12381.G2Affine
	proofBytes     [bls12381.SizeOfG2AffineCompressed]byte
}

func deterministicBLSRoster(scope string, count int) []nativeBLSKeyMaterial {
	roster := make([]nativeBLSKeyMaterial, count)
	for index := range roster {
		roster[index] = deterministicBLSKeyMaterial(fmt.Sprintf("%s:%d", scope, index))
	}
	sort.Slice(roster, func(left, right int) bool {
		return bytes.Compare(roster[left].publicKeyBytes[:], roster[right].publicKeyBytes[:]) < 0
	})
	return roster
}

func deterministicBLSKeyMaterial(scope string) nativeBLSKeyMaterial {
	seed := sha256.Sum256([]byte("sccp:final-v1:bls-normal:key:" + scope))
	var scalar fr_bls12381.Element
	scalar.SetBytes(seed[:])
	if scalar.IsZero() {
		scalar.SetOne()
	}
	secret := scalar.BigInt(new(big.Int))
	var publicKey bls12381.G1Affine
	publicKey.ScalarMultiplicationBase(secret)
	publicKeyBytes := publicKey.Bytes()
	popHashInput := append(append([]byte{}, irohaBLSPoPDomain...), publicKeyBytes[:]...)
	popMessageHash := nativeIrohaHash(popHashInput)
	proof := nativeBLSSignPoint(secret, popMessageHash[:])
	return nativeBLSKeyMaterial{
		secret:         secret,
		publicKey:      publicKey,
		publicKeyBytes: publicKeyBytes,
		proof:          proof,
		proofBytes:     proof.Bytes(),
	}
}

func nativeBLSSignPoint(secret *big.Int, message []byte) bls12381.G2Affine {
	hashPoint, err := bls12381.HashToG2(nativeW3FBLSMessage(message), w3fBLSDST)
	if err != nil {
		panic(fmt.Sprintf("hash deterministic SCCP BLS message to G2: %v", err))
	}
	var signature bls12381.G2Affine
	signature.ScalarMultiplication(&hashPoint, secret)
	return signature
}

func nativeBLSAggregateSignature(
	keys []nativeBLSKeyMaterial,
	signers []int,
	message []byte,
) bls12381.G2Affine {
	var aggregate bls12381.G2Affine
	aggregate.SetInfinity()
	for _, index := range signers {
		signature := nativeBLSSignPoint(keys[index].secret, message)
		aggregate.Add(&aggregate, &signature)
	}
	return aggregate
}

func nativeW3FBLSMessage(message []byte) []byte {
	preimage := make([]byte, 0, len(w3fBLSNormalPrefix)+len(message))
	preimage = append(preimage, w3fBLSNormalPrefix...)
	return append(preimage, message...)
}
