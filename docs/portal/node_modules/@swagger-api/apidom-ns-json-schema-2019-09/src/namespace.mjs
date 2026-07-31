import JSONSchemaElement from "./elements/JSONSchema.mjs";
import LinkDescriptionElement from "./elements/LinkDescription.mjs";
/**
 * @public
 */
const jsonSchema201909 = {
  namespace: options => {
    const {
      base
    } = options;
    base.register('jSONSchema201909', JSONSchemaElement);
    base.register('linkDescription', LinkDescriptionElement);
    return base;
  }
};
export default jsonSchema201909;