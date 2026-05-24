// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.7.4;

import "./ISccpMessageVerifier.sol";

/**
 * @title SccpGroth16Bn254MessageVerifier
 * @dev Production-style SCCP verifier for EVM lanes backed by the BN254
 * pairing precompiles.
 *
 * The immutable verifying key is supplied at deployment and cannot be updated.
 * `proofBytes` must ABI-decode as:
 *
 * (uint256 version, bytes32 message_id, uint256 source_domain,
 *  bytes32 commitment_root, uint256[2] a, uint256[4] b, uint256[2] c)
 *
 * The Groth16 circuit is expected to expose nine public signals derived as
 * field hashes of the SCCP statement words:
 *
 * 0 message_id
 * 1 payload_hash
 * 2 target_domain_word
 * 3 commitment_root
 * 4 finality_height_word
 * 5 finality_block_hash
 * 6 source_domain
 * 7 statement_hash
 * 8 destination_binding_hash
 */
contract SccpGroth16Bn254MessageVerifier is ISccpMessageVerifier {
    uint256 private constant PROOF_VERSION = 1;
    uint256 private constant PUBLIC_INPUT_COUNT = 9;
    uint256 private constant BASE_FIELD_MODULUS =
        21888242871839275222246405745257275088696311157297823662689037894645226208583;
    uint256 private constant SCALAR_FIELD_MODULUS =
        21888242871839275222246405745257275088548364400416034343698204186575808495617;

    bytes32 private constant SIGNAL_MESSAGE_ID =
        keccak256("sccp:groth16-bn254:signal:message-id:v1");
    bytes32 private constant SIGNAL_PAYLOAD_HASH =
        keccak256("sccp:groth16-bn254:signal:payload-hash:v1");
    bytes32 private constant SIGNAL_TARGET_DOMAIN =
        keccak256("sccp:groth16-bn254:signal:target-domain:v1");
    bytes32 private constant SIGNAL_COMMITMENT_ROOT =
        keccak256("sccp:groth16-bn254:signal:commitment-root:v1");
    bytes32 private constant SIGNAL_FINALITY_HEIGHT =
        keccak256("sccp:groth16-bn254:signal:finality-height:v1");
    bytes32 private constant SIGNAL_FINALITY_BLOCK_HASH =
        keccak256("sccp:groth16-bn254:signal:finality-block-hash:v1");
    bytes32 private constant SIGNAL_SOURCE_DOMAIN =
        keccak256("sccp:groth16-bn254:signal:source-domain:v1");
    bytes32 private constant SIGNAL_STATEMENT_HASH =
        keccak256("sccp:groth16-bn254:signal:statement-hash:v1");
    bytes32 private constant SIGNAL_DESTINATION_BINDING_HASH =
        keccak256("sccp:groth16-bn254:signal:destination-binding-hash:v1");

    struct G1Point {
        uint256 x;
        uint256 y;
    }

    struct G2Point {
        uint256[2] x;
        uint256[2] y;
    }

    struct Proof {
        G1Point a;
        G2Point b;
        G1Point c;
    }

    G1Point private alpha1;
    G2Point private beta2;
    G2Point private gamma2;
    G2Point private delta2;
    G1Point[] private ic;

    event VerifyingKeyConfigured(bytes32 indexed verifyingKeyHash, uint256 publicInputCount);

    constructor(
        uint256[2] memory configuredAlpha1,
        uint256[4] memory configuredBeta2,
        uint256[4] memory configuredGamma2,
        uint256[4] memory configuredDelta2,
        uint256[] memory configuredIc
    ) {
        require(
            configuredIc.length == (PUBLIC_INPUT_COUNT + 1) * 2,
            "Unexpected verifying key input count"
        );

        alpha1 = _g1(configuredAlpha1);
        beta2 = _g2(configuredBeta2);
        gamma2 = _g2(configuredGamma2);
        delta2 = _g2(configuredDelta2);
        _requireValidG1(alpha1);
        _requireValidG2(beta2);
        _requireValidG2(gamma2);
        _requireValidG2(delta2);

        for (uint256 i = 0; i < configuredIc.length; i += 2) {
            uint256[2] memory rawPoint = [configuredIc[i], configuredIc[i + 1]];
            G1Point memory point = _g1(rawPoint);
            _requireValidG1(point);
            ic.push(point);
        }

        emit VerifyingKeyConfigured(verifyingKeyHash(), PUBLIC_INPUT_COUNT);
    }

    function publicInputCount() external pure returns (uint256) {
        return PUBLIC_INPUT_COUNT;
    }

    function verifyingKeyHash() public view returns (bytes32) {
        bytes memory encoded = abi.encode(
            alpha1.x,
            alpha1.y,
            beta2.x,
            beta2.y,
            gamma2.x,
            gamma2.y,
            delta2.x,
            delta2.y
        );
        for (uint256 i = 0; i < ic.length; i++) {
            encoded = abi.encodePacked(encoded, ic[i].x, ic[i].y);
        }
        return keccak256(encoded);
    }

    function verifySccpMessageProof(
        bytes calldata proofBytes,
        bytes32[6] calldata publicInputs,
        bytes32 statementHash,
        bytes32 destinationBindingHash
    )
        external
        view
        override
        returns (
            bytes32 messageId,
            uint32 sourceDomain,
            bytes32 commitmentRoot
        )
    {
        uint256 rawVersion;
        uint256 rawSourceDomain;
        Proof memory proof;
        (rawVersion, messageId, rawSourceDomain, commitmentRoot, proof) =
            _decodeProof(proofBytes);

        require(rawVersion == PROOF_VERSION, "Unsupported Groth16 proof version");
        require(messageId != bytes32(0), "Message id is required");
        require(rawSourceDomain <= type(uint32).max, "Source domain overflow");
        require(publicInputs[0] == messageId, "Public input message id mismatch");
        require(publicInputs[3] == commitmentRoot, "Public input commitment root mismatch");
        _requireValidG1(proof.a);
        _requireValidG2(proof.b);
        _requireValidG1(proof.c);

        uint256[9] memory signals = _publicSignals(
            publicInputs,
            rawSourceDomain,
            statementHash,
            destinationBindingHash
        );
        require(_verifyProof(signals, proof), "Groth16 proof verification failed");

        sourceDomain = uint32(rawSourceDomain);
    }

    function _decodeProof(bytes calldata proofBytes)
        private
        pure
        returns (
            uint256 rawVersion,
            bytes32 messageId,
            uint256 rawSourceDomain,
            bytes32 commitmentRoot,
            Proof memory proof
        )
    {
        uint256[2] memory rawA;
        uint256[4] memory rawB;
        uint256[2] memory rawC;
        (
            rawVersion,
            messageId,
            rawSourceDomain,
            commitmentRoot,
            rawA,
            rawB,
            rawC
        ) = abi.decode(
            proofBytes,
            (uint256, bytes32, uint256, bytes32, uint256[2], uint256[4], uint256[2])
        );
        proof = Proof(_g1(rawA), _g2(rawB), _g1(rawC));
    }

    function _publicSignals(
        bytes32[6] calldata publicInputs,
        uint256 sourceDomain,
        bytes32 statementHash,
        bytes32 destinationBindingHash
    ) private pure returns (uint256[9] memory signals) {
        signals[0] = _signal(SIGNAL_MESSAGE_ID, publicInputs[0]);
        signals[1] = _signal(SIGNAL_PAYLOAD_HASH, publicInputs[1]);
        signals[2] = _signal(SIGNAL_TARGET_DOMAIN, publicInputs[2]);
        signals[3] = _signal(SIGNAL_COMMITMENT_ROOT, publicInputs[3]);
        signals[4] = _signal(SIGNAL_FINALITY_HEIGHT, publicInputs[4]);
        signals[5] = _signal(SIGNAL_FINALITY_BLOCK_HASH, publicInputs[5]);
        signals[6] = _signal(SIGNAL_SOURCE_DOMAIN, bytes32(sourceDomain));
        signals[7] = _signal(SIGNAL_STATEMENT_HASH, statementHash);
        signals[8] = _signal(SIGNAL_DESTINATION_BINDING_HASH, destinationBindingHash);
    }

    function _verifyProof(uint256[9] memory signals, Proof memory proof)
        private
        view
        returns (bool)
    {
        G1Point memory vkX = ic[0];
        for (uint256 i = 0; i < PUBLIC_INPUT_COUNT; i++) {
            vkX = _add(vkX, _scalarMul(ic[i + 1], signals[i]));
        }

        return _pairing(
            _negate(proof.a),
            proof.b,
            alpha1,
            beta2,
            vkX,
            gamma2,
            proof.c,
            delta2
        );
    }

    function _signal(bytes32 label, bytes32 value) private pure returns (uint256) {
        return uint256(keccak256(abi.encode(label, value))) % SCALAR_FIELD_MODULUS;
    }

    function _g1(uint256[2] memory point) private pure returns (G1Point memory) {
        return G1Point(point[0], point[1]);
    }

    function _g2(uint256[4] memory point) private pure returns (G2Point memory) {
        uint256[2] memory x = [point[0], point[1]];
        uint256[2] memory y = [point[2], point[3]];
        return G2Point(x, y);
    }

    function _requireNonZeroG1(G1Point memory point) private pure {
        require(point.x < BASE_FIELD_MODULUS && point.y < BASE_FIELD_MODULUS, "G1 point out of range");
        require(point.x != 0 || point.y != 0, "G1 point is zero");
    }

    function _requireNonZeroG2(G2Point memory point) private pure {
        require(
            point.x[0] < BASE_FIELD_MODULUS &&
                point.x[1] < BASE_FIELD_MODULUS &&
                point.y[0] < BASE_FIELD_MODULUS &&
                point.y[1] < BASE_FIELD_MODULUS,
            "G2 point out of range"
        );
        require(
            point.x[0] != 0 || point.x[1] != 0 || point.y[0] != 0 || point.y[1] != 0,
            "G2 point is zero"
        );
    }

    function _requireValidG1(G1Point memory point) private view {
        _requireNonZeroG1(point);
        G1Point memory checked = _scalarMul(point, 1);
        require(checked.x == point.x && checked.y == point.y, "G1 point is invalid");
    }

    function _requireValidG2(G2Point memory point) private view {
        _requireNonZeroG2(point);
        G1Point memory generator = G1Point(1, 2);
        require(
            _pairing2(generator, point, _negate(generator), point),
            "G2 point is invalid"
        );
    }

    function _negate(G1Point memory point) private pure returns (G1Point memory) {
        if (point.x == 0 && point.y == 0) {
            return G1Point(0, 0);
        }
        return G1Point(point.x, BASE_FIELD_MODULUS - (point.y % BASE_FIELD_MODULUS));
    }

    function _add(G1Point memory left, G1Point memory right)
        private
        view
        returns (G1Point memory result)
    {
        uint256[4] memory input = [left.x, left.y, right.x, right.y];
        bool success;
        assembly {
            success := staticcall(gas(), 6, input, 0x80, result, 0x40)
        }
        require(success, "G1 addition failed");
    }

    function _scalarMul(G1Point memory point, uint256 scalar)
        private
        view
        returns (G1Point memory result)
    {
        uint256[3] memory input = [point.x, point.y, scalar];
        bool success;
        assembly {
            success := staticcall(gas(), 7, input, 0x60, result, 0x40)
        }
        require(success, "G1 scalar multiplication failed");
    }

    function _pairing(
        G1Point memory a1,
        G2Point memory a2,
        G1Point memory b1,
        G2Point memory b2,
        G1Point memory c1,
        G2Point memory c2,
        G1Point memory d1,
        G2Point memory d2
    ) private view returns (bool) {
        uint256[24] memory input = [
            a1.x,
            a1.y,
            a2.x[1],
            a2.x[0],
            a2.y[1],
            a2.y[0],
            b1.x,
            b1.y,
            b2.x[1],
            b2.x[0],
            b2.y[1],
            b2.y[0],
            c1.x,
            c1.y,
            c2.x[1],
            c2.x[0],
            c2.y[1],
            c2.y[0],
            d1.x,
            d1.y,
            d2.x[1],
            d2.x[0],
            d2.y[1],
            d2.y[0]
        ];
        uint256[1] memory out;
        bool success;
        assembly {
            success := staticcall(gas(), 8, input, 0x300, out, 0x20)
        }
        require(success, "Pairing precompile failed");
        return out[0] != 0;
    }

    function _pairing2(
        G1Point memory a1,
        G2Point memory a2,
        G1Point memory b1,
        G2Point memory b2
    ) private view returns (bool) {
        uint256[12] memory input = [
            a1.x,
            a1.y,
            a2.x[1],
            a2.x[0],
            a2.y[1],
            a2.y[0],
            b1.x,
            b1.y,
            b2.x[1],
            b2.x[0],
            b2.y[1],
            b2.y[0]
        ];
        uint256[1] memory out;
        bool success;
        assembly {
            success := staticcall(gas(), 8, input, 0x180, out, 0x20)
        }
        require(success, "Pairing precompile failed");
        return out[0] != 0;
    }
}
