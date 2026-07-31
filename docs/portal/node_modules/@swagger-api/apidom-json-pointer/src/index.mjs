export {
/**
 * Representation
 */
JSONString, URIFragmentIdentifier,
/**
 * Parsing
 */
parse, CSTTranslator, ASTTranslator, XMLTranslator,
/**
 * Testing
 */
testJSONPointer, testReferenceToken, testArrayLocation, testArrayIndex, testArrayDash,
/**
 * Compiling
 */
compile,
/**
 * Escaping
 */
escape, unescape,
/**
 * Grammar
 */
Grammar,
/**
 * Errors
 */
JSONPointerError, JSONPointerParseError, JSONPointerCompileError, JSONPointerEvaluateError, JSONPointerTypeError, JSONPointerKeyError, JSONPointerIndexError } from '@swaggerexpert/json-pointer';
/**
 * Contextual Evaluation for ApiDOM
 */
export { evaluate } from '@swaggerexpert/json-pointer/evaluate/realms/apidom';
/**
 * Re-export all types
 */