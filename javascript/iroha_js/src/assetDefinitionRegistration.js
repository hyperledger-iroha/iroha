export function createRegisterAssetDefinitionInstructionBuilder({
  normalizeTransactionAssetDefinitionId,
  buildMintAssetInstruction,
}) {
  function buildRegisterAssetDefinitionInstructions({
    assetDefinition,
    mints = [],
  }) {
    const instructions = [];
    const hasOwningDomain = Object.prototype.hasOwnProperty.call(
      assetDefinition,
      "owningDomain",
    );
    const hasSnakeOwningDomain = Object.prototype.hasOwnProperty.call(
      assetDefinition,
      "owning_domain",
    );
    if (!hasOwningDomain && !hasSnakeOwningDomain) {
      throw new TypeError(
        "assetDefinition.owningDomain is required; use null for an intentionally unowned global definition",
      );
    }
    const owningDomain = hasOwningDomain
      ? assetDefinition.owningDomain
      : assetDefinition.owning_domain;
    if (owningDomain !== null && typeof owningDomain !== "string") {
      throw new TypeError("assetDefinition.owningDomain must be a domain identifier or null");
    }
    if (
      hasOwningDomain &&
      hasSnakeOwningDomain &&
      assetDefinition.owningDomain !== assetDefinition.owning_domain
    ) {
      throw new TypeError("assetDefinition ownership aliases disagree");
    }
    const hasBalanceScopePolicy = Object.prototype.hasOwnProperty.call(
      assetDefinition,
      "balanceScopePolicy",
    );
    const hasSnakeBalanceScopePolicy = Object.prototype.hasOwnProperty.call(
      assetDefinition,
      "balance_scope_policy",
    );
    if (!hasBalanceScopePolicy && !hasSnakeBalanceScopePolicy) {
      throw new TypeError("assetDefinition.balanceScopePolicy is required");
    }
    if (
      hasBalanceScopePolicy &&
      hasSnakeBalanceScopePolicy &&
      assetDefinition.balanceScopePolicy !== assetDefinition.balance_scope_policy
    ) {
      throw new TypeError("assetDefinition balance-scope policy aliases disagree");
    }
    const balanceScopePolicy = hasBalanceScopePolicy
      ? assetDefinition.balanceScopePolicy
      : assetDefinition.balance_scope_policy;
    if (balanceScopePolicy !== "Global" && balanceScopePolicy !== "DataspaceRestricted") {
      throw new TypeError(
        "assetDefinition.balanceScopePolicy must be Global or DataspaceRestricted",
      );
    }
    if (balanceScopePolicy === "DataspaceRestricted" && owningDomain === null) {
      throw new TypeError(
        "assetDefinition.owningDomain is required for DataspaceRestricted balances",
      );
    }
    const assetDefinitionId = normalizeTransactionAssetDefinitionId(
      assetDefinition.assetDefinitionId,
      "assetDefinition.assetDefinitionId",
    );
    if (typeof assetDefinition.name !== "string") {
      throw new TypeError("assetDefinition.name is required and must be a string");
    }
    const name = assetDefinition.name;
    const trimmedName = name.trim();
    if (trimmedName.length === 0) {
      throw new TypeError("assetDefinition.name must not be blank");
    }
    if (new TextEncoder().encode(trimmedName).byteLength > 128) {
      throw new TypeError("assetDefinition.name must not exceed 128 UTF-8 bytes");
    }
    if (name.includes("#") || name.includes("@")) {
      throw new TypeError("assetDefinition.name must not contain '#' or '@'");
    }
    if (/\p{Cc}/u.test(name)) {
      throw new TypeError("assetDefinition.name must not contain control characters");
    }
    if (
      Object.prototype.hasOwnProperty.call(assetDefinition, "confidentialPolicy") ||
      Object.prototype.hasOwnProperty.call(assetDefinition, "confidential_policy")
    ) {
      throw new TypeError(
        "assetDefinition cannot carry confidential policy; use RegisterZkAsset with canonical verifier bindings",
      );
    }
    instructions.push({
      Register: {
        AssetDefinition: {
          id: assetDefinitionId,
          name,
          logo: assetDefinition.logo ?? null,
          metadata: assetDefinition.metadata ?? {},
          mintable: assetDefinition.mintable ?? "Infinitely",
          spec: assetDefinition.spec ?? { scale: null },
          balance_scope_policy: balanceScopePolicy,
          owning_domain: owningDomain,
        },
      },
    });
    mints.forEach((mint) => {
      instructions.push(
        buildMintAssetInstruction({
          assetHoldingId: mint.assetHoldingId ?? mint.assetId,
          quantity: mint.quantity,
        }),
      );
    });
    return instructions;
  }

  return buildRegisterAssetDefinitionInstructions;
}
