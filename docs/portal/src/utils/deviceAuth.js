export const DEVICE_GRANT_TYPE = 'urn:ietf:params:oauth:grant-type:device_code';

function normalisePositiveSeconds(value, fallbackSeconds) {
  const parsed = Number(value);
  if (Number.isFinite(parsed) && parsed > 0) {
    return parsed;
  }
  return Math.max(1, fallbackSeconds);
}

export function buildDeviceCodeRequestPayload({clientId, scope, audience}) {
  const params = new URLSearchParams();
  params.set('client_id', clientId);
  if (scope && scope.trim() !== '') {
    params.set('scope', scope.trim());
  }
  if (audience && audience.trim() !== '') {
    params.set('audience', audience.trim());
  }
  return params;
}

export function buildTokenRequestPayload({clientId, deviceCode}) {
  const params = new URLSearchParams();
  params.set('grant_type', DEVICE_GRANT_TYPE);
  params.set('device_code', deviceCode);
  params.set('client_id', clientId);
  return params;
}

export function normaliseIntervalMs(intervalSeconds, fallbackMs) {
  const parsed = Number(intervalSeconds);
  if (Number.isFinite(parsed) && parsed > 0) {
    return Math.max(1000, Math.round(parsed * 1000));
  }
  return Math.max(1000, fallbackMs);
}

export function computeExpiryTimestamp(expiresInSeconds, fallbackSeconds, now = Date.now()) {
  const seconds = normalisePositiveSeconds(expiresInSeconds, fallbackSeconds);
  return now + seconds * 1000;
}

export function formatDuration(seconds) {
  const total = Math.max(0, Math.floor(Number(seconds) || 0));
  const minutes = Math.floor(total / 60);
  const remainder = total % 60;
  if (minutes > 0) {
    return `${minutes}m ${remainder}s`;
  }
  return `${remainder}s`;
}
