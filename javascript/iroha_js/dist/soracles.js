const buildUrl = (baseUrl, path, params = {}) => {
  const url = new URL(path, `${baseUrl}/`);
  for (const [key, value] of Object.entries(params)) {
    if (value !== undefined && value !== null && value !== "") {
      url.searchParams.set(key, String(value));
    }
  }
  return url.toString();
};

const fetchJson = async (baseUrl, path, params) => {
  const response = await fetch(buildUrl(baseUrl, path, params), {
    headers: { Accept: "application/json" },
  });
  if (!response.ok) {
    const message = await response.text().catch(() => "");
    throw new Error(`${response.status} ${response.statusText}: ${message || "request failed"}`);
  }
  return response.json();
};

export const queryOracleFeeds = (baseUrl, options = {}) =>
  fetchJson(baseUrl, "/v1/soracles/feeds", options);

export const queryOracleFeedHistory = (baseUrl, feedId, options = {}) => {
  if (!feedId || typeof feedId !== "string") {
    throw new TypeError("feedId must be a non-empty string");
  }
  return fetchJson(baseUrl, `/v1/soracles/feeds/${encodeURIComponent(feedId)}/history`, options);
};

export const getLatestDefiOracleAttestation = ({
  baseUrl,
  toriiUrl,
  domain,
  subjectId,
  status = 0,
}) => {
  const resolvedBaseUrl = baseUrl || toriiUrl;
  if (!resolvedBaseUrl) throw new TypeError("baseUrl or toriiUrl is required");
  if (domain === undefined || domain === null || domain === "") {
    throw new TypeError("domain is required");
  }
  if (subjectId === undefined || subjectId === null || subjectId === "") {
    throw new TypeError("subjectId is required");
  }
  return fetchJson(resolvedBaseUrl, "/v1/soracles/defi/attestations/latest", {
    domain,
    subject_id: subjectId,
    status,
  });
};
