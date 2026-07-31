import { Parser } from 'apg-lite';
import Grammar from "../../../grammar.mjs";
import { percentEncodeChar, isQuoted as isQuotedPredicate, unquote, quote } from "../../../utils.mjs";
import testCookieValue from "../../../cookie/test/cookie-value.mjs";
const parser = new Parser();
const grammar = new Grammar();
const cookieValueStrictPercentEncoder = cookieValue => {
  const value = String(cookieValue);

  // return early if the value is already valid
  if (testCookieValue(value)) return value;

  // detect if the value is quoted
  const isQuoted = isQuotedPredicate(value);

  // remove quotes if present for processing
  const valueToEncode = isQuoted ? unquote(value) : value;
  let result = '';
  for (const char of valueToEncode) {
    result += parser.parse(grammar, 'cookie-octet', char).success ? char : percentEncodeChar(char);
  }

  // return quoted if input was quoted, unquoted otherwise
  return isQuoted ? quote(result) : result;
};
export default cookieValueStrictPercentEncoder;