const transports = new WeakMap();
const receipts = new WeakMap();

/** @internal Install the actual Node client's constructor-owned HTTPS transport. */
export function registerPrivacyExact12CapabilityManifestTransportV1(client, transport) {
  if (transports.has(client) || typeof transport !== "function") {
    throw new TypeError("invalid Exact12 capability transport registration");
  }
  transports.set(client, transport);
}

/** @internal Fetch through a registered client; caller-defined symbols grant no authority. */
export async function fetchPrivacyExact12CapabilityManifestTransportV1(client, options) {
  const transport = transports.get(client);
  if (!transport) {
    throw new TypeError(
      "Exact12 capability admission requires the N-API Torii client; browser and mock transports cannot authorize privacy",
    );
  }
  const state = await transport(options);
  const receipt = Object.freeze({});
  receipts.set(receipt, state);
  return receipt;
}

/** @internal Consume one response receipt; archives cannot recreate its transport origin. */
export function consumePrivacyExact12CapabilityManifestTransportV1(receipt) {
  const state = receipts.get(receipt);
  if (!state) throw new TypeError("invalid or replayed Exact12 transport receipt");
  receipts.delete(receipt);
  return state;
}
