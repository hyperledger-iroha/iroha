---
lang: ar
direction: rtl
source: docs/source/sdk/python/privacy_admin.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 80eb4db98b4f188bc61f2f84d8c13d10e29d11142e4a7400ea20668163ebd8d5
source_last_modified: "2026-01-03T18:08:00.420709+00:00"
translation_last_reviewed: 2026-01-30
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraNet Privacy Admin Feed Helpers (PY6-P4/P5)

Roadmap item **PY6-P4/P5 — telemetry & admin surfaces** requires the Python SDK
to ingest the relay admin feed exposed at `/privacy/events`. Relays publish
newline-delimited JSON (`application/x-ndjson`) containing
`SoranetPrivacyEventV1` payloads that mirror the counters described in
`docs/source/soranet/privacy_metrics_pipeline.md`. Collectors poll the admin
listener, parse each entry, and forward the events to Torii or secure
aggregation pipelines.

The SDK now exports typed helpers in `iroha_python.privacy` so telemetry
collectors and admin tooling can consume these feeds without duplicating the
data-model logic:

- `fetch_privacy_events(base_url, *, session=None, timeout=10.0)` downloads the
  NDJSON feed from the given relay admin URL (either a host such as
  `http://relay-admin:7070` or the full `/privacy/events` endpoint) and returns
  a list of parsed `PrivacyEvent` objects.
- `stream_privacy_events(base_url, *, session=None, timeout=10.0, chunk_size=65536)`
  yields one `PrivacyEvent` at a time so collectors can process long-running
  feeds without buffering them entirely in memory. Pass the relay admin URL or
  the explicit endpoint path (e.g., `/admin/privacy/events`) and iterate over
  the returned generator.
- `load_privacy_events_from_ndjson(text)` parses an already-downloaded NDJSON
  blob.
- `parse_privacy_event_line(line)` / `parse_privacy_event(obj)` convert a single
  JSON line/object into the typed dataclasses.

Example collector loop:

```python
from iroha_python.privacy import (
    PrivacyEventKind,
    fetch_privacy_events,
    stream_privacy_events,
)

def poll_privacy(relay_admin_url: str) -> None:
    events = fetch_privacy_events(relay_admin_url)
    for event in events:
        print(
            f"[{event.mode.value}] kind={event.kind.value} "
            f"timestamp={event.timestamp_unix}"
        )
        if event.kind is PrivacyEventKind.THROTTLE:
            scope = event.payload.scope.value  # type: ignore[union-attr]
            print(f"  throttle scope: {scope}")

def poll_privacy_streaming(relay_admin_url: str) -> None:
    for event in stream_privacy_events(relay_admin_url):
        print(f"[stream] {event.kind.value} @ {event.timestamp_unix}")
```

The helper enforces the same schema as the Rust data model (`mode` labels,
handshake failure reasons, throttle scopes, etc.) so collectors surface clear
errors when relays emit malformed data. Use the parsed dataclasses to redact,
bucket, or forward the events as required by SNNet-8. When integrating with the
secure aggregator, pair this helper with the Torii ingestion endpoints described
in `docs/source/soranet/privacy_metrics_pipeline.md`.

## CLI helper

For quick audits or evidence collection you can run the bundled CLI:

```bash
python -m iroha_python.examples.privacy_events http://relay-admin:7070 \
  --output artifacts/privacy/relay-A.ndjson
```

By default the helper writes NDJSON to the given path (or stdout when `--output`
is omitted) and exits after draining the feed once. Pass `--format json` to
emit a single JSON array or `--stream --max-events <n>` to follow the feed
continuously:

```bash
python -m iroha_python.examples.privacy_events http://relay-admin:7070 \
  --stream --max-events 0 --output -
```

Streaming mode writes NDJSON as events arrive, making it useful for tailing
relay telemetry during incident response or when validating Alertmanager rules.
All CLI flags forward directly to :func:`fetch_privacy_events` /
:func:`stream_privacy_events`, so operators can adjust the HTTP timeout or
override the endpoint path to match their relay configuration.
