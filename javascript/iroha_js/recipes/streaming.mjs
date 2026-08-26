#!/usr/bin/env node
/**
 * Streaming helper for `/v1/events/sse`.
 *
 * This recipe demonstrates how to:
 * - subscribe to pipeline transaction events with a deterministic filter,
 * - make the live-only, no-replay reconnect semantics explicit,
 * - honour Ctrl+C / SIGTERM via `AbortController`, and
 * - surface pipeline statuses with `extractPipelineStatusKind`.
 *
 * Environment variables:
 * - TORII_URL — Torii endpoint (default: http://127.0.0.1:8080)
 * - TORII_API_TOKEN / TORII_AUTH_TOKEN — optional headers
 * - PIPELINE_STATUS — diagnostic SSE event filter only (default: Committed)
 * - STREAM_FILTER_JSON — override the SSE filter JSON
 * - STREAM_MAX_EVENTS — stop after N events (0 = run indefinitely, default: 10)
 */
import process from "node:process";

import { ToriiClient, extractPipelineStatusKind } from "@iroha/iroha-js";

const toriiUrl = process.env.TORII_URL ?? "http://127.0.0.1:8080";
const apiToken = process.env.TORII_API_TOKEN;
const authToken = process.env.TORII_AUTH_TOKEN;
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

async function main() {
  const maxEvents = resolveMaxEvents(maxEventsEnv);
  const torii = new ToriiClient(toriiUrl, {
    apiToken,
    authToken,
  });
  const controller = new AbortController();
  const filter = buildFilter();

  process.once("SIGINT", () => controller.abort());
  process.once("SIGTERM", () => controller.abort());

  console.log("Connecting to Torii:", toriiUrl);
  console.log("Streaming filter:", JSON.stringify(filter));
  console.log("This endpoint is live-only; reconnects can have a gap and do not replay events.");
  if (!Number.isFinite(maxEvents)) {
    console.log("Running until interrupted…");
  } else {
    console.log(`Will exit after ${maxEvents} events.`);
  }

  let seen = 0;
  try {
    for await (const event of torii.streamEvents({
      filter,
      signal: controller.signal,
    })) {
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
