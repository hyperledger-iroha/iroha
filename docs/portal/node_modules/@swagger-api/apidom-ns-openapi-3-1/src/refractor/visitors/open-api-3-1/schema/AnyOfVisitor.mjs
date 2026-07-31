import { AnyOfVisitor as AnyOfJSONSchema202012Options } from '@swagger-api/apidom-ns-json-schema-2020-12';
/**
 * @public
 */
class AnyOfVisitor extends AnyOfJSONSchema202012Options {
  constructor(options) {
    super(options);
    this.passingOptionsNames.push('parent');
  }
}
export default AnyOfVisitor;