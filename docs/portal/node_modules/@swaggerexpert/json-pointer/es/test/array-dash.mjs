import { Parser } from 'apg-lite';
import Grammar from "../grammar.mjs";
const grammar = new Grammar();
const parser = new Parser();
const testArrayDash = referenceToken => {
  if (typeof referenceToken !== 'string') return false;
  try {
    return parser.parse(grammar, 'array-dash', referenceToken).success;
  } catch {
    return false;
  }
};
export default testArrayDash;