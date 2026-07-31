import parse from "./parse/index.mjs";
/**
 * Test if a string is a server URL template.
 *
 * @param {string} serverURLTemplate
 * @param {Object} [options={}] - An object.
 * @param {boolean} [options.strict=true] - A boolean indicating presence of at least one `Server Variable` AST node.
 * @returns {boolean}
 */
const test = (serverURLTemplate, {
  strict = false
} = {}) => {
  try {
    const parseResult = parse(serverURLTemplate);
    if (!parseResult.result.success) return false;
    const parts = [];
    parseResult.ast.translate(parts);
    const hasServerVariable = parts.some(([type]) => type === 'server-variable');
    if (!strict && !hasServerVariable) {
      try {
        new URL(serverURLTemplate, 'https://vladimirgorej.com');
        return true;
      } catch {
        return false;
      }
    }
    return strict ? hasServerVariable : true;
  } catch {
    return false;
  }
};
export default test;