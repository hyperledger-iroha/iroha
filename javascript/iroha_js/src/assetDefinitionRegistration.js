export function createRegisterAssetDefinitionInstructionBuilder({
  normalizeTransactionAssetDefinitionId,
  buildRegisterAssetDefinitionInstruction,
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
    if (
      Object.prototype.hasOwnProperty.call(assetDefinition, "confidentialPolicy") ||
      Object.prototype.hasOwnProperty.call(assetDefinition, "confidential_policy")
    ) {
      throw new TypeError(
        "assetDefinition cannot carry confidential policy; use RegisterZkAsset with canonical verifier bindings",
      );
    }
    instructions.push(
      buildRegisterAssetDefinitionInstruction({
        assetDefinitionId,
        name: assetDefinition.name,
        description: assetDefinition.description ?? null,
        alias: assetDefinition.alias ?? null,
        logo: assetDefinition.logo ?? null,
        scale: assetDefinition.spec?.scale ?? null,
        mintable: assetDefinition.mintable ?? "Infinitely",
        metadata: assetDefinition.metadata ?? {},
        balanceScopePolicy,
        owningDomain,
      }),
    );
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
