import JSONSchemaElement from "./elements/JSONSchema.mjs";
import JSONReferenceElement from "./elements/JSONReference.mjs";
import MediaElement from "./elements/Media.mjs";
import LinkDescriptionElement from "./elements/LinkDescription.mjs";
/**
 * @public
 */
const jsonSchemaDraft4 = {
  namespace: options => {
    const {
      base
    } = options;
    base.register('jSONSchemaDraft4', JSONSchemaElement);
    base.register('jSONReference', JSONReferenceElement);
    base.register('media', MediaElement);
    base.register('linkDescription', LinkDescriptionElement);
    return base;
  }
};
export default jsonSchemaDraft4;