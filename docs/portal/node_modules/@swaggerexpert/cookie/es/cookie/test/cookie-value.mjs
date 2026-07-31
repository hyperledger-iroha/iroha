import { Parser } from 'apg-lite';
import Grammar from "../../grammar.mjs";
import { isQuoted } from "../../utils.mjs";
const parser = new Parser();
const grammar = new Grammar();
const testCookieValue = (cookieValue, {
  strict = true,
  quoted = null
} = {}) => {
  try {
    const startRule = strict ? 'cookie-value' : 'lenient-cookie-value';
    const result = parser.parse(grammar, startRule, cookieValue);
    if (typeof quoted === 'boolean') {
      return result.success && quoted === isQuoted(cookieValue);
    }
    return result.success;
  } catch {
    return false;
  }
};
export default testCookieValue;