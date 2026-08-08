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
    const defaultConfidentialPolicy = {
      mode: "TransparentOnly",
      vk_set_hash: null,
      poseidon_params_id: null,
      pedersen_params_id: null,
      pending_transition: null,
    };
    const confidentialPolicy =
      assetDefinition.confidentialPolicy === undefined
        ? defaultConfidentialPolicy
        : { ...defaultConfidentialPolicy, ...assetDefinition.confidentialPolicy };
    instructions.push({
      Register: {
        AssetDefinition: {
          id: assetDefinitionId,
          logo: assetDefinition.logo ?? null,
          metadata: assetDefinition.metadata ?? {},
          mintable: assetDefinition.mintable ?? "Infinitely",
          spec: assetDefinition.spec ?? { scale: null },
          balance_scope_policy: balanceScopePolicy,
          owning_domain: owningDomain,
          confidential_policy: confidentialPolicy,
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
