import { Parser } from 'apg-lite';
import Grammar from "../path-templating.mjs";
import parse from "../parse/index.mjs";
/**
 * Implementation of https://datatracker.ietf.org/doc/html/rfc3986#section-6.2.2.2
 */
const significantTypes = ['slash', 'path-literal', 'template-expression'];
const grammar = new Grammar();
const parser = new Parser();
const percentEndingNormalizer = pathTemplate => {
  const parseResult = parse(pathTemplate);
  if (!parseResult.result.success) return pathTemplate;
  const parts = [];
  parseResult.ast.translate(parts);
  return parts.reduce((pathTemplateNormalized, [type, value]) => {
    let normalizedValue = value;
    if (type === 'path-literal') {
      normalizedValue = value.replace(/%[0-9a-fA-F]{2}/g, match => {
        try {
          const char = decodeURIComponent(match);
          const {
            success
          } = parser.parse(grammar, 'unreserved', char);
          return success ? char : match;
        } catch {
          return match;
        }
      });
    }
    if (significantTypes.includes(type)) {
      return `${pathTemplateNormalized}${normalizedValue}`;
    }
    return pathTemplateNormalized;
  }, '');
};
export default percentEndingNormalizer;