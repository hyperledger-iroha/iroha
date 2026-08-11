/** Exact immutable genesis-header hash used as an ordinary transaction domain. */
export class NetworkId {
  private constructor();
  static readonly BYTE_LENGTH: 32;
  static parse(literal: string): NetworkId;
  static fromBytes(value: ArrayBuffer | ArrayBufferView): NetworkId;
  readonly literal: string;
  toBytes(): Uint8Array;
  equals(other: unknown): other is NetworkId;
  toString(): string;
  toJSON(): string;
}
