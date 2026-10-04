#!/usr/bin/env node
/**
 * Streaming helper for `/v1/events/sse`.
 *
 * This recipe demonstrates how to:
 * - subscribe to pipeline transaction events with a deterministic filter,
 * - make the live-only, no-replay reconnect semantics explicit,
 * - honour Ctrl+C / SIGTERM via `AbortController`, and
 * - read the typed payloads (`category`, `event`, `status`, rejection codes).
 *
 * Environment variables:
 * - TORII_URL — Torii endpoint (default: http://127.0.0.1:8080)
 * - TORII_API_TOKEN / TORII_AUTH_TOKEN — optional headers
 * - PIPELINE_STATUS — transaction status to match: Queued, Expired, Approved
 *   or Rejected (default: Approved)
 * - STREAM_FILTER — override the filter with collection-query text such as
 *   `tx_status in ["Approved", "Rejected"] and tx_dataspace_id = 0`
 * - STREAM_MAX_EVENTS — stop after N events (0 = run indefinitely, default: 10)
 */
import process from "node:process";

import { Filter, ToriiClient, field } from "@iroha/iroha-js";

const toriiUrl = process.env.TORII_URL ?? "http://127.0.0.1:8080";
const apiToken = process.env.TORII_API_TOKEN;
const authToken = process.env.TORII_AUTH_TOKEN;
const customFilter = process.env.STREAM_FILTER;
const statusKind = process.env.PIPELINE_STATUS ?? "Approved";
const maxEventsEnv = process.env.STREAM_MAX_EVENTS ?? "10";

function resolveMaxEvents(value) {
  const parsed = Number.parseInt(String(value ?? "0"), 10);
  if (Number.isNaN(parsed) || parsed < 0) {
    throw new Error(`STREAM_MAX_EVENTS must be a non-negative integer (received ${value}).`);
  }
  return parsed === 0 ? Number.POSITIVE_INFINITY : parsed;
}

function buildFilter() {
  // Parsing locally reports syntax errors with their line and column before
  // connecting; Torii applies the same grammar to event fields.
  return customFilter ? Filter.parse(customFilter) : field("tx_status").eq(statusKind);
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
  console.log("Streaming filter:", filter.toString());
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
      if (event.event === "stream_error") {
        // Terminal: the live stream lost events and cannot replay them.
        console.warn(`\n[${stamp}] stream gap: ${event.data.code} — ${event.data.message}`);
        break;
      }
      const { data } = event;
      console.log(`\n[${stamp}] ${data.category}/${data.event}${data.status ? ` ${data.status}` : ""}`);
      if (data.event === "Transaction") {
        console.log(`  hash: ${data.hash} block_height: ${data.block_height ?? "∅"}`);
        if (data.status === "Rejected") {
          console.log(`  rejection: ${data.rejection_code} — ${data.rejection_reason}`);
        }
      }
      // Heights and ids beyond Number.MAX_SAFE_INTEGER arrive as bigint.
      console.log("  payload:", JSON.stringify(data, (_key, value) => (typeof value === "bigint" ? value.toString() : value)));
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
