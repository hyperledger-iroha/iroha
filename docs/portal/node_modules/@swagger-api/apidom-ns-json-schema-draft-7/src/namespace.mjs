import { JSONReferenceElement } from '@swagger-api/apidom-ns-json-schema-draft-6';
import JSONSchemaElement from "./elements/JSONSchema.mjs";
import LinkDescriptionElement from "./elements/LinkDescription.mjs";
/**
 * @public
 */
const jsonSchemaDraft7 = {
  namespace: options => {
    const {
      base
    } = options;
    base.register('jSONSchemaDraft7', JSONSchemaElement);
    base.register('jSONReference', JSONReferenceElement);
    base.register('linkDescription', LinkDescriptionElement);
    return base;
  }
};
export default jsonSchemaDraft7;