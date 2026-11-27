// Credential-free transport of opaque Parliament ballot frames and whole public records.
const route = "/v1/gov/anonymous-ballots";
const media = "application/x-norito";

function selector(value, size) {
  if (typeof value === "string" && value.length === 2 * size && /^[0-9a-f]+$/.test(value)) return value;
  if (!(value instanceof Uint8Array) || value.byteLength !== size) throw new TypeError("Invalid ballot record selector");
  return Array.from(value, (byte) => byte.toString(16).padStart(2, "0")).join("");
}

async function boundedBody(response, maximum) {
  const length = response.headers.get("content-length");
  if ((length !== null && (!/^[0-9]{1,19}$/.test(length) || Number(length) > maximum)) ||
      (response.headers.get("content-encoding") ?? "identity").toLowerCase() !== "identity") {
    await response.body?.cancel();
    throw new Error("Invalid anonymous ballot response length or encoding");
  }
  const chunks = [];
  let size = 0;
  if (response.body) {
    const reader = response.body.getReader();
    try {
      while (true) {
        const { done, value } = await reader.read();
        if (done) break;
        size += value.byteLength;
        if (size > maximum) {
          await reader.cancel();
          throw new Error("Anonymous ballot response exceeds its bound");
        }
        chunks.push(value);
      }
    } finally {
      reader.releaseLock();
    }
  }
  if (length !== null && size !== Number(length)) throw new Error("Truncated anonymous ballot response");
  const bytes = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
  return bytes;
}

/** HTTP refusal with an SDK-authored message and optional stable Torii code. */
export class AnonymousBallotRefusal extends Error {
  constructor(status, code, retryAfter) {
    super(`Anonymous ballot request refused (HTTP ${status})`);
    this.status = status;
    this.code = typeof code === "string" && /^[A-Za-z0-9_.:-]{1,128}$/.test(code) ? code : null;
    this.retryAfterSeconds = /^[0-9]{1,9}$/.test(retryAfter ?? "") ? Number(retryAfter) : null;
  }
}

/** Independent HTTP transport for wallet-produced anonymous ballots. */
export class AnonymousBallotClientV1 {
  #base;

  constructor(baseUrl, { allowLocalHttp = false } = {}) {
    const url = new URL(baseUrl);
    const local = /^(localhost|\[::1\]|127\.\d+\.\d+\.\d+|10\.\d+\.\d+\.\d+|192\.168\.\d+\.\d+|172\.(1[6-9]|2\d|3[01])\.\d+\.\d+)$/.test(url.hostname);
    if (url.username || url.password || url.href.includes("?") || url.href.includes("#") ||
        (url.protocol !== "https:" && !(allowLocalHttp && local && url.protocol === "http:"))) {
      throw new TypeError("Invalid anonymous ballot Torii base URL");
    }
    this.#base = url.href.replace(/\/+$/, "") + route;
  }

  /** An empty 202 means the node handled the frame. Confirm inclusion in the public board. */
  async submitAnonymousBallot(envelope, signal) {
    if (!(envelope instanceof Uint8Array) || !envelope.byteLength || envelope.byteLength > 262144) {
      throw new TypeError("Expected a ballot frame of 1 to 262144 bytes");
    }
    await this.#request("", new Uint8Array(envelope), 0, signal);
  }

  getAnonymousBallotBoard(scope, signal) {
    return this.#request("/boards/" + selector(scope, 64), null, 1048576, signal);
  }

  getAnonymousParliamentAttempt(attempt, signal) {
    return this.#request("/attempts/" + selector(attempt, 32), null, 16777216, signal);
  }

  async #request(path, body, limit, signal) {
    const url = this.#base + path;
    const response = await fetch(url, {
      method: body === null ? "GET" : "POST",
      headers: body === null ? { Accept: media } : { "Content-Type": media },
      body, credentials: "omit", redirect: "error", referrerPolicy: "no-referrer", cache: "no-store", signal,
    });
    if (response.redirected || response.url !== url) {
      await response.body?.cancel();
      throw new Error("Anonymous ballot response came from a different URL");
    }
    if (!response.ok) {
      let code;
      try {
        const bytes = await boundedBody(response, 65536);
        if (response.headers.get("content-type")?.split(";")[0].trim().toLowerCase() === "application/json") {
          code = JSON.parse(new TextDecoder().decode(bytes)).code;
        }
      } catch {
        if (signal?.aborted) throw signal.reason;
      }
      throw new AnonymousBallotRefusal(response.status, code, response.headers.get("retry-after"));
    }
    if (response.status !== (body === null ? 200 : 202) ||
        (body === null && response.headers.get("content-type")?.toLowerCase() !== media)) {
      await response.body?.cancel();
      throw new Error("Unexpected anonymous ballot response status or media type");
    }
    const bytes = await boundedBody(response, limit);
    if (body === null && !bytes.length) throw new Error("Anonymous ballot record is empty");
    return bytes;
  }
}
