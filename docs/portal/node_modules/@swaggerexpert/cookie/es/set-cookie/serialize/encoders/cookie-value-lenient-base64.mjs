import testCookieValue from "../../../cookie/test/cookie-value.mjs";
import base64Encoder from "./base64.node.mjs";
import { unquote, quote, isQuoted as isQuotedPredicate } from "../../../utils.mjs";
const cookieValueLenientBase64Encoder = (cookieValue, encoder = base64Encoder) => {
  const value = String(cookieValue);

  // return early if the value is already valid
  if (testCookieValue(value, {
    strict: false
  })) return value;

  // detect if the value is quoted
  const isQuoted = isQuotedPredicate(value);

  // remove quotes if present for processing
  const valueToEncode = isQuoted ? unquote(value) : value;

  // encode the value
  const result = encoder(valueToEncode);

  // return quoted if input was quoted, unquoted otherwise
  return isQuoted ? quote(result) : result;
};
export default cookieValueLenientBase64Encoder;