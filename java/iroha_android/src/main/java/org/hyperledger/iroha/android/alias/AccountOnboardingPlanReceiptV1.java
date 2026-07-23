package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;

/** Stateless signer-authenticated sponsored-onboarding receipt. */
public final class AccountOnboardingPlanReceiptV1 extends AliasJsonValue {
  private final AccountOnboardingPlanBodyV1 body;
  private final String planHash;
  private final String signature;

  /** Constructs one exact receipt. */
  public AccountOnboardingPlanReceiptV1(
      final AccountOnboardingPlanBodyV1 body,
      final String planHash,
      final String signature) {
    if (body == null) throw new IllegalArgumentException("body must not be null");
    if (signature == null || signature.isEmpty() || (signature.length() & 1) != 0) {
      throw new IllegalArgumentException("signature must be non-empty even-length hexadecimal");
    }
    for (int index = 0; index < signature.length(); index++) {
      if (Character.digit(signature.charAt(index), 16) < 0) {
        throw new IllegalArgumentException("signature must be non-empty even-length hexadecimal");
      }
    }
    this.body = body;
    this.planHash = AliasNameSupport.requireHash(planHash, "planHash");
    this.signature = signature;
  }

  public AccountOnboardingPlanBodyV1 body() { return body; }
  public String planHash() { return planHash; }
  public String signature() { return signature; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("body", body.toJsonMap());
    map.put("plan_hash", planHash);
    map.put("signature", signature);
    return map;
  }
}
