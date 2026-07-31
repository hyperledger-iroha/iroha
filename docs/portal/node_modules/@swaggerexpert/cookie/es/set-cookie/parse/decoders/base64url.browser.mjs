import { toBase64 } from "../../../utils.mjs";
const base64urlDecoder = input => {
  const base64 = toBase64(input);
  const binaryString = atob(base64);
  const bytes = new Uint8Array([...binaryString].map(char => char.charCodeAt(0)));
  return new TextDecoder().decode(bytes);
};
export default base64urlDecoder;