// Deterministic UTF-16 code unit ordering for canonicalized payloads.
export function compareUtf16(left, right) {
  if (left === right) {
    return 0;
  }
  const a = String(left);
  const b = String(right);
  const min = Math.min(a.length, b.length);
  for (let index = 0; index < min; index += 1) {
    const diff = a.charCodeAt(index) - b.charCodeAt(index);
    if (diff !== 0) {
      return diff;
    }
  }
  return a.length - b.length;
}
