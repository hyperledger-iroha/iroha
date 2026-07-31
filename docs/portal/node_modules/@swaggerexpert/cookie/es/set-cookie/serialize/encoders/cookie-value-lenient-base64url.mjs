import cookieValueLenientBase64Encoder from "./cookie-value-lenient-base64.mjs";
import base64urlEncoder from "./base64url.node.mjs";
const cookieValueLenientBase64urlEncoder = cookieValue => {
  return cookieValueLenientBase64Encoder(cookieValue, base64urlEncoder);
};
export default cookieValueLenientBase64urlEncoder;