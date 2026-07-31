import JSONSchemaElement from "./elements/JSONSchema.mjs";
import LinkDescriptionElement from "./elements/LinkDescription.mjs";
/**
 * @public
 */
const jsonSchema202012 = {
  namespace: options => {
    const {
      base
    } = options;
    base.register('jSONSchema202012', JSONSchemaElement);
    base.register('linkDescription', LinkDescriptionElement);
    return base;
  }
};
export default jsonSchema202012;