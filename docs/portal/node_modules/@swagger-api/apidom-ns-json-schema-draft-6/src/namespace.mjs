import { JSONReferenceElement, MediaElement } from '@swagger-api/apidom-ns-json-schema-draft-4';
import JSONSchemaElement from "./elements/JSONSchema.mjs";
import LinkDescriptionElement from "./elements/LinkDescription.mjs";
/**
 * @public
 */
const jsonSchemaDraft6 = {
  namespace: options => {
    const {
      base
    } = options;
    base.register('jSONSchemaDraft6', JSONSchemaElement);
    base.register('jSONReference', JSONReferenceElement);
    base.register('media', MediaElement);
    base.register('linkDescription', LinkDescriptionElement);
    return base;
  }
};
export default jsonSchemaDraft6;