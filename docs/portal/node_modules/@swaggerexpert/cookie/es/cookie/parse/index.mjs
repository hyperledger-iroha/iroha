import { Ast as AST, Parser } from 'apg-lite';
import Grammar from "../../grammar.mjs";
import cookieStringCallback from "./callbacks/cookie-string.mjs";
import cookiePairCallback from "./callbacks/cookie-pair.mjs";
import cookieNameCallback from "./callbacks/cookie-name.mjs";
import cookieValueCallback from "./callbacks/cookie-value.mjs";
const grammar = new Grammar();
const parse = (cookieString, {
  strict = true
} = {}) => {
  const parser = new Parser();
  parser.ast = new AST();
  if (strict) {
    parser.ast.callbacks['cookie-string'] = cookieStringCallback;
    parser.ast.callbacks['cookie-pair'] = cookiePairCallback;
    parser.ast.callbacks['cookie-name'] = cookieNameCallback;
    parser.ast.callbacks['cookie-value'] = cookieValueCallback;
  } else {
    parser.ast.callbacks['lenient-cookie-string'] = cookieStringCallback;
    parser.ast.callbacks['lenient-cookie-pair'] = cookiePairCallback;
    parser.ast.callbacks['lenient-cookie-name'] = cookieNameCallback;
    parser.ast.callbacks['lenient-cookie-value'] = cookieValueCallback;
  }
  const startRule = strict ? 'cookie-string' : 'lenient-cookie-string';
  const result = parser.parse(grammar, startRule, cookieString);
  return {
    result,
    ast: parser.ast
  };
};
export default parse;