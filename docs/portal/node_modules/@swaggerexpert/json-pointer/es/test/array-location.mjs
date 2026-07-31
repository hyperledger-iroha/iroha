import { Parser } from 'apg-lite';
import Grammar from "../grammar.mjs";
const grammar = new Grammar();
const parser = new Parser();
const testArrayLocation = referenceToken => {
  if (typeof referenceToken !== 'string') return false;
  try {
    return parser.parse(grammar, 'array-location', referenceToken).success;
  } catch {
    return false;
  }
};
export default testArrayLocation;