import { toBase64url } from "../../../utils.mjs";
import base64Encoder from "./base64.node.mjs";
const base64urlEncoder = input => {
  return toBase64url(base64Encoder(input));
};
export default base64urlEncoder;