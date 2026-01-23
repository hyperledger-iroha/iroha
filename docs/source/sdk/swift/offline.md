<!-- Offline queue/journal guidance for Swift SDK users -->

# Offline queues and journals (Swift)

This guide documents how to persist Connect and pipeline traffic on iOS/macOS using the Swift SDK, including bounded defaults, evidence export, and replay/recovery flows.

## Components
- **ConnectQueueJournal** — per-direction ciphertext journal for Connect WebSocket sessions. Default caps: 32 records, 1 MiB per direction. Use `ConnectQueueJournal.Configuration` to adjust bounds.
- **ConnectSessionDiagnostics / ConnectReplayRecorder** — writes `state.json`, queue files, and `metrics.ndjson` for audit or replay.
- **OfflineJournal** — wallet-side pending/committed ledger with a hash chain + HMAC (shared with Rust/Android). Defaults: 4096 records, 4 MiB; configure via `OfflineJournalConfiguration`.
- **FilePendingTransactionQueue** — pipeline retry queue (newline-delimited base64 JSON). Defaults: 256 records, 1 MiB; configure via `FilePendingTransactionQueueConfiguration`.

## Connect queue usage
```swift
let sid = ConnectSid.generate(chainId: "sora", appPublicKey: appPk, nonce16: nonce)
let journal = ConnectQueueJournal(
    sessionID: sid.rawBytes,
    configuration: .init(maxRecordsPerQueue: 64, maxBytesPerQueue: 1 << 20)
)
try journal.append(direction: .appToWallet, sequence: 1, ciphertext: payload)
let records = try journal.records(direction: .appToWallet, nowMs: ConnectQueueJournal.timestampNow())
```
Notes:
- Oversize or truncated files raise `ConnectQueueError.overflow` / `.corrupted` instead of loading whole files.
- Use `ConnectSessionDiagnostics.exportJournalBundle(to:)` to copy queue files + manifest + metrics for operators or replay harnesses.

## Offline wallet journal
```swift
let key = try OfflineJournalKey(rawBytes: Data(repeating: 0x11, count: 32))
let journal = try OfflineJournal(
    url: URL(fileURLWithPath: "/tmp/offline.ijournal"),
    key: key,
    configuration: .init(maxRecords: 1024, maxBytes: 2 << 20)
)
let entry = try journal.appendPending(txId: txId, payload: noritoBytes)
try journal.markCommitted(txId: txId)
```
- Hash chain: `chain = BLAKE2b-256(prev_chain || tx_id)`.
- Auth: `HMAC-SHA256(prev_chain || record_without_hmac)`.
- Opening or appending beyond limits raises `OfflineJournalError.integrityViolation`; export and rotate the file before retrying.
- Use `OfflineWallet.buildSignedReceipt(chainId: ..., journal: journal)` to sign receipts and append them to the journal in one call.

## Pipeline retry queue
```swift
let queue = try FilePendingTransactionQueue(
    fileURL: queueURL,
    configuration: .init(maxRecords: 128, maxBytes: 512 * 1024)
)
try queue.enqueue(PendingTransactionRecord(envelope: envelope))
let drained = try queue.drain()
```
- Rejects oversized files and too many records with `overflowBytes/overflowRecords`.
- Corrupted lines surface `corruptedEntry` so callers can export then truncate.

## Evidence and replay
1. Capture during a session with `ConnectReplayRecorder.record(frame:)`.
2. Export the bundle:
   ```swift
   let diagnostics = ConnectSessionDiagnostics(sessionID: sid.rawBytes)
   try diagnostics.exportJournalBundle(to: bundleDir)
   try diagnostics.exportQueueMetrics(to: bundleDir.appendingPathComponent("metrics.ndjson"))
   ```
3. Replay by iterating `ConnectQueueJournal.records` and feeding frames back into your wallet/app harness.

## Troubleshooting checklist
- **Overflow errors**: raise bounds intentionally only after exporting evidence; otherwise drain and clear the queue.
- **Corrupted journal**: keep the file for audit, create a fresh journal directory, and retry. Verify NoritoBridge availability for BLAKE3 hashing paths.
- **Missing bridge**: ensure `NoritoBridge.xcframework` is bundled and codesigned; remove debug-only `NORITO_BRIDGE_*` overrides in production.

## Operational guidance
- Ship with explicit `maxRecords`/`maxBytes` tuned to your retry policy; monitor `connect.queue_*` and `swift.offline.queue_depth` gauges.
- Include queue/journal export steps in support playbooks so operators can collect bundles before clearing state.

## QR stream handoff
Use `OfflineQrStreamEncoder` to split receipts/bundles into frames and animate them as QR codes.

- **Scan pipeline:** `OfflineQrStreamVisionScanner` decodes raw QR bytes via Vision. Use
  `OfflineQrStreamCameraSession` to drive an `AVCaptureSession` and feed frames into
  `OfflineQrStreamScanSession`.
- **Text fallback:** if only `payloadStringValue` is available, decode
  `iroha:qr1:<base64(frame_bytes)>` via `OfflineQrStreamTextCodec`.
- **Playback skins:** use `OfflineQrStreamPlaybackSkin.sakura` for default animation,
  `sakuraReducedMotion` for accessibility, and `sakuraLowPower` for battery-friendly playback.

## Petal stream handoff (custom scanner)
Petal stream renders the same QR stream frames as a sakura petal field and requires a custom
scanner.

- **Encode grids:** use `OfflinePetalStreamEncoder.encodeGrids` to pick a consistent grid size
  for a frame set.
- **Scan pipeline:** `OfflinePetalStreamCameraSession` samples luminance from camera frames and
  feeds results into `OfflinePetalStreamScanSession`.
- **Manual sampling:** if you render your own frames, use
  `OfflinePetalStreamSampler.sampleGridFromRGBA` and call `OfflinePetalStreamScanSession.ingest`.
