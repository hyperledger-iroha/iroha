import { Parser, Stats } from 'apg-lite';
import Grammar from "../grammar.mjs";
import JSONPointerParseError from "../errors/JSONPointerParseError.mjs";
import ASTTranslator from "./translators/ASTTranslator.mjs";
import Trace from "./trace/Trace.mjs";
const grammar = new Grammar();
const parse = (jsonPointer, {
  translator = new ASTTranslator(),
  stats = false,
  trace = false
} = {}) => {
  if (typeof jsonPointer !== 'string') {
    throw new TypeError('JSON Pointer must be a string');
  }
  try {
    const parser = new Parser();
    if (translator) parser.ast = translator;
    if (stats) parser.stats = new Stats();
    if (trace) parser.trace = new Trace();
    const result = parser.parse(grammar, 'json-pointer', jsonPointer);
    return {
      result,
      tree: result.success && translator ? parser.ast.getTree() : undefined,
      stats: parser.stats,
      trace: parser.trace
    };
  } catch (error) {
    throw new JSONPointerParseError('Unexpected error during JSON Pointer parsing', {
      cause: error,
      jsonPointer
    });
  }
};
export default parse;