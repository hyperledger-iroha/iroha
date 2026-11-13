#ifndef KAGEMUSHA_SPARTAN_DEVICE_FIXTURE_H
#define KAGEMUSHA_SPARTAN_DEVICE_FIXTURE_H
#include <stddef.h>
#include <stdint.h>
#include "connect_norito_bridge.h"
#ifdef __cplusplus
extern "C" {
#endif
/* Isolated independently issued fixture only. No production wallet handle is accepted.
 * Every callback is synchronous; the background caller keeps its context alive for
 * the entire run. collect may block while an async Apple task performs its effects.
 * journal is borrowed only until collect returns; serialize its calls and never
 * retain it. Payment signing uses the canonical platform callbacks below. */
typedef int32_t (*SpartanFixtureCollectAppleV1)(void *context,
    const uint8_t *slot32, const uint8_t *payment_key65,
    const uint8_t *challenge32, void *journal);
/* A Send approval must use the SDK's hardware-backed device authentication over
 * this exact native review. Zero is confirmed; nonzero is refusal/unavailable. */
typedef int32_t (*SpartanFixtureConfirmV1)(void *context,
    const uint8_t *review, size_t review_len);
/* 0 originals: output is three u32-BE lengths followed by each exact original;
 * 1 begin(stage1..3), 2 retain(stage1..3,input), 3 complete. Mutations return empty.
 * Return 0 on success; negative on refusal. Refusal output is bounded UTF-8 text. */
int32_t spartan_fixture_apple_journal_v1(void *journal, uint32_t operation,
    uint8_t stage, const uint8_t *input, size_t input_len,
    uint8_t *output, size_t capacity, size_t *output_len);
/* Call on a background thread. root/artifacts are absolute UTF-8 paths. root must
 * be a dedicated new fixture directory; it is reused for exact cold-start recovery.
 * Required artifacts: program-bundle.norito, parameter-manifest.norito,
 * parameter-signature.bin emitted by the real full recursive chain compiler.
 * output receives a UTF-8 report; return 0 means all executed assertions passed,
 * negative means failure/pending. Passing this fixture is not FI admission. */
int32_t spartan_fixture_run_apple_v1(
    const connect_norito_kagemusha_platform_v1 *platform,
    const uint8_t *root, size_t root_len,
    const uint8_t *artifacts, size_t artifacts_len,
    void *context, SpartanFixtureCollectAppleV1 collect,
    SpartanFixtureConfirmV1 confirm,
    uint8_t *output, size_t capacity, size_t *output_len);
#ifdef __cplusplus
}
#endif
#endif
