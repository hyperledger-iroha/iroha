// Canonical variable-shape cells of §5.3.2: standard snake bytes, hash
// chunks and committee key chunks.
//
// Every parser here rejects any shape other than the single canonical one
// (`SccpErrors.BadSnake`), so one byte string or list has exactly one cell
// encoding and golden BoCs pin it. Every cell of a canonical shape is an
// ordinary cell: `CTOS` would resolve a library reference transparently, so a
// relayer could otherwise smuggle an exotic cell with canonical content into
// the stored committee keys (§5.3.2).

import "@stdlib/exotic-cells"
import "sccp-constants"
import "sccp-errors"

/// Opens one cell of a canonical shape. Exotic cells (library references,
/// pruned branches, Merkle proofs and updates) throw `BadSnake`.
@inline
fun sccpBeginParseOrdinary(c: cell): slice {
    val (cs, isExotic) = c.beginParseSpecial();
    assert (!isExotic) throw SccpErrors.BadSnake;
    return cs;
}

// ---------------------------------------------------------------------------
// SnakeBytes.

/// Parses a canonical `SnakeBytes` cell.
///
/// Every chunk with a continuation holds exactly 127 bytes and one reference;
/// the last chunk holds 1..=127 bytes and no reference; every chunk is an
/// ordinary cell. Returns the data-only slices of all chunks (in order) and
/// the total byte length. A total above `maxBytes` throws `lengthError`.
fun sccpParseSnakeBytes(root: cell, maxBytes: int, lengthError: int): (array<slice>, int) {
    var chunks: array<slice> = [];
    var total = 0;
    var cs = sccpBeginParseOrdinary(root);
    while (true) {
        val bits = cs.remainingBitsCount();
        val refs = cs.remainingRefsCount();
        if (refs == 1) {
            assert (bits == SCCP_SNAKE_CHUNK_BITS) throw SccpErrors.BadSnake;
            total += SCCP_SNAKE_CHUNK_BYTES;
            assert (total < maxBytes) throw lengthError;
            chunks.push(cs.preloadBits(bits));
            cs = sccpBeginParseOrdinary(cs.preloadRef());
        } else {
            assert (refs == 0) throw SccpErrors.BadSnake;
            assert (
                (bits >= 8) & (bits <= SCCP_SNAKE_CHUNK_BITS) & (bits % 8 == 0)
            ) throw SccpErrors.BadSnake;
            total += bits / 8;
            assert (total <= maxBytes) throw lengthError;
            chunks.push(cs);
            return (chunks, total);
        }
    }
}

/// Writer producing canonical `SnakeBytes` (127-byte chunks).
struct SnakeWriter {
    done: array<builder>
    cur: builder
}

fun SnakeWriter.create(): SnakeWriter {
    return { done: [], cur: beginCell() };
}

/// Appends a `bytes`-byte (≤ 32) big-endian unsigned integer.
fun SnakeWriter.writeUint(mutate self, value: int, bytes: int) {
    var bits = bytes * 8;
    var rest = value;
    while (bits > 0) {
        var room = SCCP_SNAKE_CHUNK_BITS - self.cur.bitsCount();
        if (room == 0) {
            self.done.push(self.cur);
            self.cur = beginCell();
            room = SCCP_SNAKE_CHUNK_BITS;
        }
        val take = min(room, bits);
        val low = bits - take;
        val part = rest >> low;
        self.cur.storeUint(part, take);
        rest -= part << low;
        bits = low;
    }
}

/// Appends the byte-aligned data bits of `s`.
fun SnakeWriter.writeSlice(mutate self, s: slice) {
    var data = s;
    while (!data.isEndOfBits()) {
        var room = SCCP_SNAKE_CHUNK_BITS - self.cur.bitsCount();
        if (room == 0) {
            self.done.push(self.cur);
            self.cur = beginCell();
            room = SCCP_SNAKE_CHUNK_BITS;
        }
        val take = min(room, data.remainingBitsCount());
        self.cur.storeSlice(data.loadBits(take));
    }
}

/// Returns the chunk builders (data only, in order) of a finished writer.
fun SnakeWriter.chunks(self): array<builder> {
    var all = self.done;
    all.push(self.cur);
    return all;
}

/// Links chunk builders into the canonical snake cell.
fun sccpSnakeCellFromChunks(chunks: array<builder>): cell {
    var i = chunks.size() - 1;
    var tail = chunks.get(i).endCell();
    while (i > 0) {
        i -= 1;
        tail = chunks.get(i).storeRef(tail).endCell();
    }
    return tail;
}

// ---------------------------------------------------------------------------
// HashChunk paths.

/// Parses `(Maybe ^HashChunk)` holding exactly `pathLen` hashes, 3 per chunk
/// except the last. Throws `BadProof` when `pathLen > maxLen` and `BadSnake`
/// for any non-canonical shape, exotic chunks included.
fun sccpLoadHashPath(path: cell?, pathLen: int, maxLen: int): array<int> {
    assert (pathLen <= maxLen) throw SccpErrors.BadProof;
    var hashes: array<int> = [];
    if (pathLen == 0) {
        assert (path == null) throw SccpErrors.BadSnake;
        return hashes;
    }
    assert (path != null) throw SccpErrors.BadSnake;
    var remaining = pathLen;
    var cs = sccpBeginParseOrdinary(path!);
    while (true) {
        val k = min(remaining, SCCP_HASHES_PER_CHUNK);
        remaining -= k;
        val more = remaining > 0 ? 1 : 0;
        assert (cs.remainingBitsCount() == k * 256 + 1) throw SccpErrors.BadSnake;
        assert (cs.remainingRefsCount() == more) throw SccpErrors.BadSnake;
        repeat (k) {
            hashes.push(cs.loadUint(256));
        }
        assert (cs.loadUint(1) == more) throw SccpErrors.BadSnake;
        if (more == 0) {
            return hashes;
        }
        cs = sccpBeginParseOrdinary(cs.preloadRef());
    }
}

/// Builds the canonical `(Maybe ^HashChunk)` for `hashes` (null when empty).
fun sccpBuildHashPath(hashes: array<int>): cell? {
    val count = hashes.size();
    if (count == 0) {
        return null;
    }
    var start = ((count - 1) / SCCP_HASHES_PER_CHUNK) * SCCP_HASHES_PER_CHUNK;
    var next: cell? = null;
    while (start >= 0) {
        var b = beginCell();
        val end = min(start + SCCP_HASHES_PER_CHUNK, count);
        var i = start;
        while (i < end) {
            b.storeUint(hashes.get(i), 256);
            i += 1;
        }
        b.storeMaybeRef(next);
        next = b.endCell();
        start -= SCCP_HASHES_PER_CHUNK;
    }
    return next;
}

// ---------------------------------------------------------------------------
// KeyChunk lists (§5.3.1).

/// A parsed `^KeyChunk` list: the `n` keys as 384-bit slices and the
/// data-only key bits of every chunk, in order (the §3.7 root preimage tail).
struct ParsedKeys {
    keys: array<slice>
    parts: array<slice>
}

/// Parses a `^KeyChunk` list of exactly `n` compressed keys, 2 per chunk
/// except the last (which holds 1 or 2). Every chunk must be an ordinary
/// cell of exactly `384k + 1` bits with the Maybe bit set iff a next chunk
/// follows; anything else throws `BadSnake`. `sccp_init` and key-changing
/// rotations store a list only after this check, so the stored committee
/// never depends on a masterchain library that could later be unpublished.
fun sccpLoadKeys(list: cell, n: int): ParsedKeys {
    var keys: array<slice> = [];
    var parts: array<slice> = [];
    var remaining = n;
    var cs = sccpBeginParseOrdinary(list);
    while (true) {
        val k = min(remaining, SCCP_KEYS_PER_CHUNK);
        remaining -= k;
        val more = remaining > 0 ? 1 : 0;
        assert (cs.remainingBitsCount() == k * SCCP_KEY_BITS + 1) throw SccpErrors.BadSnake;
        assert (cs.remainingRefsCount() == more) throw SccpErrors.BadSnake;
        parts.push(cs.preloadBits(k * SCCP_KEY_BITS));
        repeat (k) {
            keys.push(cs.loadBits(SCCP_KEY_BITS));
        }
        assert (cs.loadUint(1) == more) throw SccpErrors.BadSnake;
        if (more == 0) {
            return { keys, parts };
        }
        cs = sccpBeginParseOrdinary(cs.preloadRef());
    }
}

/// Builds the canonical `^KeyChunk` list for `keys` (at least one 384-bit slice).
fun sccpBuildKeys(keys: array<slice>): cell {
    val count = keys.size();
    var start = ((count - 1) / SCCP_KEYS_PER_CHUNK) * SCCP_KEYS_PER_CHUNK;
    var next: cell? = null;
    while (start >= 0) {
        var b = beginCell();
        val end = min(start + SCCP_KEYS_PER_CHUNK, count);
        var i = start;
        while (i < end) {
            b.storeSlice(keys.get(i));
            i += 1;
        }
        b.storeMaybeRef(next);
        next = b.endCell();
        start -= SCCP_KEYS_PER_CHUNK;
    }
    return next!;
}
