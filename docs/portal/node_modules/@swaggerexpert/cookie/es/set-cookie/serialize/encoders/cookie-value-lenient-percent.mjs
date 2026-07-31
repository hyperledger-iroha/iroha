import { Parser } from 'apg-lite';
import Grammar from "../../../grammar.mjs";
import { percentEncodeChar, isQuoted as isQuotedPredicate, unquote, quote } from "../../../utils.mjs";
import testCookieValue from "../../../cookie/test/cookie-value.mjs";
const parser = new Parser();
const grammar = new Grammar();
const cookieValueLenientPercentEncoder = cookieValue => {
  const value = String(cookieValue);

  // return early if the value is already valid
  if (testCookieValue(value, {
    strict: false
  })) return value;

  // detect if the value is quoted
  const isQuoted = isQuotedPredicate(value);

  // remove quotes if present for processing
  const valueToEncode = isQuoted ? unquote(value) : value;
  const startRule = isQuoted ? 'lenient-quoted-char' : 'lenient-cookie-octet';
  let result = '';
  for (const char of valueToEncode) {
    result += parser.parse(grammar, startRule, char).success ? char : percentEncodeChar(char);
  }

  // return quoted if input was quoted, unquoted otherwise
  return isQuoted ? quote(result) : result;
};
export default cookieValueLenientPercentEncoder;