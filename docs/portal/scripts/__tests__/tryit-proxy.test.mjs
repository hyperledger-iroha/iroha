import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdtemp, writeFile} from 'node:fs/promises';
import {once} from 'node:events';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {Readable, Writable} from 'node:stream';
import test from 'node:test';

import {
  createMetricsCollector,
  createMetricsServer,
  createProxyContext,
  createRateLimiter,
  parseListenAddress,
  parseOrigins,
  proxyRequest,
  verifySpecDigest,
} from '../tryit-proxy-lib.mjs';
import {signPayload} from './helpers/openapi-signing.mjs';

test('parseListenAddress supports host:port and port only', () => {
  assert.deepEqual(parseListenAddress('127.0.0.1:8080'), {host: '127.0.0.1', port: 8080});
  assert.deepEqual(parseListenAddress('8081'), {host: '0.0.0.0', port: 8081});
  assert.deepEqual(parseListenAddress('[::1]:9000'), {host: '::1', port: 9000});
});

test('parseOrigins splits comma separated list', () => {
  const origins = parseOrigins('https://example.com/, http://localhost:3000');
  assert.equal(origins.size, 2);
  assert(origins.has('https://example.com'));
  assert(origins.has('http://localhost:3000'));
});

test('parseOrigins rejects invalid entries', () => {
  assert.throws(
    () => parseOrigins('example.com'),
    /invalid origin "example.com"/,
  );
  assert.throws(
    () => parseOrigins('ftp://example.com'),
    /origin must use http or https/,
  );
  assert.throws(
    () => parseOrigins(new Set([42])),
    /allowed origins must be strings/,
  );
  assert.throws(
    () => parseOrigins(['']),
    /allowed origin entries must be non-empty strings/,
  );
});

test('createProxyContext normalises allowed origins and enforces container types', () => {
  const context = createProxyContext({
    target: 'https://torii.example',
    allowedOrigins: [' https://Example.com/', 'https://example.com'],
    rateLimiter: createRateLimiter({max: 1, windowMs: 1000}),
    fetchImpl: async () => new Response('ok', {status: 200}),
  });
  assert.equal(context.allowedOrigins.size, 1);
  assert(context.allowedOrigins.has('https://example.com'));

  assert.throws(
    () =>
      createProxyContext({
        target: 'https://torii.example',
        allowedOrigins: 'https://example.com',
        rateLimiter: createRateLimiter({max: 1, windowMs: 1000}),
        fetchImpl: async () => new Response('ok', {status: 200}),
      }),
    /allowedOrigins must be a Set or array of origin strings/,
  );
});

test('metrics collector validates bucket configuration', () => {
  assert.throws(
    () => createMetricsCollector({durationBucketsMs: []}),
    /durationBucketsMs must contain at least one bucket boundary/,
  );
});

test('proxy forwards requests and injects bearer token', async () => {
  const calls = [];
  const context = createProxyContext({
    target: 'https://torii.example',
    allowedOrigins: new Set(['http://localhost:3000']),
    defaultBearer: 'fallback',
    rateLimiter: createRateLimiter({max: 10, windowMs: 60_000}),
    fetchImpl: async (url, options) => {
      calls.push({url: url.toString(), options});
      const bodyText = options.body ? options.body.toString('utf8') : '';
      return new Response(JSON.stringify({received: JSON.parse(bodyText)}), {
        status: 200,
        headers: {'content-type': 'application/json', 'x-request-id': 'abc123'},
      });
    },
  });

  const req = createMockRequest({
    method: 'POST',
    url: '/proxy/blocks?limit=1',
    headers: {
      'content-type': 'application/json',
      origin: 'http://localhost:3000',
      'x-tryit-auth': 'test-override',
    },
    body: JSON.stringify({foo: 'bar'}),
  });
  const res = createMockResponse();

  await proxyRequest(context, req, res);
  await waitForFinish(res);

  assert.equal(res.statusCode, 200);
  assert.equal(res.getHeader('x-request-id'), 'abc123');
  assert.deepEqual(JSON.parse(res.bodyString()), {received: {foo: 'bar'}});
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, 'https://torii.example/blocks?limit=1');
  assert.equal(calls[0].options.headers.get('authorization'), 'Bearer test-override');
});

test('proxy preserves Norito binary payloads and Content-Type', async () => {
  const calls = [];
  const payload = Buffer.from([0xde, 0xad, 0xbe, 0xef]);
  const context = createProxyContext({
    target: 'https://torii.example',
    allowedOrigins: new Set(['http://localhost:3000']),
    rateLimiter: createRateLimiter({max: 10, windowMs: 60_000}),
    fetchImpl: async (url, options) => {
      calls.push({url: url.toString(), options});
      return new Response('ok', {
        status: 202,
        headers: {'content-type': 'application/json', 'x-request-id': 'nrpc-1'},
      });
    },
  });

  const req = createMockRequest({
    method: 'POST',
    url: '/proxy/v2/pipeline/submit',
    headers: {
      origin: 'http://localhost:3000',
      'content-type': 'application/x-norito',
    },
    body: payload,
  });
  const res = createMockResponse();

  await proxyRequest(context, req, res);
  await waitForFinish(res);

  assert.equal(res.statusCode, 202);
  assert.equal(res.getHeader('x-request-id'), 'nrpc-1');
  assert.equal(calls.length, 1);
  assert.equal(calls[0].url, 'https://torii.example/v2/pipeline/submit');
  assert.equal(calls[0].options.headers.get('content-type'), 'application/x-norito');
  assert(Buffer.isBuffer(calls[0].options.body), 'body must be forwarded as Buffer');
  assert.deepEqual(calls[0].options.body, payload);
});

test('proxy tags requests with a default X-TryIt-Client header', async () => {
  const calls = [];
  const context = createProxyContext({
    target: 'https://torii.example',
    allowedOrigins: new Set(['http://localhost:3000']),
    clientId: 'docs-portal-default',
    rateLimiter: createRateLimiter({max: 5, windowMs: 60_000}),
    fetchImpl: async (url, options) => {
      calls.push({url: url.toString(), client: options.headers.get('x-tryit-client')});
      return new Response('ok', {status: 200});
    },
  });

  const req = createMockRequest({
    method: 'GET',
    url: '/proxy/status',
    headers: {origin: 'http://localhost:3000'},
  });
  const res = createMockResponse();

  await proxyRequest(context, req, res);
  await waitForFinish(res);

  assert.equal(res.statusCode, 200);
  assert.equal(calls.length, 1);
  assert.equal(calls[0].client, 'docs-portal-default');
});

test('proxy applies caller-supplied X-TryIt-Client tags and rejects unsafe values', async () => {
  const calls = [];
  const context = createProxyContext({
    target: 'https://torii.example',
    allowedOrigins: new Set(['http://localhost:3000']),
    clientId: 'docs-portal',
    rateLimiter: createRateLimiter({max: 5, windowMs: 60_000}),
    fetchImpl: async (url, options) => {
      calls.push({url: url.toString(), client: options.headers.get('x-tryit-client')});
      return new Response('ok', {status: 200});
    },
  });

  const req = createMockRequest({
    method: 'GET',
    url: '/proxy/status',
    headers: {
      origin: 'http://localhost:3000',
      'x-tryit-client': '  docs-portal-rapidoc  ',
    },
  });
  const res = createMockResponse();
  await proxyRequest(context, req, res);
  await waitForFinish(res);
  assert.equal(res.statusCode, 200);
  assert.equal(calls[0].client, 'docs-portal-rapidoc');

  const badReq = createMockRequest({
    method: 'GET',
    url: '/proxy/status',
    headers: {
      origin: 'http://localhost:3000',
      'x-tryit-client': 'malicious\nheader',
    },
  });
  const badRes = createMockResponse();
  await proxyRequest(context, badReq, badRes);
  await waitForFinish(badRes);
  assert.equal(badRes.statusCode, 400);
  assert.deepEqual(JSON.parse(badRes.bodyString()), {error: 'bad_request'});
});

test('rate limiter returns 429 after threshold', async () => {
  let fetchCount = 0;
  const context = createProxyContext({
    target: 'https://torii.example',
    allowedOrigins: new Set(['http://localhost:3000']),
    rateLimiter: createRateLimiter({max: 1, windowMs: 60_000}),
    fetchImpl: async () => {
      fetchCount += 1;
      return new Response('ok', {status: 200});
    },
  });

  const firstReq = createMockRequest({
    method: 'GET',
    url: '/proxy/status',
    headers: {origin: 'http://localhost:3000'},
  });
  const firstRes = createMockResponse();
  await proxyRequest(context, firstReq, firstRes);
  await waitForFinish(firstRes);
  assert.equal(firstRes.statusCode, 200);
  assert.equal(fetchCount, 1);

  const secondReq = createMockRequest({
    method: 'GET',
    url: '/proxy/status',
    headers: {origin: 'http://localhost:3000'},
  });
  const secondRes = createMockResponse();
  await proxyRequest(context, secondReq, secondRes);
  await waitForFinish(secondRes);
  assert.equal(secondRes.statusCode, 429);
  assert.equal(fetchCount, 1);
  assert.equal(JSON.parse(secondRes.bodyString()).error, 'rate_limited');
});

test('proxy enforces max body size and records telemetry', async () => {
  const collector = createMetricsCollector({
    durationBucketsMs: [10, 50],
  });
  const context = createProxyContext({
    target: 'https://torii.example',
    allowedOrigins: new Set(['http://localhost:3000']),
    maxBodyBytes: 4,
    metrics: collector,
    rateLimiter: createRateLimiter({max: 10, windowMs: 60_000}),
    fetchImpl: async () => {
      throw new Error('request should be rejected before reaching upstream');
    },
  });

  const req = createMockRequest({
    method: 'POST',
    url: '/proxy/v2/pipeline/submit',
    headers: {
      origin: 'http://localhost:3000',
      'content-type': 'application/json',
    },
    body: '0123456789',
  });
  const res = createMockResponse();

  await proxyRequest(context, req, res);
  await waitForFinish(res);

  assert.equal(res.statusCode, 413);
  assert.equal(JSON.parse(res.bodyString()).error, 'request entity too large');
  const metrics = collector.formatPrometheus();
  assert.match(metrics, /tryit_proxy_body_too_large_total 1/);
  assert.match(metrics, /tryit_proxy_outcomes_total\{outcome="body_too_large"\} 1/);
});

test('preflight advertises telemetry trace headers', async () => {
  const context = createProxyContext({
    target: 'https://torii.example',
    allowedOrigins: new Set(['http://localhost:3000']),
    rateLimiter: createRateLimiter({max: 1, windowMs: 60_000}),
    fetchImpl: async () => new Response('ok', {status: 200}),
  });

  const req = createMockRequest({
    method: 'OPTIONS',
    url: '/proxy/status',
    headers: {origin: 'http://localhost:3000'},
  });
  const res = createMockResponse();

  await proxyRequest(context, req, res);
  await waitForFinish(res);

  assert.equal(res.statusCode, 204);
  const allowed = res.getHeader('access-control-allow-headers');
  assert.ok(allowed, 'expected access-control-allow-headers');
  const headerSet = new Set(
    allowed
      .split(',')
      .map((value) => value.trim())
      .filter(Boolean),
  );
  for (const header of [
    'Content-Type',
    'Authorization',
    'X-TryIt-Auth',
    'X-Request-ID',
    'X-Sora-Request-ID',
    'X-Sora-Trace-Id',
    'X-Torii-Request-ID',
  ]) {
    assert(headerSet.has(header), `missing ${header} in Access-Control-Allow-Headers`);
  }
});

test('proxy rejects headers with control characters to prevent injection', async () => {
  let fetchCount = 0;
  const context = createProxyContext({
    target: 'https://torii.example',
    allowedOrigins: new Set(['http://localhost:3000']),
    rateLimiter: createRateLimiter({max: 10, windowMs: 60_000}),
    fetchImpl: async () => {
      fetchCount += 1;
      return new Response('ok', {status: 200});
    },
  });

  const req = createMockRequest({
    method: 'GET',
    url: '/proxy/status',
    headers: {
      origin: 'http://localhost:3000',
      'x-request-id': 'abc\r\nX-Evil: 1',
    },
  });
  const res = createMockResponse();

  await proxyRequest(context, req, res);
  await waitForFinish(res);

  assert.equal(res.statusCode, 400);
  assert.equal(fetchCount, 0);
  assert.equal(JSON.parse(res.bodyString()).error, 'bad_request');
});

test('logs request metadata without leaking bearer tokens', async () => {
  const logger = createCaptureLogger();
  const context = createProxyContext({
    target: 'https://torii.example',
    allowedOrigins: new Set(['http://localhost:3000']),
    defaultBearer: 'fallback',
    rateLimiter: createRateLimiter({max: 10, windowMs: 60_000}),
    fetchImpl: async () => new Response('ok', {status: 200}),
    logger,
  });

  const req = createMockRequest({
    method: 'GET',
    url: '/proxy/status',
    headers: {
      origin: 'http://localhost:3000',
      'x-tryit-auth': 'secret-token',
      'x-request-id': 'req-123',
    },
  });
  const res = createMockResponse();

  await proxyRequest(context, req, res);
  await waitForFinish(res);

  const successEntry = logger.entries.find(
    (entry) => entry.level === 'info' && entry.args[1] === 'forward_success',
  );
  assert(successEntry, 'expected forward_success log entry');
  const payload = successEntry.args[2];
  assert.equal(payload.requestId, 'req-123');
  assert.equal(payload.authSource, 'override');
  assert(!JSON.stringify(logger.entries).includes('secret-token'));
});

test('verifySpecDigest succeeds when manifest matches', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'tryit-proxy-test-'));
  const specPath = path.join(dir, 'torii.json');
  const manifestPath = path.join(dir, 'manifest.json');
  const body = Buffer.from(JSON.stringify({openapi: '3.1.0'}), 'utf8');
  await writeFile(specPath, body);
  const digest = createHash('sha256').update(body).digest('hex');
  const signature = signPayload(body);
  await writeFile(
    manifestPath,
    JSON.stringify({
      artifact: {
        sha256_hex: digest,
        bytes: Buffer.byteLength(body),
        signature: {
          algorithm: 'ed25519',
          public_key_hex: signature.publicKeyHex,
          signature_hex: signature.signatureHex,
        },
      },
    }),
  );

  const result = await verifySpecDigest({specPath, manifestPath});
  assert.equal(result.actual, digest);
  assert.equal(result.bytes, Buffer.byteLength(body));
});

test('verifySpecDigest rejects mismatched digests', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'tryit-proxy-test-'));
  const specPath = path.join(dir, 'torii.json');
  const manifestPath = path.join(dir, 'manifest.json');
  const body = Buffer.from('{"openapi":"3.1.0","info":{"title":"demo"}}', 'utf8');
  await writeFile(specPath, body);
  const signature = signPayload(body);
  await writeFile(
    manifestPath,
    JSON.stringify({
      artifact: {
        sha256_hex: 'deadbeef',
        bytes: Buffer.byteLength(body),
        signature: {
          algorithm: 'ed25519',
          public_key_hex: signature.publicKeyHex,
          signature_hex: signature.signatureHex,
        },
      },
    }),
  );

  await assert.rejects(
    verifySpecDigest({specPath, manifestPath}),
    /OpenAPI spec digest mismatch/,
  );
});

test('verifySpecDigest rejects missing signatures', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'tryit-proxy-test-nosig-'));
  const specPath = path.join(dir, 'torii.json');
  const manifestPath = path.join(dir, 'manifest.json');
  const body = Buffer.from('{"openapi":"3.1.0"}', 'utf8');
  await writeFile(specPath, body);
  const digest = createHash('sha256').update(body).digest('hex');
  await writeFile(
    manifestPath,
    JSON.stringify({
      artifact: {
        sha256_hex: digest,
        bytes: body.length,
      },
    }),
  );

  await assert.rejects(
    verifySpecDigest({specPath, manifestPath}),
    /missing artifact\.signature/i,
  );
});

test('verifySpecDigest rejects invalid signatures', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'tryit-proxy-test-badsig-'));
  const specPath = path.join(dir, 'torii.json');
  const manifestPath = path.join(dir, 'manifest.json');
  const body = Buffer.from('{"openapi":"3.1.0","info":{"title":"demo"}}', 'utf8');
  await writeFile(specPath, body);
  const digest = createHash('sha256').update(body).digest('hex');
  const signature = signPayload(body);
  await writeFile(
    manifestPath,
    JSON.stringify({
      artifact: {
        sha256_hex: digest,
        bytes: body.length,
        signature: {
          algorithm: 'ed25519',
          public_key_hex: signature.publicKeyHex,
          signature_hex: '00'.repeat(signature.signatureHex.length / 2),
        },
      },
    }),
  );

  await assert.rejects(
    verifySpecDigest({specPath, manifestPath}),
    /signature verification failed/i,
  );
});

test('metrics collector records outcomes and metrics server exposes metrics', async () => {
  let currentTime = 0;
  const collector = createMetricsCollector({
    nowFn: () => currentTime,
    durationBucketsMs: [50, 100, 250],
  });
  currentTime = 10;
  const tracker = collector.trackRequest('GET');
  currentTime = 70;
  tracker.end(200, 'success');
  currentTime = 80;
  const tracker2 = collector.trackRequest('POST');
  currentTime = 400;
  collector.recordRateLimit(tracker2);
  currentTime = 500;
  const tracker3 = collector.trackRequest('POST');
  currentTime = 520;
  collector.recordBodyLimit(tracker3);
  const text = collector.formatPrometheus();
  assert.match(text, /tryit_proxy_requests_total\{method="GET"\} 1/);
  assert.match(text, /tryit_proxy_rate_limited_total 1/);
  assert.match(text, /tryit_proxy_body_too_large_total 1/);
  assert.match(text, /tryit_proxy_request_duration_ms_bucket\{le="50"\} 1/);
  assert.match(text, /tryit_proxy_request_duration_ms_bucket\{le="100"\} 2/);
  assert.match(text, /tryit_proxy_request_duration_ms_bucket\{le="250"\} 2/);
  assert.match(text, /tryit_proxy_request_duration_ms_bucket\{le="\+Inf"\} 3/);
  const metricsServer = createMetricsServer({collector, path: '/metrics-test'});
  const mockRes = createMockResponse();
  const mockReq = {
    method: 'GET',
    url: '/metrics-test',
    headers: {},
    socket: {destroy() {}},
  };
  metricsServer.emit('request', mockReq, mockRes);
  await waitForFinish(mockRes);
  assert.equal(mockRes.statusCode, 200);
  assert.match(mockRes.bodyString(), /tryit_proxy_outcomes_total/);
  metricsServer.close();
});

function createMockRequest({method, url, headers = {}, body}) {
  const normalisedHeaders = {};
  for (const [key, value] of Object.entries(headers)) {
    normalisedHeaders[key.toLowerCase()] = value;
  }

  const source = body == null ? null : Buffer.from(body);
  let pushed = false;

  class MockRequest extends Readable {
    constructor() {
      super();
      this.method = method;
      this.url = url;
      this.headers = normalisedHeaders;
      this.socket = {remoteAddress: '127.0.0.1'};
    }

    _read() {
      if (pushed) {
        this.push(null);
        return;
      }
      pushed = true;
      if (source) {
        this.push(source);
      }
      this.push(null);
    }
  }

  return new MockRequest();
}

function createMockResponse() {
  class MockResponse extends Writable {
    constructor() {
      super();
      this.statusCode = 200;
      this.statusMessage = 'OK';
      this.headers = {};
      this._chunks = [];
    }

    setHeader(name, value) {
      this.headers[name.toLowerCase()] = value;
    }

    getHeader(name) {
      return this.headers[name.toLowerCase()];
    }

    _write(chunk, _encoding, callback) {
      this._chunks.push(Buffer.from(chunk));
      callback();
    }

    end(chunk, encoding, callback) {
      if (typeof chunk === 'function') {
        callback = chunk;
        chunk = undefined;
      } else if (typeof encoding === 'function') {
        callback = encoding;
      }
      if (chunk) {
        this._chunks.push(Buffer.from(chunk));
      }
      super.end(callback);
    }

    bodyString() {
      return Buffer.concat(this._chunks).toString('utf8');
    }
  }

  return new MockResponse();
}

async function waitForFinish(stream) {
  if (stream.writableEnded) {
    return;
  }
  await once(stream, 'finish');
}

function createCaptureLogger() {
  const entries = [];
  return {
    entries,
    info: (...args) => entries.push({level: 'info', args}),
    warn: (...args) => entries.push({level: 'warn', args}),
    error: (...args) => entries.push({level: 'error', args}),
  };
}
