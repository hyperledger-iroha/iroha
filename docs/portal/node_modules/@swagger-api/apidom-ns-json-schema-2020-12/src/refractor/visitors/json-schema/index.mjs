import { JSONSchemaVisitor as JSONSchema201909Visitor } from '@swagger-api/apidom-ns-json-schema-2019-09';
import JSONSchemaElement from "../../../elements/JSONSchema.mjs";
/**
 * @public
 */
class JSONSchemaVisitor extends JSONSchema201909Visitor {
  constructor(options) {
    super(options);
    this.element = new JSONSchemaElement();
  }

  // eslint-disable-next-line class-methods-use-this
  get defaultDialectIdentifier() {
    return 'https://json-schema.org/draft/2020-12/schema';
  }
}
export default JSONSchemaVisitor;