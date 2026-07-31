import testCookieValue from "../../../cookie/test/cookie-value.mjs";
import base64Encoder from "./base64.node.mjs";
import { isQuoted as isQuotedPredicate, unquote, quote } from "../../../utils.mjs";
const cookieValueStrictBase64Encoder = (cookieValue, encoder = base64Encoder) => {
  const value = String(cookieValue);

  // return early if the value is already valid
  if (testCookieValue(value)) return value;

  // detect if the value is quoted
  const isQuoted = isQuotedPredicate(value);

  // remove quotes if present for processing
  const valueToEncode = isQuoted ? unquote(value) : value;

  // base64 encode the value
  const base64EncodedValue = encoder(valueToEncode);

  // return quoted if input was quoted, unquoted otherwise
  return isQuoted ? quote(base64EncodedValue) : base64EncodedValue;
};
export default cookieValueStrictBase64Encoder;