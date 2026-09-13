/** Validate a caller-selected I105 deployment prefix without coercion or defaults. */
export function requireNetworkPrefix(value, context = "networkPrefix") {
  if (!Number.isInteger(value) || value < 0 || value > 65535) {
    throw new TypeError(`${context} must be an integer in 0..65535`);
  }
  return value;
}
