import {useEffect, useRef} from 'react';
import {useLocation} from '@docusaurus/router';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';

import {
  buildAnalyticsPayload,
  normaliseSampleRate,
  shouldSendEvent,
} from '../utils/analytics';

function sendAnalytics(endpoint, payload) {
  if (typeof window === 'undefined' || !endpoint) {
    return;
  }
  const body = JSON.stringify(payload);
  if (typeof navigator !== 'undefined' && navigator.sendBeacon) {
    const blob = new Blob([body], {type: 'application/json'});
    if (navigator.sendBeacon(endpoint, blob)) {
      return;
    }
  }
  fetch(endpoint, {
    method: 'POST',
    headers: {'Content-Type': 'application/json'},
    body,
    keepalive: true,
  }).catch(() => {
    // Suppress network errors: analytics should never break the docs shell.
  });
}

export default function AnalyticsTracker() {
  const {siteConfig, i18n} = useDocusaurusContext();
  const location = useLocation();
  const releaseTag = siteConfig.customFields?.releaseTag ?? 'dev';
  const analyticsConfig = siteConfig.customFields?.analytics ?? {};
  const endpoint = (analyticsConfig.endpoint ?? '').trim();
  const sampleRate = normaliseSampleRate(analyticsConfig.sampleRate);
  const lastSentPath = useRef('');

  useEffect(() => {
    if (!endpoint) {
      return;
    }
    const path = `${location.pathname}${location.search || ''}`;
    if (path === lastSentPath.current) {
      return;
    }
    if (!shouldSendEvent(sampleRate)) {
      lastSentPath.current = path;
      return;
    }
    const payload = buildAnalyticsPayload({
      path,
      locale: i18n?.currentLocale ?? 'en',
      release: releaseTag,
      event: 'pageview',
    });
    sendAnalytics(endpoint, payload);
    lastSentPath.current = path;
  }, [
    endpoint,
    i18n?.currentLocale,
    location.pathname,
    location.search,
    releaseTag,
    sampleRate,
  ]);

  return null;
}
