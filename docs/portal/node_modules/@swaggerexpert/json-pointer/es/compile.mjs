import escape from "./escape.mjs";
import JSONPointerCompileError from "./errors/JSONPointerCompileError.mjs";
const compile = referenceTokens => {
  if (!Array.isArray(referenceTokens)) {
    throw new TypeError('Reference tokens must be a list of strings or numbers');
  }
  try {
    if (referenceTokens.length === 0) {
      return '';
    }
    return `/${referenceTokens.map(referenceToken => {
      if (typeof referenceToken !== 'string' && typeof referenceToken !== 'number') {
        throw new TypeError('Reference token must be a string or number');
      }
      return escape(String(referenceToken));
    }).join('/')}`;
  } catch (error) {
    throw new JSONPointerCompileError('Unexpected error during JSON Pointer compilation', {
      cause: error,
      referenceTokens
    });
  }
};
export default compile;