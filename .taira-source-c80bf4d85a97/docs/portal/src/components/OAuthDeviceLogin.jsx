import React, {useEffect, useMemo, useState} from 'react';

import {
  buildDeviceCodeRequestPayload,
  buildTokenRequestPayload,
  computeExpiryTimestamp,
  formatDuration,
  normaliseIntervalMs,
} from '../utils/deviceAuth';

const STATUS_IDLE = 'idle';
const STATUS_PENDING = 'pending';
const STATUS_POLLING = 'polling';
const STATUS_ERROR = 'error';
const STATUS_SUCCESS = 'success';

function hasOAuthConfig(config) {
  return Boolean(config.deviceCodeUrl && config.tokenUrl && config.clientId);
}

export default function OAuthDeviceLogin({config, tokenValue, onTokenChange}) {
  const isConfigured = hasOAuthConfig(config);
  const defaultPollMs = config.pollIntervalMs ?? 5000;
  const deviceCodeFallbackSeconds = config.deviceCodeExpiresSeconds ?? 600;
  const tokenLifetimeSeconds = config.tokenLifetimeSeconds ?? 900;

  const [status, setStatus] = useState(STATUS_IDLE);
  const [pollDelayMs, setPollDelayMs] = useState(defaultPollMs);
  const [session, setSession] = useState(null);
  const [message, setMessage] = useState('');
  const [messageTone, setMessageTone] = useState('info');
  const [deviceCountdown, setDeviceCountdown] = useState(null);
  const [tokenInfo, setTokenInfo] = useState(null);
  const [tokenCountdown, setTokenCountdown] = useState(null);
  const [requestInFlight, setRequestInFlight] = useState(false);

  useEffect(() => {
    if (session == null) {
      setDeviceCountdown(null);
      return undefined;
    }
    if (typeof window === 'undefined') {
      return undefined;
    }
    const updateCountdown = () => {
      const remaining = Math.max(0, Math.floor((session.expiresAt - Date.now()) / 1000));
      setDeviceCountdown(remaining);
      if (remaining === 0) {
        setSession(null);
        setStatus(STATUS_ERROR);
        setMessage('Device code expired. Start again to request a new login code.');
        setMessageTone('danger');
      }
    };
    updateCountdown();
    const interval = window.setInterval(updateCountdown, 1000);
    return () => window.clearInterval(interval);
  }, [session]);

  useEffect(() => {
    if (tokenInfo == null) {
      setTokenCountdown(null);
      return undefined;
    }
    if (typeof window === 'undefined') {
      return undefined;
    }
    const updateCountdown = () => {
      const remaining = Math.max(0, Math.floor((tokenInfo.expiresAt - Date.now()) / 1000));
      setTokenCountdown(remaining);
      if (remaining === 0) {
        setTokenInfo(null);
        setMessage('Access token expired. Sign in again to mint a fresh token.');
        setMessageTone('warning');
        onTokenChange('');
      }
    };
    updateCountdown();
    const interval = window.setInterval(updateCountdown, 1000);
    return () => window.clearInterval(interval);
  }, [tokenInfo, onTokenChange]);

  useEffect(() => {
    if (!session) {
      return undefined;
    }
    if (status !== STATUS_PENDING && status !== STATUS_POLLING) {
      return undefined;
    }
    if (typeof window === 'undefined') {
      return undefined;
    }

    let cancelled = false;
    const timer = window.setTimeout(async () => {
      if (cancelled) {
        return;
      }
      setStatus(STATUS_POLLING);
      try {
        const response = await fetch(config.tokenUrl, {
          method: 'POST',
          headers: {'Content-Type': 'application/x-www-form-urlencoded'},
          body: buildTokenRequestPayload({
            clientId: config.clientId,
            deviceCode: session.deviceCode,
          }),
        });
        const payload = await response.json();
        if (cancelled) {
          return;
        }
        if (response.ok && payload.access_token) {
          const expiresAt = computeExpiryTimestamp(
            payload.expires_in,
            tokenLifetimeSeconds,
          );
          setTokenInfo({expiresAt, scope: payload.scope ?? config.scope});
          onTokenChange(payload.access_token);
          setStatus(STATUS_SUCCESS);
          setMessage('Access token issued for the Try it console.');
          setMessageTone('success');
          setSession(null);
          return;
        }

        handleTokenError(payload.error);
      } catch (error) {
        if (!cancelled) {
          setStatus(STATUS_ERROR);
          setSession(null);
          setMessage(
            `Failed to poll the OAuth token endpoint: ${
              error instanceof Error ? error.message : String(error)
            }`,
          );
          setMessageTone('danger');
          onTokenChange('');
        }
      }
    }, pollDelayMs);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [session, status, pollDelayMs, config.tokenUrl, config.clientId, onTokenChange]);

  const activeStatus = useMemo(() => {
    if (!isConfigured) {
      return 'OAuth disabled for this build.';
    }
    switch (status) {
      case STATUS_PENDING:
        return 'Awaiting approval…';
      case STATUS_POLLING:
        return 'Polling for token…';
      case STATUS_SUCCESS:
        return 'Signed in';
      case STATUS_ERROR:
        return 'OAuth error';
      default:
        return 'Signed out';
    }
  }, [status, isConfigured]);

  if (!isConfigured) {
    return null;
  }

  function handleTokenError(code) {
    switch (code) {
      case 'authorization_pending':
        setStatus(STATUS_PENDING);
        break;
      case 'slow_down':
        setPollDelayMs((prev) => prev + 1000);
        setStatus(STATUS_PENDING);
        break;
      case 'expired_token':
        setStatus(STATUS_ERROR);
        setSession(null);
        setMessage('The device code expired before approval. Start again to request a new code.');
        setMessageTone('danger');
        onTokenChange('');
        break;
      case 'access_denied':
        setStatus(STATUS_ERROR);
        setSession(null);
        setMessage('Access was denied during approval. The Try it console remains signed out.');
        setMessageTone('warning');
        onTokenChange('');
        break;
      default:
        setStatus(STATUS_ERROR);
        setSession(null);
        setMessage(`OAuth server reported an error: ${code ?? 'unknown_error'}.`);
        setMessageTone('danger');
        onTokenChange('');
        break;
    }
  }

  async function startDeviceFlow() {
    setRequestInFlight(true);
    setMessage('');
    try {
      const response = await fetch(config.deviceCodeUrl, {
        method: 'POST',
        headers: {'Content-Type': 'application/x-www-form-urlencoded'},
        body: buildDeviceCodeRequestPayload({
          clientId: config.clientId,
          scope: config.scope,
          audience: config.audience,
        }),
      });
      if (!response.ok) {
        throw new Error(`device code request failed (${response.status})`);
      }
      const payload = await response.json();
      const expiresAt = computeExpiryTimestamp(
        payload.expires_in,
        deviceCodeFallbackSeconds,
      );
      setSession({
        deviceCode: payload.device_code,
        userCode: payload.user_code,
        verificationUri: payload.verification_uri,
        verificationUriComplete: payload.verification_uri_complete,
        expiresAt,
      });
      setPollDelayMs(
        normaliseIntervalMs(payload.interval, defaultPollMs),
      );
      setStatus(STATUS_PENDING);
      setMessage('Enter the displayed code within the expiry window to mint a short-lived token.');
      setMessageTone('info');
    } catch (error) {
      setStatus(STATUS_ERROR);
      setMessage(
        `Unable to start the OAuth device flow: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
      setMessageTone('danger');
    } finally {
      setRequestInFlight(false);
    }
  }

  function cancelFlow() {
    setSession(null);
    setStatus(STATUS_IDLE);
    setMessage('Device flow cancelled.');
    setMessageTone('warning');
  }

  function clearToken() {
    setTokenInfo(null);
    setStatus(STATUS_IDLE);
    setMessage('Signed out of the Try it console.');
    setMessageTone('info');
    onTokenChange('');
  }

  const approvalTarget =
    session && (session.verificationUriComplete ?? session.verificationUri ?? '#');
  const approvalLabel =
    session && (session.verificationUri ?? session.verificationUriComplete ?? 'device portal');

  return (
    <div className="tryItOAuth margin-bottom--lg">
      <div className="tryItOAuth__header">
        <div>
          <h4 className="tryItOAuth__title">OAuth sign-in</h4>
          <p className="tryItOAuth__subtitle">
            Device-code login mints a short-lived token so you can call Torii without sharing static
            credentials.
          </p>
        </div>
        <span className="badge badge--secondary">{activeStatus}</span>
      </div>
      {message && (
        <div className={`alert alert--${messageTone} margin-bottom--md`} role="alert">
          {message}
        </div>
      )}
      {session ? (
        <div className="tryItOAuth__session">
          <p className="tryItOAuth__codeLabel">Use this code:</p>
          <div className="tryItOAuth__codeRow">
            <span className="tryItOAuth__code">{session.userCode}</span>
            <button
              type="button"
              className="button button--sm button--secondary"
              onClick={() => navigator.clipboard?.writeText(session.userCode)}
            >
              Copy
            </button>
          </div>
          <p className="margin-top--sm">
            Visit{' '}
            <a href={approvalTarget} target="_blank" rel="noreferrer">
              {approvalLabel}
            </a>{' '}
            to approve access. Expires in <strong>{formatDuration(deviceCountdown ?? 0)}</strong>.
          </p>
          <div className="tryItOAuth__actions">
            <button
              type="button"
              className="button button--sm button--primary"
              disabled={requestInFlight}
              onClick={startDeviceFlow}
            >
              Refresh code
            </button>
            <button
              type="button"
              className="button button--sm button--secondary"
              onClick={cancelFlow}
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <div className="tryItOAuth__actions">
          <button
            type="button"
            className="button button--primary button--sm"
            onClick={startDeviceFlow}
            disabled={requestInFlight}
          >
            {requestInFlight ? 'Requesting…' : 'Sign in with device code'}
          </button>
          <button
            type="button"
            className="button button--secondary button--sm"
            onClick={clearToken}
            disabled={!tokenValue && !tokenInfo}
          >
            Sign out
          </button>
        </div>
      )}
      {tokenInfo && (
        <p className="tryItOAuth__tokenExpiry">
          Token expires in <strong>{formatDuration(tokenCountdown ?? 0)}</strong>. The field below is
          auto-populated for the Try it console.
        </p>
      )}
    </div>
  );
}
