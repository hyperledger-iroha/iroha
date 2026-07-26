"""Streaming relay registry helpers (NSC-30)."""

from __future__ import annotations

import json
import math
import time
from dataclasses import dataclass, field, asdict
from pathlib import Path
from typing import Dict, Iterable, List, Optional

SECONDS_PER_MINUTE = 60


def _now() -> float:
    return time.time()


def _clamp(value: float, *, min_value: float = 0.0, max_value: float = 1.0) -> float:
    return max(min_value, min(max_value, value))


@dataclass
class Session:
    relay_id: str
    asset_id: str
    budget: float
    expected_chunks: int
    funded_at: float
    unused_budget: float = 0.0

    def per_chunk_reward(self) -> float:
        if self.expected_chunks <= 0:
            raise ValueError("expected_chunks must be positive")
        return self.budget / self.expected_chunks


@dataclass
class RelayRecord:
    relay_id: str
    operator: str
    advertised_mbps: float
    registered_at: float
    bond: float
    reputation: float = 0.5
    strikes: int = 0
    locked_until: float = 0.0
    pending_rewards: Dict[str, float] = field(default_factory=dict)
    audit_frozen: bool = False

    def is_locked(self, now: float) -> bool:
        return now < self.locked_until


class StreamingRelayRegistry:
    """In-memory helper modelling the registry contract behaviour."""

    BASE_BOND = 1_000.0
    GRACE_PERIOD_SECONDS = 30 * SECONDS_PER_MINUTE
    HALF_LIFE_HOURS = 4.0

    def __init__(
        self,
        *,
        records: Optional[Dict[str, RelayRecord]] = None,
        sessions: Optional[Dict[str, Session]] = None,
    ) -> None:
        self.records: Dict[str, RelayRecord] = records or {}
        self.sessions: Dict[str, Session] = sessions or {}

    # -- Persistence -----------------------------------------------------

    @classmethod
    def load(cls, path: Path) -> "StreamingRelayRegistry":
        if not path.exists():
            return cls()
        payload = json.loads(path.read_text())
        records = {
            k: RelayRecord(**v)  # type: ignore[arg-type]
            for k, v in payload.get("records", {}).items()
        }
        sessions = {
            k: Session(**v)  # type: ignore[arg-type]
            for k, v in payload.get("sessions", {}).items()
        }
        return cls(records=records, sessions=sessions)

    def dump(self, path: Path) -> None:
        payload = {
            "records": {k: asdict(v) for k, v in self.records.items()},
            "sessions": {k: asdict(v) for k, v in self.sessions.items()},
        }
        path.write_text(json.dumps(payload, indent=2, sort_keys=True))

    # -- Registry operations --------------------------------------------

    @staticmethod
    def compute_bond(advertised_mbps: float) -> float:
        if advertised_mbps < 0:
            raise ValueError("advertised throughput must be non-negative")
        return StreamingRelayRegistry.BASE_BOND * math.log2(
            1.0 + advertised_mbps / 5.0
        )

    def register_relay(
        self,
        relay_id: str,
        operator: str,
        *,
        advertised_mbps: float,
        bonded_amount: Optional[float] = None,
        now: Optional[float] = None,
    ) -> RelayRecord:
        ts = _now() if now is None else now
        if relay_id in self.records:
            raise ValueError(f"relay {relay_id} already registered")
        required_bond = self.compute_bond(advertised_mbps)
        actual_bond = bonded_amount if bonded_amount is not None else required_bond
        if actual_bond < required_bond:
            raise ValueError(
                f"bond {actual_bond:.2f} below required minimum {required_bond:.2f}"
            )
        record = RelayRecord(
            relay_id=relay_id,
            operator=operator,
            advertised_mbps=advertised_mbps,
            registered_at=ts,
            bond=actual_bond,
        )
        self.records[relay_id] = record
        return record

    def fund_session(
        self,
        session_id: str,
        relay_id: str,
        *,
        asset_id: str,
        budget: float,
        expected_chunks: int,
        now: Optional[float] = None,
    ) -> Session:
        if session_id in self.sessions:
            raise ValueError(f"session {session_id} already exists")
        if relay_id not in self.records:
            raise ValueError(f"relay {relay_id} not registered")
        session = Session(
            relay_id=relay_id,
            asset_id=asset_id,
            budget=budget,
            expected_chunks=expected_chunks,
            funded_at=_now() if now is None else now,
        )
        self.sessions[session_id] = session
        return session

    def record_performance(
        self,
        session_id: str,
        *,
        delivered_chunks: int,
        missed_chunks: int,
        acknowledged_chunks: Optional[int] = None,
        corrupted_chunks: int = 0,
        stale_proofs: bool = False,
        viewer_sessions: int = 0,
        viewer_rebuffer_events: int = 0,
        avg_latency_ms: float = 0.0,
        baseline_latency_ms: float = 0.0,
        governance_flag: bool = False,
        elapsed_seconds: int = 5 * SECONDS_PER_MINUTE,
        now: Optional[float] = None,
    ) -> RelayRecord:
        session = self.sessions.get(session_id)
        if session is None:
            raise ValueError(f"session {session_id} not found")
        relay = self.records.get(session.relay_id)
        if relay is None:
            raise ValueError(f"relay {session.relay_id} not registered")

        ts = _now() if now is None else now
        total = delivered_chunks + missed_chunks
        success_ratio = delivered_chunks / total if total > 0 else 1.0

        if viewer_sessions > 0:
            viewer_score = 1.0 - (viewer_rebuffer_events / viewer_sessions)
        else:
            viewer_score = 1.0
        viewer_score = _clamp(viewer_score)

        if baseline_latency_ms > 0:
            latency_ratio = (baseline_latency_ms / max(avg_latency_ms, 1.0))
            latency_score = _clamp(latency_ratio, max_value=1.25)
        else:
            latency_score = 1.0
        latency_score = _clamp(latency_score / 1.25)

        governance_score = 0.0 if governance_flag else 1.0

        composite = (
            0.5 * success_ratio
            + 0.2 * latency_score
            + 0.2 * viewer_score
            + 0.1 * governance_score
        )
        composite = _clamp(composite)

        # EMA with 4h half-life.
        delta_hours = elapsed_seconds / 3600.0
        alpha = 1.0 - 0.5 ** (delta_hours / self.HALF_LIFE_HOURS)
        relay.reputation = (1.0 - alpha) * relay.reputation + alpha * composite

        # Reward accounting.
        per_chunk = session.per_chunk_reward()
        acknowledged = (
            acknowledged_chunks if acknowledged_chunks is not None else delivered_chunks
        )
        relay.pending_rewards.setdefault(session.asset_id, 0.0)
        relay.pending_rewards[session.asset_id] += per_chunk * acknowledged * 0.90
        # Budget reimbursements and treasury share tracked as unused budget.
        session.unused_budget += per_chunk * acknowledged * 0.08
        session.budget -= per_chunk * acknowledged

        if success_ratio >= 1.0 and missed_chunks == 0:
            # decay idle penalty
            relay.reputation = min(1.0, max(relay.reputation, 0.5))

        # Check slashing triggers (respect grace period except for corrupted chunks).
        in_grace = ts - relay.registered_at < self.GRACE_PERIOD_SECONDS
        triggers: Iterable[str] = []
        if total > 0 and (missed_chunks / total) >= 0.15 and not in_grace:
            triggers = (*triggers, "missed")
        if corrupted_chunks > 0:
            triggers = (*triggers, "corrupted")
        if stale_proofs and not in_grace:
            triggers = (*triggers, "stale_proof")

        if triggers:
            self._apply_slash(relay, ts, reason=", ".join(triggers))

        return relay

    def claim_rewards(
        self, relay_id: str, asset_id: str, *, treasury_fee: float = 0.02
    ) -> float:
        relay = self.records.get(relay_id)
        if relay is None:
            raise ValueError(f"relay {relay_id} not registered")
        amount = relay.pending_rewards.get(asset_id, 0.0)
        if amount <= 0:
            return 0.0
        treasury_cut = amount * treasury_fee
        relay.pending_rewards[asset_id] = 0.0
        return amount - treasury_cut

    def rank_relays(
        self,
        *,
        min_reputation: float = 0.0,
        include_ineligible: bool = False,
        now: Optional[float] = None,
    ) -> List[Dict[str, object]]:
        """Return relay ordering based on reputation and bonded collateral."""

        ts = _now() if now is None else now
        rankings: List[Dict[str, object]] = []
        for record in self.records.values():
            collateral_factor = math.log1p(max(record.bond, 0.0))
            score = record.reputation * collateral_factor
            eligibility_reason: Optional[str] = None

            if record.audit_frozen:
                eligibility_reason = "audit_frozen"
            elif record.is_locked(ts):
                eligibility_reason = "locked"
            elif record.reputation < min_reputation:
                eligibility_reason = "reputation_below_threshold"

            eligible = eligibility_reason is None
            if eligible or include_ineligible:
                rankings.append(
                    {
                        "relay_id": record.relay_id,
                        "operator": record.operator,
                        "reputation": round(record.reputation, 6),
                        "bond": round(record.bond, 6),
                        "score": round(score, 6),
                        "eligible": eligible,
                        "reason": eligibility_reason,
                        "locked_until": record.locked_until,
                        "audit_frozen": record.audit_frozen,
                    }
                )

        rankings.sort(key=lambda entry: entry["score"], reverse=True)
        return rankings

    # -- Internal utilities ---------------------------------------------

    def _apply_slash(self, relay: RelayRecord, now: float, *, reason: str) -> None:
        relay.strikes += 1
        if relay.strikes == 1:
            penalty, lock_minutes = 0.05, 10
        elif relay.strikes == 2:
            penalty, lock_minutes = 0.20, 60
        else:
            penalty, lock_minutes = 1.0, 72 * 60
            relay.audit_frozen = True
        slash_amount = relay.bond * penalty
        relay.bond = max(0.0, relay.bond - slash_amount)
        relay.locked_until = now + lock_minutes * SECONDS_PER_MINUTE
        relay.reputation = max(0.0, relay.reputation - penalty * 0.5)
