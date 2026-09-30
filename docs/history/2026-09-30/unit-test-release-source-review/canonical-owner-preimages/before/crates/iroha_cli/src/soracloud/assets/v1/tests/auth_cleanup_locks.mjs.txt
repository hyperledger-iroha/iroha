
function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

const now = 1_000_000;
statePut(challengeStateKey("expired"), {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  challenge_id: "expired",
  expires_at_unix_ms: now - 1
});
statePut(challengeStateKey("invalid-expiry"), {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  challenge_id: "",
  expires_at_unix_ms: "not-a-number"
});
statePut(challengeExpiredStateKey("old-marker"), {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  challenge_id: "old-marker",
  marked_at_unix_ms: now - AUTH_CHALLENGE_EXPIRED_TTL_MS - 1
});
statePut(challengeExpiredStateKey("fresh-marker"), {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  challenge_id: "fresh-marker",
  marked_at_unix_ms: now
});
statePut(challengeConsumeLockStateKey("stale-lock"), {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  challenge_id: "stale-lock",
  owner: "owner",
  expires_at_unix_ms: now - 1
});
statePut(sessionStateKey("expired-session"), {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  session_id: "expired-session",
  expires_at_unix_ms: now - 1
});

cleanupExpiredAuthRecords(now);

assert(stateGet(challengeStateKey("expired")) === null, "expired challenge should be removed");
assert(stateGet(challengeExpiredStateKey("expired"))?.challenge_id === "expired", "expired challenge marker should be written");
assert(stateGet(challengeStateKey("invalid-expiry")) === null, "invalid challenge expiry should be removed");
assert(stateGet(challengeExpiredStateKey("old-marker")) === null, "old expired marker should be deleted");
assert(stateGet(challengeExpiredStateKey("fresh-marker"))?.challenge_id === "fresh-marker", "fresh expired marker should remain");
assert(stateGet(challengeConsumeLockStateKey("stale-lock")) === null, "stale consume lock should be removed");
assert(stateGet(sessionStateKey("expired-session")) === null, "expired session should be removed");

const lock = acquireChallengeConsumeLock("active-lock", now);
assert(lock?.challenge_id === "active-lock", "first consume lock acquisition should succeed");
assert(acquireChallengeConsumeLock("active-lock", now) === null, "second consume lock acquisition should fail closed");
releaseChallengeConsumeLock({ challenge_id: "active-lock", owner: "wrong-owner" });
assert(stateGet(challengeConsumeLockStateKey("active-lock")) !== null, "wrong lock owner must not release the lock");
releaseChallengeConsumeLock(lock);
assert(stateGet(challengeConsumeLockStateKey("active-lock")) === null, "matching lock owner should release the lock");

statePut(challengeConsumeLockStateKey("reclaimed-lock"), {
  schema_version: AUTH_STATE_SCHEMA_VERSION,
  challenge_id: "reclaimed-lock",
  owner: "old-owner",
  expires_at_unix_ms: now - 1
});
assert(
  acquireChallengeConsumeLock("reclaimed-lock", now)?.challenge_id === "reclaimed-lock",
  "expired consume lock should be reclaimed"
);
