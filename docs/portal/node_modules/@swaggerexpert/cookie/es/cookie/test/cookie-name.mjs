import { Parser } from 'apg-lite';
import Grammar from "../../grammar.mjs";
const parser = new Parser();
const grammar = new Grammar();
const testCookieName = (cookieName, {
  strict = true
} = {}) => {
  try {
    const startRule = strict ? 'cookie-name' : 'lenient-cookie-name';
    return parser.parse(grammar, startRule, cookieName).success;
  } catch {
    return false;
  }
};
export default testCookieName;