import {
  LocalSigningContext,
  NetworkId,
  OperatorSigningContext,
  ToriiClient,
  publicKeyFromPrivate,
  publicKeyMultihash,
  signEd25519,
} from "../src/index.js";

const NETWORK_ID = NetworkId.parse(
  process.env.IROHA_TORII_INTEGRATION_NETWORK_ID
    ?? "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149",
);
const ACCOUNT_ID = process.env.IROHA_TORII_INTEGRATION_ACCOUNT_ID
  ?? "sorauﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV";
const PRIVATE_KEY_WIRE = Buffer.from(
  process.env.IROHA_TORII_INTEGRATION_PRIVATE_KEY_HEX
    ?? "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53",
  "hex",
);
const PRIVATE_KEY = PRIVATE_KEY_WIRE.length === 35
  && PRIVATE_KEY_WIRE.subarray(0, 3).equals(Buffer.from([0x80, 0x26, 0x20]))
  ? PRIVATE_KEY_WIRE.subarray(3)
  : PRIVATE_KEY_WIRE;
const OPERATOR_PRIVATE_KEY_HEX =
  process.env.IROHA_TORII_INTEGRATION_OPERATOR_PRIVATE_KEY_HEX;
if (
  typeof OPERATOR_PRIVATE_KEY_HEX !== "string"
  || !/^(?:[0-9a-fA-F]{64}|802620[0-9a-fA-F]{64})$/u.test(OPERATOR_PRIVATE_KEY_HEX)
) {
  throw new Error(
    "IROHA_TORII_INTEGRATION_OPERATOR_PRIVATE_KEY_HEX must provide a runtime-only 32-byte Ed25519 private key (optionally prefixed by 802620)",
  );
}
const OPERATOR_PRIVATE_KEY_WIRE = Buffer.from(OPERATOR_PRIVATE_KEY_HEX, "hex");
const OPERATOR_PRIVATE_KEY = OPERATOR_PRIVATE_KEY_WIRE.length === 35
  && OPERATOR_PRIVATE_KEY_WIRE.subarray(0, 3).equals(Buffer.from([0x80, 0x26, 0x20]))
  ? OPERATOR_PRIVATE_KEY_WIRE.subarray(3)
  : OPERATOR_PRIVATE_KEY_WIRE;
const OPERATOR_PUBLIC_KEY = process.env.IROHA_TORII_INTEGRATION_OPERATOR_PUBLIC_KEY
  ?? publicKeyMultihash(publicKeyFromPrivate(OPERATOR_PRIVATE_KEY));
export const INTEGRATION_OPERATOR_SIGNING_CONTEXT = new OperatorSigningContext(NETWORK_ID, {
  publicKey: OPERATOR_PUBLIC_KEY,
  sign: (message) => signEd25519(message, OPERATOR_PRIVATE_KEY),
});

/** Live-test client whose existing expensive query calls are exact-network signed. */
export class AuthenticatedIntegrationToriiClient extends ToriiClient {
  constructor(baseUrl, options = {}) {
    super(baseUrl, {
      localSigningContext: new LocalSigningContext(NETWORK_ID),
      canonicalRequestAuth: { accountId: ACCOUNT_ID, privateKey: PRIVATE_KEY },
      operatorSigningContext: INTEGRATION_OPERATOR_SIGNING_CONTEXT,
      ...options,
    });
  }
}
