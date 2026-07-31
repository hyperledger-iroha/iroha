import parse from "../parse/index.mjs";
const significantTypes = ['slash', 'path-literal', 'template-expression'];

/**
 * Implementation of https://datatracker.ietf.org/doc/html/rfc3986#section-6.2.2.1
 */

const caseNormalizer = pathTemplate => {
  const parseResult = parse(pathTemplate);
  if (!parseResult.result.success) return pathTemplate;
  const parts = [];
  parseResult.ast.translate(parts);
  return parts.reduce((pathTemplateNormalized, [type, value]) => {
    let normalizedValue = value;
    if (type === 'path-literal') {
      normalizedValue = value.replace(/%[0-9a-fA-F]{2}/g, match => match.toUpperCase());
    }
    if (significantTypes.includes(type)) {
      return `${pathTemplateNormalized}${normalizedValue}`;
    }
    return pathTemplateNormalized;
  }, '');
};
export default caseNormalizer;