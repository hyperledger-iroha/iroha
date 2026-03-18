import {useEffect, useMemo, useState} from 'react';

const LATEST_LABEL = 'latest';
const VERSIONS_INDEX_URL = '/openapi/versions.json';

function normaliseVersions(list) {
  return Array.from(new Set(list ?? [])).filter(Boolean).sort();
}

function normaliseEntries(entries) {
  const map = new Map();
  for (const entry of entries ?? []) {
    if (!entry || typeof entry.label !== 'string') {
      continue;
    }
    map.set(entry.label, entry);
  }
  return map;
}

/**
 * Resolve the OpenAPI spec URL for a given version label.
 *
 * The `latest` alias (default) points to `/openapi/torii.json`. Other values
 * map to `/openapi/versions/<version>/torii.json`.
 *
 * @param {string} version
 * @returns {string}
 */
export function specUrlForVersion(version) {
  if (!version || version === LATEST_LABEL) {
    return '/openapi/torii.json';
  }
  return `/openapi/versions/${encodeURIComponent(version)}/torii.json`;
}

/**
 * Fetch available OpenAPI versions exposed under `static/openapi/versions.json`.
 *
 * Returns a sorted list plus loading/error flags so callers can decide how to
 * surface fallbacks.
 */
export function useOpenApiVersions(options = {}) {
  const {enabled = true} = options;
  const [versions, setVersions] = useState([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState(null);
  const [generatedAt, setGeneratedAt] = useState(null);
  const [metadataByVersion, setMetadataByVersion] = useState(new Map());

  useEffect(() => {
    let cancelled = false;
    if (!enabled) {
      setVersions([]);
      setLoading(false);
      setError(null);
      setGeneratedAt(null);
      setMetadataByVersion(new Map());
      return undefined;
    }
    async function load() {
      setLoading(true);
      setError(null);
      try {
        const response = await fetch(VERSIONS_INDEX_URL, {cache: 'no-cache'});
        if (!response.ok) {
          throw new Error(`failed to fetch versions index (${response.status})`);
        }
        const data = await response.json();
        if (!cancelled) {
          setVersions(normaliseVersions(data?.versions));
          setGeneratedAt(data?.generatedAt ?? null);
          setMetadataByVersion(normaliseEntries(data?.entries));
        }
      } catch (err) {
        if (!cancelled) {
          setError(err instanceof Error ? err : new Error(String(err)));
          setVersions([]);
          setGeneratedAt(null);
          setMetadataByVersion(new Map());
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }
    load();
    return () => {
      cancelled = true;
    };
  }, [enabled]);

  const withLatest = useMemo(() => {
    return [LATEST_LABEL, ...normaliseVersions(versions)];
  }, [versions]);

  return {
    versions: withLatest,
    loading,
    error,
    hasExplicitVersions: versions.length > 0,
    generatedAt,
    metadataByVersion,
  };
}
