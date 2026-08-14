export function createSubscriptionResponseNormalizers({
  cloneJsonValue,
  ensureRecord,
  isPlainObject,
  normalizeAccountId,
  normalizeAppApiTransactionDraft,
  normalizeAssetDefinitionId,
  normalizeUnsignedInteger,
  requireExactBoolean,
  requireExactLowerEvenHexString,
  requireExactNonEmptyString,
  requireNonEmptyString,
}) {
  function normalizeInstructionDrafts(value, context, minimumLength = 1) {
    if (!Array.isArray(value) || value.length < minimumLength) {
      throw new TypeError(`${context} must contain at least ${minimumLength} instruction(s)`);
    }
    return value.map((raw, index) => {
      const item = ensureRecord(raw, `${context}[${index}]`);
      return {
        wire_id: requireNonEmptyString(item.wire_id, `${context}[${index}].wire_id`),
        payload_hex: requireExactLowerEvenHexString(
          item.payload_hex,
          `${context}[${index}].payload_hex`,
        ),
      };
    });
  }

  function normalizePlanCreateResponse(payload, expectedPlanId) {
    const context = "subscription plan create response";
    const record = ensureRecord(payload, context);
    const draft = normalizeAppApiTransactionDraft(record, context, ["plan_id"]);
    const planId = normalizeAssetDefinitionId(record.plan_id);
    if (planId !== expectedPlanId) {
      throw new TypeError(`${context}.plan_id is not bound to the request`);
    }
    return { ...draft, plan_id: planId };
  }

  function normalizeUsageDraft(payload, expectedSubscriptionId, context) {
    const record = ensureRecord(payload, context);
    const draft = normalizeAppApiTransactionDraft(record, context, ["subscription_id"]);
    const subscriptionId = requireExactNonEmptyString(
      record.subscription_id,
      `${context}.subscription_id`,
    );
    if (subscriptionId !== expectedSubscriptionId) {
      throw new TypeError(`${context}.subscription_id is not bound to the request`);
    }
    return { ...draft, subscription_id: subscriptionId };
  }

  function normalizeCreateResponse(payload, expected) {
    const context = "subscription create response";
    const record = ensureRecord(payload, context);
    if (record.version !== 1 || record.action !== "create") {
      throw new TypeError(`${context} must use the V1 create layout`);
    }
    if (!isPlainObject(record.resulting_subscription)) {
      throw new TypeError(`${context}.resulting_subscription is required`);
    }
    const normalized = {
      version: 1,
      authority: normalizeAccountId(record.authority, `${context}.authority`),
      action: "create",
      subscription_id: requireNonEmptyString(
        record.subscription_id,
        `${context}.subscription_id`,
      ),
      plan_id: normalizeAssetDefinitionId(record.plan_id),
      billing_trigger_id: requireNonEmptyString(
        record.billing_trigger_id,
        `${context}.billing_trigger_id`,
      ),
      usage_trigger_id: record.usage_trigger_id == null
        ? null
        : requireNonEmptyString(record.usage_trigger_id, `${context}.usage_trigger_id`),
      first_charge_ms: normalizeUnsignedInteger(
        record.first_charge_ms,
        `${context}.first_charge_ms`,
        { allowZero: true },
      ),
      provider_usage_grant_included: requireExactBoolean(
        record.provider_usage_grant_included,
        `${context}.provider_usage_grant_included`,
      ),
      resulting_subscription: cloneJsonValue(
        record.resulting_subscription,
        `${context}.resulting_subscription`,
      ),
      tx_instructions: normalizeInstructionDrafts(
        record.tx_instructions,
        `${context}.tx_instructions`,
        2,
      ),
    };
    if (
      normalized.authority !== expected.authority
      || normalized.subscription_id !== expected.subscription_id
      || normalized.plan_id !== expected.plan_id
    ) {
      throw new TypeError(`${context} is not bound to the request`);
    }
    return normalized;
  }

  function normalizeActionResponse(
    payload,
    expectedAction,
    expectedAuthority,
    expectedSubscriptionId,
    context,
  ) {
    const record = ensureRecord(payload, context);
    if (record.version !== 1 || record.action !== expectedAction) {
      throw new TypeError(`${context} must use the expected V1 ${expectedAction} layout`);
    }
    if (!isPlainObject(record.details)) {
      throw new TypeError(`${context}.details is required`);
    }
    const normalized = {
      version: 1,
      authority: normalizeAccountId(record.authority, `${context}.authority`),
      action: expectedAction,
      subscription_id: requireNonEmptyString(
        record.subscription_id,
        `${context}.subscription_id`,
      ),
      details: cloneJsonValue(record.details, `${context}.details`),
      tx_instructions: normalizeInstructionDrafts(
        record.tx_instructions,
        `${context}.tx_instructions`,
      ),
    };
    if (
      normalized.authority !== expectedAuthority
      || normalized.subscription_id !== expectedSubscriptionId
    ) {
      throw new TypeError(`${context} is not bound to the request`);
    }
    return normalized;
  }

  return Object.freeze({
    normalizeSubscriptionActionResponse: normalizeActionResponse,
    normalizeSubscriptionCreateResponse: normalizeCreateResponse,
    normalizeSubscriptionPlanCreateResponse: normalizePlanCreateResponse,
    normalizeSubscriptionUsageDraft: normalizeUsageDraft,
  });
}
