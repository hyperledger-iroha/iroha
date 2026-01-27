## Iroha Android SDK — Proposed Changes

### Summary

Two categories of changes: (1) Java 8 API compatibility fixes, and (2) new functionality for offline allowance registration.

### 1. Java 8 API Compatibility (5 files)

Replace Java 9+ APIs with Java 8 equivalents to ensure compatibility on Android devices without full desugaring support.

**Files affected:**
- `FilePendingTransactionQueue.java`
- `OfflineAuditLogger.java`
- `OfflineCounterJournal.java`
- `OfflineVerdictJournal.java`

**Changes:**

| Java 9+ API | Java 8 Replacement |
|---|---|
| `Files.writeString(path, str, ...)` | `Files.write(path, str.getBytes(StandardCharsets.UTF_8), ...)` |
| `Files.readString(path, charset)` | `new String(Files.readAllBytes(path), charset)` |
| `String.isBlank()` | `str.trim().isEmpty()` |

### 2. New Feature: `registerAllowance` on `OfflineToriiClient` (1 file)

**File:** `OfflineToriiClient.java`

Added `registerAllowance(OfflineWalletCertificate, String authority, String privateKeyHex)` method that POSTs a signed certificate to the offline allowances endpoint for on-ledger registration.

This also required adding `toJsonMap()` to `OfflineWalletCertificate.java` to serialize the certificate for the request body.

### 3. Improved Error Reporting in `OfflineToriiClient` (1 file)

**File:** `OfflineToriiClient.java`

The HTTP error handler now includes the response body in the exception message (`"Offline request failed with status <code>: <body>"`), making it easier to diagnose server-side errors.

### 4. Import Cleanup in `OfflineToriiClient` (1 file)

Removed unused imports: `TransportResponse`, `PlatformHttpTransportExecutor`, `OfflineProofRequestKind`. Added missing import: `OfflineWalletCertificate`.