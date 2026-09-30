import { Buffer } from "node:buffer";

// Read branded backing geometry, never caller-supplied byteLength/buffer fields.
const typedArrayPrototype = Object.getPrototypeOf(Uint8Array.prototype);
const byteGetters = (prototype, names) => names.map((name) =>
  Object.getOwnPropertyDescriptor(prototype, name).get);
const viewFields = ["buffer", "byteOffset", "byteLength"];
const typedArrayGetters = byteGetters(typedArrayPrototype, viewFields);
const dataViewGetters = byteGetters(DataView.prototype, viewFields);
const arrayBufferLength = Object.getOwnPropertyDescriptor(ArrayBuffer.prototype, "byteLength").get;
const isView = ArrayBuffer.isView;
const ByteView = Uint8Array;

/** Copy only a branded, non-shared byte window after enforcing its allocation bound. */
export function snapshotBoundedBytes(value, label, maximum, LengthError = TypeError) {
  let backing;
  let offset = 0;
  let length;
  try {
    if (isView(value)) {
      let fields;
      try {
        fields = typedArrayGetters.map((get) => Reflect.apply(get, value, []));
      } catch {
        fields = dataViewGetters.map((get) => Reflect.apply(get, value, []));
      }
      [backing, offset, length] = fields;
    } else {
      backing = value;
    }
    // This brand check also excludes shared backing: no stable signed snapshot
    // can be promised while another agent can concurrently mutate its storage.
    const backingLength = Reflect.apply(arrayBufferLength, backing, []);
    if (length === undefined) length = backingLength;
  } catch {
    throw new TypeError(`${label} must be exact bytes backed by an ordinary ArrayBuffer`);
  }
  if (length === 0 || length > maximum) {
    throw new LengthError(
      `${label} must contain 1..${maximum} bytes`,
    );
  }
  return Buffer.from(new ByteView(backing, offset, length));
}
