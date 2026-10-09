"""Executable model of the SCCP destination contracts (`specs/sccp.md` §5.1).

One `Destination` models one deployed revision: the committee state machine
(§5.1.2), `VerifyCertificate` (§5.1.3), checkpoints, finalization (§5.1.5),
controls (§5.1.6) and voids (§5.1.8). Every entry point is atomic like an EVM
call: a rejection restores the state and discards the events, while a latch or
a rotation that leaves the destination not `Live` persists and stops
processing. Outcomes use the §5.2.2 error names.

`flavor = "evm"` follows the EVM inputs (`TairaCertificateV1` with the
committee and `signerYs` in calldata, unlimited checkpoints). `flavor = "ton"`
reads the keys from storage, keeps three checkpoint slots and needs
`next_keys` for a key-changing rotation. TON's asynchronous bucket and wallet
messages are not modelled: a TON finalize or void is shown with the outcome its
minter-side checks give (`TODO:` model the bucket round trip of §5.3.4).

Reference choices where the specification names no error (all pinned in
`fixtures/sccp/committee_transitions_v1.json` under `error_choices`):

* a call on a destination that is not `Live` answers `Equivocated` when latched
  and `CommitteeNotAccepted` when expired;
* step 1 length failures answer `BadCertificate` (QC_FIXED, signature,
  `signerYs`), `BadHeader` (X) and `BadCommittee` (committee);
* step 6 and step 10 key failures (flags, sign bit, `y ≥ p`, off-curve `y`,
  `apk = O`) answer `BadCommittee`; signature encoding, subgroup and pairing
  failures answer `BadSignature`;
* `voidFrozen` range failures answer `BadAmount` (EVM) and `BadRange` (TON).
"""

from __future__ import annotations

import copy
from dataclasses import dataclass, field

from .bls12_381 import (
    G1_GENERATOR,
    P,
    PointEncodingError,
    g1_add,
    g1_decompress,
    g1_is_on_curve,
    g1_neg,
    g2_decompress,
    g2_from_evm192,
    g2_in_subgroup,
    pairing_product_is_one,
)
from .common import (
    MAX_CERTIFICATES_PER_CALL,
    MAX_FAST_PAUSE_MS,
    MAX_VOID_FROZEN_RANGE_EVM,
    MAX_VOID_FROZEN_RANGE_TON,
    TON_CHECKPOINT_SLOTS,
    SccpError,
)
from .finality import (
    QC_FIXED_LEN,
    X_LEN,
    ZERO32,
    Certificate,
    FinalityHeader,
    checkpoint_id,
    commit_message,
    commit_preimage,
    committee_n_valid,
    committee_root,
    parse_header,
    quorum,
    result_r,
    y_sign_flag_matches,
)
from .hash_to_curve import hash_to_g2
from .merkle import check_block_ref, check_control_constraints, check_leaf, control_leaf, transfer_leaf
from .payload import TAIRA, TON, Profile, decode_payload, message_id

_VERIFY_CACHE: dict[tuple, bool] = {}


def cached_pairing_check(apk, message: bytes, sigma) -> bool:
    """`e(apk, H(m)) · e(−g1, σ) = 1`, memoized on its exact inputs (pure function)."""
    key = (apk, message, sigma)
    if key not in _VERIFY_CACHE:
        _VERIFY_CACHE[key] = pairing_product_is_one([(apk, hash_to_g2(message)), (g1_neg(G1_GENERATOR), sigma)])
    return _VERIFY_CACHE[key]


@dataclass
class BlockRef:
    """`BlockRefV1`."""

    height: int
    sccp_root: bytes
    message_count: int
    history_index: int = 0
    history_path: list[bytes] = field(default_factory=list)

    def as_json(self) -> dict:
        return {
            "height": self.height,
            "sccpRoot": "0x" + self.sccp_root.hex(),
            "messageCount": self.message_count,
            "historyIndex": self.history_index,
            "historyPath": ["0x" + h.hex() for h in self.history_path],
        }


@dataclass
class MessageProof:
    """`MessageProofV1`."""

    payload: bytes
    leaf_index: int
    path: list[bytes]
    block: BlockRef

    def as_json(self) -> dict:
        return {
            "payload": "0x" + self.payload.hex(),
            "leafIndex": self.leaf_index,
            "path": ["0x" + h.hex() for h in self.path],
            "block": self.block.as_json(),
        }


@dataclass
class ControlProof:
    """`ControlProofV1`."""

    control_nonce: int
    track: int
    parliament_paused: int
    fast_pause_until_ms: int
    certificate_id: bytes
    effect_hash: bytes
    leaf_index: int
    path: list[bytes]
    block: BlockRef

    def as_json(self) -> dict:
        return {
            "controlNonce": self.control_nonce,
            "track": self.track,
            "parliamentPaused": bool(self.parliament_paused) if self.parliament_paused in (0, 1) else self.parliament_paused,
            "fastPauseUntilMs": self.fast_pause_until_ms,
            "certificateId": "0x" + self.certificate_id.hex(),
            "effectHash": "0x" + self.effect_hash.hex(),
            "leafIndex": self.leaf_index,
            "path": ["0x" + h.hex() for h in self.path],
            "block": self.block.as_json(),
        }


@dataclass
class Deployment:
    """Immutables of one destination (§5.1.1)."""

    flavor: str
    taira_network_id: bytes
    consensus_instance: bytes
    network: Profile
    destination_word: bytes
    route_revision: int
    max_wrapped_supply: int
    pin_generation: int
    pin_root: bytes
    pin_start_height: int
    pin_until_ms: int
    pin_keys: list[bytes] | None = None
    cheap_path: bool = True

    def as_json(self) -> dict:
        return {
            "flavor": self.flavor,
            "taira_network_id": "0x" + self.taira_network_id.hex(),
            "consensus_instance": "0x" + self.consensus_instance.hex(),
            "network": self.network.name,
            "destination_word": "0x" + self.destination_word.hex(),
            "route_revision": self.route_revision,
            "max_wrapped_supply": str(self.max_wrapped_supply),
            "pin": {
                "generation": self.pin_generation,
                "committee_root": "0x" + self.pin_root.hex(),
                "start_height": self.pin_start_height,
                "until_ms": self.pin_until_ms,
                "keys": None if self.pin_keys is None else ["0x" + k.hex() for k in self.pin_keys],
            },
            "cheap_path": self.cheap_path,
        }


@dataclass
class State:
    """Mutable destination state (§5.1.1)."""

    root: bytes
    generation: int
    start: int
    high: int
    until_ms: int
    keys: list[bytes] | None
    checkpoints: dict[int, bytes] = field(default_factory=dict)
    equivocated: bool = False
    parliament_paused: bool = False
    fast_pause_until_ms: int = 0
    control_nonce: int = 0
    op_count: int = 0
    consumed: set[int] = field(default_factory=set)
    total_supply: int = 0
    minted: dict[str, int] = field(default_factory=dict)

    def as_json(self, now_ms: int | None = None) -> dict:
        out = {
            "committee": {
                "root": "0x" + self.root.hex(),
                "generation": self.generation,
                "start": self.start,
                "high": self.high,
                "until_ms": self.until_ms,
            },
            "checkpoints": [
                {"height": h, "id": "0x" + self.checkpoints[h].hex()} for h in sorted(self.checkpoints)
            ],
            "equivocated": self.equivocated,
            "pause": {
                "parliament_paused": self.parliament_paused,
                "fast_pause_until_ms": self.fast_pause_until_ms,
                "control_nonce": self.control_nonce,
            },
            "op_count": self.op_count,
            "consumed": sorted(self.consumed),
            "total_supply": str(self.total_supply),
        }
        if self.keys is not None:
            out["committee"]["keys"] = ["0x" + k.hex() for k in self.keys]
        if now_ms is not None:
            out["live"] = (not self.equivocated) and now_ms <= self.until_ms
            out["pause"]["effective"] = self.parliament_paused or now_ms < self.fast_pause_until_ms
        return out


class _Stop(Exception):
    """Internal: a latch or a lost `Live` ends `submitCheckpoints` processing (persisted)."""

    def __init__(self, reason: str):
        super().__init__(reason)
        self.reason = reason


class Destination:
    """One destination revision with its state and event log."""

    def __init__(self, deployment: Deployment):
        self.d = deployment
        self.state = State(
            root=deployment.pin_root,
            generation=deployment.pin_generation,
            start=deployment.pin_start_height,
            high=deployment.pin_start_height - 1,
            until_ms=deployment.pin_until_ms,
            keys=list(deployment.pin_keys) if deployment.flavor == "ton" else None,
        )
        self.events: list[dict] = []

    # -- helpers -----------------------------------------------------------

    def live(self, now_ms: int) -> bool:
        return not self.state.equivocated and now_ms <= self.state.until_ms

    def effective_pause(self, now_ms: int) -> bool:
        return self.state.parliament_paused or now_ms < self.state.fast_pause_until_ms

    def _require_live(self, now_ms: int) -> None:
        if self.state.equivocated:
            raise SccpError("Equivocated")
        if now_ms > self.state.until_ms:
            raise SccpError("CommitteeNotAccepted", "destination expired")

    def _emit(self, name: str, **fields) -> None:
        self.events.append({"event": name, **fields})

    def _run(self, fn, *args):
        """Run an entry point atomically; return the outcome record."""
        saved_state = copy.deepcopy(self.state)
        saved_events = len(self.events)
        try:
            result, stopped = fn(*args)
        except SccpError as err:
            self.state = saved_state
            del self.events[saved_events:]
            out = {"outcome": err.name}
            if err.args_map:
                out["error_args"] = err.args_map
            if err.detail:
                # Which check failed (not part of any fixture: outcomes are compared by name).
                out["detail"] = err.detail
            return out
        out = {"outcome": "ok", "events": self.events[saved_events:]}
        if stopped:
            out["stopped"] = stopped
        if result is not None:
            out["return"] = result
        return out

    # -- VerifyCertificate (§5.1.3) --------------------------------------------

    def _verify(self, cert: Certificate):
        """Steps 1–12; returns `(X, id)` with the certificate published if the full path ran."""
        st = self.state
        evm = self.d.flavor == "evm"
        # 1. exact lengths
        if len(cert.qc) != QC_FIXED_LEN:
            raise SccpError("BadCertificate", "QC_FIXED length")
        if len(cert.header) != X_LEN:
            raise SccpError("BadHeader", "X length")
        if evm:
            if len(cert.signature_evm) != 192:
                raise SccpError("BadCertificate", "signature length")
            if len(cert.committee) % 48 or not committee_n_valid(len(cert.committee) // 48):
                raise SccpError("BadCommittee", "committee length")
            n = len(cert.committee) // 48
            if len(cert.signer_ys) != 48 * quorum(n):
                raise SccpError("BadCertificate", "signerYs length")
            keys = [cert.committee[48 * i : 48 * i + 48] for i in range(n)]
        else:
            if len(cert.signature_ton) != 96:
                raise SccpError("BadCertificate", "signature length")
            keys = st.keys
            n = len(keys)
        # 2. parse X
        try:
            x = parse_header(cert.header)
        except SccpError as err:
            raise SccpError("BadHeader", err.name) from None
        if x.network_id != self.d.taira_network_id:
            raise SccpError("BadHeader", "network")
        if not x.active:
            raise SccpError("BadHeader", "inactive X")
        # 3. classify
        if x.generation != st.generation or x.committee_root != st.root:
            raise SccpError("CommitteeNotAccepted")
        # 4. committee root (EVM)
        if evm and committee_root(keys) != st.root:
            raise SccpError("BadCommittee", "committee root mismatch")
        # 5. signers
        qc = cert.qc
        signers = int.from_bytes(qc[112:116], "big")
        if signers >> n:
            raise SccpError("BadCertificate", "spare signer bits")
        indices = [i for i in range(n) if signers >> i & 1]
        if len(indices) != quorum(n):
            raise SccpError("BadCertificate", "popcount")
        # 6. EVM encodings, before any precompile
        if evm:
            ys = [int.from_bytes(cert.signer_ys[48 * j : 48 * j + 48], "big") for j in range(len(indices))]
            limbs = [int.from_bytes(cert.signature_evm[48 * j : 48 * j + 48], "big") for j in range(4)]
            if any(y >= P for y in ys):
                raise SccpError("BadCommittee", "signer y not reduced")
            if any(limb >= P for limb in limbs):
                raise SccpError("BadSignature", "signature limb not reduced")
            if not any(cert.signature_evm):
                raise SccpError("BadSignature", "zero signature")
            for i, y in zip(indices, ys):
                key = keys[i]
                if key[0] & 0xC0 != 0x80:
                    raise SccpError("BadCommittee", "key flags")
                if not y_sign_flag_matches(key, y):
                    raise SccpError("BadCommittee", "key sign bit")
        # 7. id and R
        cid = checkpoint_id(cert.header)
        result = result_r(cert.header, qc[80:112])
        # 8. cheap path
        if self.d.cheap_path and st.checkpoints.get(x.height) == cid:
            return x, cid
        # 9. m
        message = commit_message(
            commit_preimage(
                self.d.consensus_instance,
                int.from_bytes(qc[0:8], "big"),
                qc[8:40],
                x.height,
                int.from_bytes(qc[40:48], "big"),
                qc[48:80],
                result,
            )
        )
        # 10. aggregate
        apk = None
        if evm:
            for i, y in zip(indices, ys):
                xcoord = int.from_bytes(bytes([keys[i][0] & 0x1F]) + keys[i][1:], "big")
                point = (xcoord, y)
                if xcoord >= P or not g1_is_on_curve(point):
                    raise SccpError("BadCommittee", "signer key off curve")
                apk = g1_add(apk, point)
            try:
                sigma = g2_from_evm192(cert.signature_evm)
            except PointEncodingError:
                raise SccpError("BadSignature", "signature not on curve") from None
        else:
            for i in indices:
                apk = g1_add(apk, g1_decompress(keys[i]))
            try:
                sigma = g2_decompress(cert.signature_ton)
            except PointEncodingError:
                raise SccpError("BadSignature", "signature encoding") from None
        if apk is None:
            raise SccpError("BadCommittee", "aggregate key is the identity")
        # 11. pairing (subgroup checks as EIP-2537 PAIRING_CHECK and TVM do)
        if sigma is None or not g2_in_subgroup(sigma):
            raise SccpError("BadSignature", "signature not in G2")
        if not cached_pairing_check(apk, message, sigma):
            raise SccpError("BadSignature", "pairing")
        # 12. publish
        self._emit(
            "SccpCertificate",
            height=x.height,
            generation=x.generation,
            qc="0x" + cert.qc.hex(),
            header="0x" + cert.header.hex(),
            signature="0x" + (cert.signature_evm if evm else cert.signature_ton).hex(),
        )
        return x, cid

    def _violation(self, x: FinalityHeader, cid: bytes):
        """V1–V3 (§5.1.2) as `(reason, a, b)` or `None`."""
        st = self.state
        stored = st.checkpoints.get(x.height)
        if stored is not None and stored != cid:
            return 1, stored, cid
        if x.height < st.start:
            return 2, x.committee_root, cid
        if x.rotation and x.height < st.high:
            return 3, x.committee_root, cid
        return None

    def _record_checkpoint(self, x: FinalityHeader, cid: bytes) -> bool:
        st = self.state
        if x.height in st.checkpoints:
            return False
        if self.d.flavor == "ton" and len(st.checkpoints) >= TON_CHECKPOINT_SLOTS:
            lowest = min(st.checkpoints)
            if x.height <= lowest:
                return False
            del st.checkpoints[lowest]
        st.checkpoints[x.height] = cid
        self._emit("SccpCheckpointed", height=x.height, header="0x" + cid.hex())
        return True

    def _inline_x(self, cert: Certificate, now_ms: int) -> FinalityHeader:
        """Inline use: VerifyCertificate as CUR, V1–V3 → `LatchRequired`, checkpoint and `high`."""
        x, cid = self._verify(cert)
        if self._violation(x, cid) is not None:
            raise SccpError("LatchRequired")
        self._record_checkpoint(x, cid)
        self.state.high = max(self.state.high, x.height)
        return x

    def _checkpoint_x(self, header: bytes, now_ms: int) -> FinalityHeader:
        """Checkpoint use (§5.1.3): stored id, X1–X7 with `ACTIVE`, `Live`."""
        if len(header) != X_LEN:
            raise SccpError("BadHeader", "X length")
        height = int.from_bytes(header[48:56], "big")
        stored = self.state.checkpoints.get(height)
        if stored is None or stored != checkpoint_id(header):
            raise SccpError("UnknownCheckpoint")
        try:
            x = parse_header(header)
        except SccpError as err:
            raise SccpError("BadHeader", err.name) from None
        if not x.active:
            raise SccpError("BadHeader", "inactive X")
        self._require_live(now_ms)
        return x

    def _obtain_x(self, source: tuple, now_ms: int) -> FinalityHeader:
        kind, value = source
        if kind == "inline":
            return self._inline_x(value, now_ms)
        if kind == "checkpoint":
            return self._checkpoint_x(value, now_ms)
        raise ValueError(kind)

    # -- submitCheckpoints (§5.1.2, §5.1.3) -------------------------------------

    def submit_checkpoints(self, certs: list[Certificate], now_ms: int, next_keys: list | None = None) -> dict:
        """`submitCheckpoints(certs)` (EVM) or a sequence of `sccp_checkpoint` messages (TON: one each)."""
        return self._run(self._submit_checkpoints, certs, now_ms, next_keys)

    def _submit_checkpoints(self, certs, now_ms, next_keys):
        if len(certs) > MAX_CERTIFICATES_PER_CALL:
            raise SccpError("TooManyCertificates")
        if not certs:
            raise SccpError("NothingToDo")
        self._require_live(now_ms)
        changed = False
        stopped = None
        try:
            for index, cert in enumerate(certs):
                keys_for_rotation = None if next_keys is None else next_keys[index]
                changed |= self._apply_checkpoint(cert, now_ms, keys_for_rotation)
        except _Stop as stop:
            changed = True
            stopped = stop.reason
        if not changed:
            raise SccpError("NothingToDo")
        return None, stopped

    def _apply_checkpoint(self, cert: Certificate, now_ms: int, next_keys: list[bytes] | None) -> bool:
        st = self.state
        x, cid = self._verify(cert)
        violation = self._violation(x, cid)
        if violation is not None:
            reason, a, b = violation
            st.equivocated = True
            st.until_ms = 0
            self._emit(
                "SccpLatched", reason=reason, generation=x.generation, height=x.height, a="0x" + a.hex(), b="0x" + b.hex()
            )
            raise _Stop("latch")
        changed = self._record_checkpoint(x, cid)
        if x.height > st.high:
            st.high = x.height
            changed = True
        if x.rotation:
            if self.d.flavor == "ton" and x.next_committee_root != st.root:
                if (
                    next_keys is None
                    or not committee_n_valid(len(next_keys))
                    or any(next_keys[i] >= next_keys[i + 1] for i in range(len(next_keys) - 1))
                    or committee_root(next_keys) != x.next_committee_root
                ):
                    raise SccpError("BadCommittee", "next_keys")
                st.keys = list(next_keys)
            st.root = x.next_committee_root
            st.generation += 1
            st.start = x.height + 1
            st.high = x.height
            st.until_ms = min(x.timestamp_ms, now_ms) + x.validity_ms
            self._emit(
                "SccpCommitteeRotated",
                generation=st.generation,
                root="0x" + st.root.hex(),
                untilMs=st.until_ms,
                startHeight=st.start,
            )
            changed = True
            if not self.live(now_ms):
                raise _Stop("frozen")
        return changed

    # -- finalize (§5.1.5) ---------------------------------------------------

    def finalize(self, source: tuple, proof: MessageProof, now_ms: int) -> dict:
        """`finalizeFromTaira(cert, proof)` with `("inline", cert)`, or `finalizeFromCheckpoint` with `("checkpoint", X)`."""
        return self._run(self._finalize, source, proof, now_ms)

    def _check_message(self, x: FinalityHeader, proof: MessageProof, now_ms: int, *, void: bool):
        b = proof.block
        check_block_ref(b.height, b.sccp_root, b.message_count, b.history_index, b.history_path, x)
        payload = decode_payload(proof.payload, check_recipient=False)
        if (
            payload.source_domain != 0
            or payload.dest_domain != self.d.network.domain
            or payload.route_revision != self.d.route_revision
        ):
            raise SccpError("BadPayload", "not addressed to this revision")
        recipient = payload.recipient
        if self.d.network is TON:
            # addr_std in workchain 0 with a nonzero account id that is not the minter.
            if recipient[:4] != bytes(4) or not any(recipient[4:]) or recipient[4:] == self.d.destination_word:
                raise SccpError("BadRecipient")
        elif not any(recipient) or recipient == self.d.destination_word[12:]:
            raise SccpError("BadRecipient")
        if void:
            if now_ms <= payload.deadline_ms:
                raise SccpError("DeadlineNotReached")
        elif now_ms > payload.deadline_ms:
            raise SccpError("DeadlinePassed")
        mid = message_id(TAIRA, self.d.network, self.d.taira_network_id, proof.payload)
        check_leaf(transfer_leaf(mid, self.d.destination_word), proof.leaf_index, b.message_count, proof.path, b.sccp_root)
        return payload, mid

    def _finalize(self, source, proof, now_ms):
        x = self._obtain_x(source, now_ms)
        self._require_live(now_ms)
        if self.effective_pause(now_ms):
            raise SccpError("MintingIsPaused")
        payload, mid = self._check_message(x, proof, now_ms, void=False)
        st = self.state
        if payload.nonce in st.consumed:
            raise SccpError("AlreadyConsumed", nonce=payload.nonce)
        st.consumed.add(payload.nonce)
        if st.total_supply + payload.amount > self.d.max_wrapped_supply:
            raise SccpError("SupplyCapExceeded")
        st.total_supply += payload.amount
        recipient = "0x" + payload.recipient.hex()
        st.minted[recipient] = st.minted.get(recipient, 0) + payload.amount
        st.op_count += 1
        self._emit(
            "SccpFinalized", messageId="0x" + mid.hex(), nonce=payload.nonce, recipient=recipient, tokenAmount=str(payload.amount)
        )
        return "0x" + mid.hex(), None

    # -- applyControl (§5.1.6) -----------------------------------------------

    def apply_control(self, source: tuple, proof: ControlProof, now_ms: int) -> dict:
        """`applyControl` / `applyControlFromCheckpoint`."""
        return self._run(self._apply_control, source, proof, now_ms)

    def _apply_control(self, source, proof, now_ms):
        x = self._obtain_x(source, now_ms)
        self._require_live(now_ms)
        b = proof.block
        check_block_ref(b.height, b.sccp_root, b.message_count, b.history_index, b.history_path, x)
        leaf = control_leaf(
            self.d.taira_network_id,
            self.d.network,
            self.d.destination_word,
            self.d.route_revision,
            proof.control_nonce,
            proof.track,
            proof.parliament_paused,
            proof.fast_pause_until_ms,
            proof.certificate_id,
            proof.effect_hash,
            check=False,
        )
        check_leaf(leaf, proof.leaf_index, b.message_count, proof.path, b.sccp_root)
        check_control_constraints(
            proof.track, proof.parliament_paused, proof.fast_pause_until_ms, proof.certificate_id, proof.effect_hash
        )
        st = self.state
        if proof.control_nonce <= st.control_nonce:
            raise SccpError("StaleControl")
        st.parliament_paused = bool(proof.parliament_paused)
        st.fast_pause_until_ms = min(proof.fast_pause_until_ms, now_ms + MAX_FAST_PAUSE_MS)
        st.control_nonce = proof.control_nonce
        st.op_count += 1
        self._emit(
            "SccpControlApplied",
            controlNonce=proof.control_nonce,
            track=proof.track,
            parliamentPaused=st.parliament_paused,
            fastPauseUntilMs=st.fast_pause_until_ms,
            certificateId="0x" + proof.certificate_id.hex(),
        )
        return None, None

    # -- voids (§5.1.8) ------------------------------------------------------

    def void_expired(self, nonce: int, source: tuple, proof: MessageProof, now_ms: int) -> dict:
        """`voidExpired(nonce, cert, proof)` / `voidExpiredFromCheckpoint(nonce, ref, proof)`."""
        return self._run(self._void_expired, nonce, source, proof, now_ms)

    def _void_expired(self, nonce, source, proof, now_ms):
        x = self._obtain_x(source, now_ms)
        self._require_live(now_ms)
        payload, mid = self._check_message(x, proof, now_ms, void=True)
        if nonce != payload.nonce:
            raise SccpError("BadNonce", expected=payload.nonce)
        st = self.state
        if payload.nonce in st.consumed:
            raise SccpError("AlreadyConsumed", nonce=payload.nonce)
        st.consumed.add(payload.nonce)
        st.op_count += 1
        self._emit("SccpVoided", messageId="0x" + mid.hex(), nonce=payload.nonce)
        return None, None

    def void_frozen(self, first_nonce: int, count: int, now_ms: int) -> dict:
        """`voidFrozen(firstNonce, count)` (EVM; TON `sccp_void_frozen` within one bucket)."""
        return self._run(self._void_frozen, first_nonce, count, now_ms)

    def _void_frozen(self, first, count, now_ms):
        evm = self.d.flavor == "evm"
        limit = MAX_VOID_FROZEN_RANGE_EVM if evm else MAX_VOID_FROZEN_RANGE_TON
        bad_range = count == 0 or count > limit or first + count > 1 << 64
        if not evm and not bad_range:
            bad_range = first >> 9 != (first + count - 1) >> 9
        if bad_range:
            raise SccpError("BadAmount" if evm else "BadRange")
        if self.live(now_ms):
            raise SccpError("NotFrozen")
        st = self.state
        for nonce in range(first, first + count):
            if nonce in st.consumed:
                raise SccpError("AlreadyConsumed", nonce=nonce)
        st.consumed.update(range(first, first + count))
        st.op_count += 1
        if evm:
            for nonce in range(first, first + count):
                self._emit("SccpVoided", messageId="0x" + ZERO32.hex(), nonce=nonce)
        else:
            self._emit("SccpVoided", messageId="0x" + ZERO32.hex(), firstNonce=first, count=count)
        return None, None

    # -- verification only ---------------------------------------------------

    def verify_certificate(self, cert: Certificate, now_ms: int) -> dict:
        """`VerifyCertificate` steps 1–12 against the current state, with no state change.

        Returns the outcome of the checks only (`ok` or the error name); the
        violation predicates and entry-point actions are not evaluated.
        """

        def check():
            self._require_live(now_ms)
            self._verify(cert)
            return None, None

        saved_state = copy.deepcopy(self.state)
        saved_events = len(self.events)
        out = self._run(check)
        self.state = saved_state
        del self.events[saved_events:]
        out.pop("events", None)
        return out

    # -- views ---------------------------------------------------------------

    def pause_state(self, now_ms: int) -> dict:
        """`pauseState()` and `mintingPaused()` at `now_ms`."""
        st = self.state
        return {
            "parliamentPaused": st.parliament_paused,
            "fastPauseUntilMs": st.fast_pause_until_ms,
            "controlNonce": st.control_nonce,
            "effective": self.effective_pause(now_ms),
        }
