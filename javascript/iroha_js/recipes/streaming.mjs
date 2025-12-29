#!/usr/bin/env node
/**
 * Streaming helper for `/v1/events/sse`.
 *
 * This recipe demonstrates how to:
 * - subscribe to pipeline transaction events with a deterministic filter,
 * - persist the `Last-Event-ID` cursor so runs resume after restarts,
 * - honour Ctrl+C / SIGTERM via `AbortController`, and
 * - surface pipeline statuses with `extractPipelineStatusKind`.
 *
 * Environment variables:
 * - TORII_URL — Torii endpoint (default: http://127.0.0.1:8080)
 * - TORII_API_TOKEN / TORII_AUTH_TOKEN — optional headers
 * - PIPELINE_STATUS — filter status (default: Committed)
 * - STREAM_FILTER_JSON — override the SSE filter JSON
 * - STREAM_CURSOR_FILE — path for the last event id (default: artifacts/js/torii_stream.cursor)
 * - STREAM_MAX_EVENTS — stop after N events (0 = run indefinitely, default: 10)
 */
import fs from "node:fs/promises";
import path from "node:path";
import process from "node:process";

import { ToriiClient, extractPipelineStatusKind } from "@iroha/iroha-js";

const toriiUrl = process.env.TORII_URL ?? "http://127.0.0.1:8080";
const apiToken = process.env.TORII_API_TOKEN;
const authToken = process.env.TORII_AUTH_TOKEN;
const cursorFile =
  process.env.STREAM_CURSOR_FILE ?? path.join("artifacts", "js", "torii_stream.cursor");
const customFilter = process.env.STREAM_FILTER_JSON;
const statusKind = process.env.PIPELINE_STATUS ?? "Committed";
const maxEventsEnv = process.env.STREAM_MAX_EVENTS ?? "10";

function resolveMaxEvents(value) {
  const parsed = Number.parseInt(String(value ?? "0"), 10);
  if (Number.isNaN(parsed) || parsed < 0) {
    throw new Error(`STREAM_MAX_EVENTS must be a non-negative integer (received ${value}).`);
  }
  return parsed === 0 ? Number.POSITIVE_INFINITY : parsed;
}

function buildFilter() {
  if (customFilter) {
    try {
      const parsed = JSON.parse(customFilter);
      if (parsed == null || typeof parsed !== "object") {
        throw new TypeError("STREAM_FILTER_JSON must decode to an object");
      }
      return parsed;
    } catch (error) {
      throw new Error(
        `failed to parse STREAM_FILTER_JSON: ${
          error instanceof Error ? error.message : String(error)
        }`,
      );
    }
  }
  return {
    Pipeline: {
      Transaction: {
        status: statusKind,
      },
    },
  };
}

async function readCursor(filePath) {
  try {
    const contents = await fs.readFile(filePath, "utf8");
    const trimmed = contents.trim();
    return trimmed.length === 0 ? null : trimmed;
  } catch (error) {
    if (error && typeof error === "object" && "code" in error && error.code === "ENOENT") {
      return null;
    }
    throw error;
  }
}

async function writeCursor(filePath, id) {
  if (!id) {
    return;
  }
  await fs.mkdir(path.dirname(filePath), { recursive: true });
  await fs.writeFile(filePath, `${id}\n`, "utf8");
}

async function main() {
  const maxEvents = resolveMaxEvents(maxEventsEnv);
  const torii = new ToriiClient(toriiUrl, {
    apiToken,
    authToken,
  });
  const controller = new AbortController();
  const filter = buildFilter();
  const resumeId = await readCursor(cursorFile);

  process.once("SIGINT", () => controller.abort());
  process.once("SIGTERM", () => controller.abort());

  console.log("Connecting to Torii:", toriiUrl);
  console.log("Streaming filter:", JSON.stringify(filter));
  if (resumeId) {
    console.log("Resuming from Last-Event-ID:", resumeId);
  }
  if (!Number.isFinite(maxEvents)) {
    console.log("Running until interrupted…");
  } else {
    console.log(`Will exit after ${maxEvents} events.`);
  }

  let seen = 0;
  try {
    for await (const event of torii.streamEvents({
      filter,
      lastEventId: resumeId ?? undefined,
      signal: controller.signal,
    })) {
      if (event.id) {
        await writeCursor(cursorFile, event.id);
      }
      const stamp = new Date().toISOString();
      console.log(`\n[${stamp}] event=${event.event ?? "message"} id=${event.id ?? "∅"}`);
      if (event.retry != null) {
        console.log(`  retry: ${event.retry}ms`);
      }
      if (event.data == null) {
        console.log("  (no data payload)");
      } else {
        console.log("  payload:", JSON.stringify(event.data, null, 2));
        const status = extractPipelineStatusKind(event.data);
        if (status) {
          console.log(`  pipeline_status: ${status}`);
        }
      }
      seen += 1;
      if (Number.isFinite(maxEvents) && seen >= maxEvents) {
        break;
      }
    }
  } catch (error) {
    if (controller.signal.aborted) {
      console.warn("Stream aborted:", error?.name ?? "AbortError");
    } else {
      throw error;
    }
  } finally {
    controller.abort();
  }
}

main().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});

