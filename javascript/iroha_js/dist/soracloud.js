import { getNativeBinding } from "./native.js";

function buildNativeSoraCloudHfDeployRequest(input) {
  const native = getNativeBinding();
  if (typeof native.soracloudBuildHfDeployRequestJson !== "function") {
    throw new Error(
      "Native binding does not expose soracloudBuildHfDeployRequestJson",
    );
  }
  return JSON.parse(
    native.soracloudBuildHfDeployRequestJson(
      input.repoId,
      input.revision ?? undefined,
      input.modelName,
      input.serviceName,
      input.apartmentName ?? undefined,
      input.storageClass,
      String(input.leaseTermMs),
      input.leaseAssetDefinitionId,
      String(input.baseFeeNanos),
      input.privateKeyHex,
    ),
  );
}

/**
 * Build the signed body accepted by `/v1/soracloud/hf/deploy`.
 *
 * @param {{ repoId: string, revision?: string, modelName: string, serviceName: string, apartmentName?: string, storageClass: "hot" | "warm" | "cold", leaseTermMs: number | bigint | string, leaseAssetDefinitionId: string, baseFeeNanos: number | bigint | string, privateKeyHex: string }} input
 * @returns {{ payload: Record<string, unknown>, provenance: { signer: string, signature: string }, generated_service_provenance?: { signer: string, signature: string }, generated_apartment_provenance?: { signer: string, signature: string } }}
 */
export function buildSoraCloudHfDeployRequest(input) {
  return buildNativeSoraCloudHfDeployRequest(input ?? {});
}
