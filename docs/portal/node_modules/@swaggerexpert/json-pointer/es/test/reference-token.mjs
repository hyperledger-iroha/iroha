import { Parser } from 'apg-lite';
import Grammar from "../grammar.mjs";
const grammar = new Grammar();
const parser = new Parser();
const testReferenceToken = referenceToken => {
  if (typeof referenceToken !== 'string') return false;
  try {
    return parser.parse(grammar, 'reference-token', referenceToken).success;
  } catch {
    return false;
  }
};
export default testReferenceToken;