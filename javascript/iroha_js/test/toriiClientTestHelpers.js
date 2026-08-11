import fs from "node:fs/promises";

export async function fileExists(filePath) {
  try {
    await fs.access(filePath);
    return true;
  } catch {
    return false;
  }
}

export function createSseResponse(chunks) {
  const body = {
    async *[Symbol.asyncIterator]() {
      const encoder = new TextEncoder();
      for (const chunk of chunks) {
        yield encoder.encode(chunk);
      }
    },
  };
  return {
    status: 200,
    headers: {
      get(name) {
        return name.toLowerCase() === "content-type" ? "text/event-stream" : null;
      },
    },
    body,
  };
}

export async function withEnv(overrides, fn) {
  const original = {};
  for (const [key, value] of Object.entries(overrides)) {
    original[key] = process.env[key];
    if (value === null || value === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = value;
    }
  }
  try {
    await fn();
  } finally {
    for (const [key, value] of Object.entries(original)) {
      if (value === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = value;
      }
    }
  }
}
