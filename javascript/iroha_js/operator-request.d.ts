import type { Buffer } from "buffer";
import type { NetworkId } from "./dist/networkId.js";

/** Exact-network signer used for fresh, one-shot Torii operator requests. */
export class OperatorSigningContext {
  constructor(
    networkId: NetworkId,
    signer: {
      publicKey: string;
      sign: (
        message: Buffer,
      ) =>
        | Promise<ArrayBuffer | ArrayBufferView | Buffer>
        | ArrayBuffer
        | ArrayBufferView
        | Buffer;
    },
  );
  readonly networkId: NetworkId;
  readonly publicKey: string;
  sign(message: ArrayBuffer | ArrayBufferView | Buffer): Promise<Buffer>;
}
