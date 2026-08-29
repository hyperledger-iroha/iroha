// SPDX-License-Identifier: Apache-2.0
pragma solidity 0.7.4;
pragma experimental ABIEncoderV2;

/**
 * @title SccpSha256ReplayForest
 * @dev Stateless final-V1 verifier shared by the Ethereum, BSC, and TRON routes.
 * Route contracts retain only shard roots and checked counters. This runtime is
 * deployed and code-hash-bound by each route; it has no owner or mutable state.
 */
contract SccpSha256ReplayForest {
    struct SccpReplayWitness {
        bytes32 expectedShardRoot;
        bytes32 priorRecordDigest;
        bytes32 siblingBitmap;
        bytes32[] siblings;
    }

    struct SccpAddressReplayRecord {
        uint8 operation;
        bytes32 replayId;
        bytes32 payloadSha256;
        uint128 amountScale9;
        uint8 principalKind;
        address principal;
        bytes32 auxiliaryIdentitySha256;
    }

    uint16 public constant REPLAY_DEPTH = 248;
    bytes private constant REPLAY_MAGIC = "SCCP-REPLAY-SMT-V1";

    /** Hash one exact production replay domain. */
    function domainHash(
        uint32 sourceNetwork,
        uint32 targetNetwork,
        uint8 boundary,
        uint32 routeRevision,
        bytes32 routeConfigurationHash,
        uint8 actorKind,
        bytes calldata actorBytes
    ) external pure returns (bytes32) {
        require(routeRevision != 0, "SR01");
        require(routeConfigurationHash != bytes32(0), "SR02");
        require(
            _validDomain(sourceNetwork, targetNetwork, boundary, actorKind, actorBytes),
            "SR03"
        );
        return sha256(abi.encodePacked(
            REPLAY_MAGIC,
            bytes1(0x00),
            sourceNetwork,
            targetNetwork,
            boundary,
            routeRevision,
            routeConfigurationHash,
            actorKind,
            uint16(actorBytes.length),
            actorBytes
        ));
    }

    /** Derive the complete replay key; byte zero selects its shard. */
    function replayKey(bytes32 replayDomainHash, bytes32 replayId)
        external
        pure
        returns (bytes32)
    {
        return _replayKey(replayDomainHash, replayId);
    }

    /**
     * Hash an occupied EVM or TRON record. The principal is the exact 20-byte
     * account payload; operation-specific auxiliary bytes are committed by the
     * caller as their canonical SHA-256.
     */
    function addressRecordDigest(
        uint8 operation,
        bytes32 replayId,
        bytes32 payloadSha256,
        uint128 amountScale9,
        uint8 principalKind,
        address principal,
        bytes32 auxiliaryIdentitySha256
    ) external pure returns (bytes32) {
        SccpAddressReplayRecord memory record = SccpAddressReplayRecord({
            operation: operation,
            replayId: replayId,
            payloadSha256: payloadSha256,
            amountScale9: amountScale9,
            principalKind: principalKind,
            principal: principal,
            auxiliaryIdentitySha256: auxiliaryIdentitySha256
        });
        return _addressRecordDigest(record);
    }

    /** Hash a record and prepare exactly one empty-to-occupied transition. */
    function prepareAddressOccupation(
        bytes32 replayDomainHash,
        SccpAddressReplayRecord calldata record,
        bytes calldata encodedWitness
    )
        external
        pure
        returns (
            uint8 shard,
            bytes32 key,
            bytes32 recordDigest,
            bytes32 oldShardRoot,
            bytes32 newShardRoot
        )
    {
        recordDigest = _addressRecordDigest(record);
        SccpReplayWitness memory witness = _decodeWitness(encodedWitness);
        oldShardRoot = witness.expectedShardRoot;
        (shard, key, newShardRoot) = _prepareOccupation(
            replayDomainHash,
            record.replayId,
            recordDigest,
            oldShardRoot,
            witness
        );
    }

    function _addressRecordDigest(SccpAddressReplayRecord memory record)
        private
        pure
        returns (bytes32)
    {
        require(_validOperation(record.operation), "SR04");
        require(record.principalKind == 1 || record.principalKind == 2,
            "SR05");
        require(
            record.replayId != bytes32(0)
                && record.payloadSha256 != bytes32(0)
                && record.amountScale9 != 0
                && record.auxiliaryIdentitySha256 != bytes32(0),
            "SR06"
        );
        bytes memory principalBytes = abi.encodePacked(record.principal);
        bytes32 principalDigest = sha256(abi.encodePacked(
            REPLAY_MAGIC,
            bytes1(0x03),
            record.principalKind,
            uint16(principalBytes.length),
            principalBytes
        ));
        bytes32 auxiliaryDigest = sha256(abi.encodePacked(
            REPLAY_MAGIC,
            bytes1(0x04),
            record.operation,
            record.auxiliaryIdentitySha256
        ));
        return sha256(abi.encodePacked(
            REPLAY_MAGIC,
            bytes1(0x02),
            record.operation,
            record.replayId,
            record.payloadSha256,
            record.amountScale9,
            principalDigest,
            auxiliaryDigest
        ));
    }

    /** Return the canonical root of a completely empty shard. */
    function emptyShardRoot() external pure returns (bytes32) {
        bytes32 empty = _emptyLeaf();
        for (uint16 level = 0; level < REPLAY_DEPTH; level++) {
            empty = _parent(level, empty, empty);
        }
        return empty;
    }

    /**
     * Verify canonical non-membership and compute the occupied replacement root.
     * The caller commits the returned root and increments both checked counters
     * only after its economic mutation succeeds.
     */
    function prepareOccupation(
        bytes32 replayDomainHash,
        bytes32 replayId,
        bytes32 recordDigest,
        bytes32 currentShardRoot,
        bytes calldata encodedWitness
    ) external pure returns (uint8 shard, bytes32 key, bytes32 newShardRoot) {
        SccpReplayWitness memory witness = _decodeWitness(encodedWitness);
        return _prepareOccupation(
            replayDomainHash,
            replayId,
            recordDigest,
            currentShardRoot,
            witness
        );
    }

    function _prepareOccupation(
        bytes32 replayDomainHash,
        bytes32 replayId,
        bytes32 recordDigest,
        bytes32 currentShardRoot,
        SccpReplayWitness memory witness
    ) private pure returns (uint8 shard, bytes32 key, bytes32 newShardRoot) {
        require(
            replayDomainHash != bytes32(0)
                && replayId != bytes32(0)
                && recordDigest != bytes32(0)
                && currentShardRoot != bytes32(0),
            "SR07"
        );
        require(witness.expectedShardRoot == currentShardRoot, "SR08");
        require(witness.priorRecordDigest == bytes32(0), "SR09");
        key = _replayKey(replayDomainHash, replayId);
        shard = uint8(bytes1(key));
        bytes32 oldShardRoot;
        (oldShardRoot, newShardRoot) = _foldOccupation(key, recordDigest, witness);
        require(oldShardRoot == currentShardRoot, "SR10");
        require(newShardRoot != bytes32(0) && newShardRoot != oldShardRoot,
            "SR11");
    }

    /** Strictly verify membership in one current shard root. */
    function verifyMembership(
        bytes32 key,
        bytes32 recordDigest,
        bytes32 currentShardRoot,
        bytes calldata encodedWitness
    ) external pure returns (bool) {
        SccpReplayWitness memory witness = _decodeWitness(encodedWitness);
        require(key != bytes32(0) && recordDigest != bytes32(0),
            "SR12");
        require(
            witness.expectedShardRoot == currentShardRoot
                && witness.priorRecordDigest == recordDigest,
            "SR13"
        );
        return _foldSingle(key, _occupiedLeaf(key, recordDigest), witness) == currentShardRoot;
    }

    /** Strictly verify non-membership in one current shard root. */
    function verifyNonMembership(
        bytes32 key,
        bytes32 currentShardRoot,
        bytes calldata encodedWitness
    ) external pure returns (bool) {
        SccpReplayWitness memory witness = _decodeWitness(encodedWitness);
        require(key != bytes32(0), "SR14");
        require(
            witness.expectedShardRoot == currentShardRoot
                && witness.priorRecordDigest == bytes32(0),
            "SR15"
        );
        return _foldSingle(key, _emptyLeaf(), witness) == currentShardRoot;
    }

    function _foldOccupation(
        bytes32 key,
        bytes32 recordDigest,
        SccpReplayWitness memory witness
    ) private pure returns (bytes32 oldRoot, bytes32 newRoot) {
        _validateWitnessShape(witness);
        bytes32 empty = _emptyLeaf();
        oldRoot = empty;
        newRoot = _occupiedLeaf(key, recordDigest);
        uint256 supplied = 0;
        uint256 bitmap = uint256(witness.siblingBitmap);
        uint256 keyBits = uint256(key);
        for (uint16 level = 0; level < REPLAY_DEPTH; level++) {
            bytes32 sibling = empty;
            if ((bitmap & (uint256(1) << level)) != 0) {
                sibling = witness.siblings[supplied++];
                require(sibling != empty, "SR16");
            }
            if ((keyBits & (uint256(1) << level)) != 0) {
                oldRoot = _parent(level, sibling, oldRoot);
                newRoot = _parent(level, sibling, newRoot);
            } else {
                oldRoot = _parent(level, oldRoot, sibling);
                newRoot = _parent(level, newRoot, sibling);
            }
            empty = _parent(level, empty, empty);
        }
        require(supplied == witness.siblings.length, "SR17");
    }

    function _foldSingle(
        bytes32 key,
        bytes32 leaf,
        SccpReplayWitness memory witness
    ) private pure returns (bytes32 root) {
        _validateWitnessShape(witness);
        bytes32 empty = _emptyLeaf();
        root = leaf;
        uint256 supplied = 0;
        uint256 bitmap = uint256(witness.siblingBitmap);
        uint256 keyBits = uint256(key);
        for (uint16 level = 0; level < REPLAY_DEPTH; level++) {
            bytes32 sibling = empty;
            if ((bitmap & (uint256(1) << level)) != 0) {
                sibling = witness.siblings[supplied++];
                require(sibling != empty, "SR16");
            }
            root = (keyBits & (uint256(1) << level)) != 0
                ? _parent(level, sibling, root)
                : _parent(level, root, sibling);
            empty = _parent(level, empty, empty);
        }
        require(supplied == witness.siblings.length, "SR17");
    }

    function _validateWitnessShape(SccpReplayWitness memory witness) private pure {
        require(witness.expectedShardRoot != bytes32(0), "SR18");
        uint256 bitmap = uint256(witness.siblingBitmap);
        require(bitmap >> REPLAY_DEPTH == 0, "SR19");
        uint256 count = 0;
        uint256 value = bitmap;
        while (value != 0) {
            count += value & 1;
            value >>= 1;
        }
        require(count == witness.siblings.length && count <= REPLAY_DEPTH,
            "SR17");
    }

    function _decodeWitness(bytes calldata encodedWitness)
        private
        pure
        returns (SccpReplayWitness memory witness)
    {
        witness = abi.decode(encodedWitness, (SccpReplayWitness));
        bytes memory canonical = abi.encode(witness);
        require(
            canonical.length == encodedWitness.length
                && keccak256(canonical) == keccak256(encodedWitness),
            "SR20"
        );
    }

    function _validDomain(
        uint32 source,
        uint32 target,
        uint8 boundary,
        uint8 actorKind,
        bytes calldata actor
    ) private pure returns (bool) {
        if (!_isProduction(source) || !_isProduction(target)) return false;
        if (actorKind == 0) {
            return actor.length == 0
                && ((boundary == 0x01 && source == 0x40 && _isExternal(target))
                    || (boundary == 0x02 && _isExternal(source) && target == 0x40));
        }
        if (actorKind == 1) {
            return actor.length == 20
                && ((boundary == 0x10 && _isEvm(source) && target == 0x40)
                    || (boundary == 0x11 && source == 0x40 && _isEvm(target)));
        }
        if (actorKind == 2) {
            return actor.length == 20
                && ((boundary == 0x20 && source == 0x43 && target == 0x40)
                    || (boundary == 0x21 && source == 0x40 && target == 0x43));
        }
        if (actorKind == 3) {
            bool inbound = boundary == 0x30 || boundary == 0x32 || boundary == 0x34
                || boundary == 0x36 || boundary == 0x37;
            bool outbound = boundary == 0x31 || boundary == 0x33 || boundary == 0x35;
            return actor.length == 36
                && ((inbound && source == 0x40 && target == 0x44)
                    || (outbound && source == 0x44 && target == 0x40));
        }
        return false;
    }

    function _validOperation(uint8 operation) private pure returns (bool) {
        return operation == 0x01 || operation == 0x02
            || operation == 0x10 || operation == 0x11
            || operation == 0x20 || operation == 0x21
            || (operation >= 0x30 && operation <= 0x37);
    }

    function _isProduction(uint32 network) private pure returns (bool) {
        return network >= 0x40 && network <= 0x44;
    }

    function _isExternal(uint32 network) private pure returns (bool) {
        return network >= 0x41 && network <= 0x44;
    }

    function _isEvm(uint32 network) private pure returns (bool) {
        return network == 0x41 || network == 0x42;
    }

    function _replayKey(bytes32 replayDomainHash, bytes32 replayId)
        private
        pure
        returns (bytes32)
    {
        return sha256(abi.encodePacked(REPLAY_MAGIC, bytes1(0x01), replayDomainHash, replayId));
    }

    function _emptyLeaf() private pure returns (bytes32) {
        return sha256(abi.encodePacked(REPLAY_MAGIC, bytes1(0x10)));
    }

    function _occupiedLeaf(bytes32 key, bytes32 recordDigest)
        private
        pure
        returns (bytes32)
    {
        return sha256(abi.encodePacked(REPLAY_MAGIC, bytes1(0x11), key, recordDigest));
    }

    function _parent(uint16 level, bytes32 left, bytes32 right)
        private
        pure
        returns (bytes32)
    {
        return sha256(abi.encodePacked(REPLAY_MAGIC, bytes1(0x12), level, left, right));
    }
}
