import { utilities, identifiers } from 'apg-lite';
import JSONPointerParseError from "../../errors/JSONPointerParseError.mjs";
const cst = ruleName => {
  return (state, chars, phraseIndex, phraseLength, data) => {
    if (!(typeof data === 'object' && data !== null && !Array.isArray(data))) {
      throw new JSONPointerParseError("parser's user data must be an object");
    }
    if (state === identifiers.SEM_PRE) {
      const node = {
        type: ruleName,
        text: utilities.charsToString(chars, phraseIndex, phraseLength),
        start: phraseIndex,
        length: phraseLength,
        children: []
      };
      if (data.stack.length > 0) {
        const parent = data.stack[data.stack.length - 1];
        parent.children.push(node);
      } else {
        data.root = node;
      }
      data.stack.push(node);
    }
    if (state === identifiers.SEM_POST) {
      data.stack.pop();
    }
  };
};
export default cst;