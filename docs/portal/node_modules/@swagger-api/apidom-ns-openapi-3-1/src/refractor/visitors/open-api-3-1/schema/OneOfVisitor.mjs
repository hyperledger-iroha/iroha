import { OneOfVisitor as OneOfJSONSchema202012Options } from '@swagger-api/apidom-ns-json-schema-2020-12';
/**
 * @public
 */
class OneOfVisitor extends OneOfJSONSchema202012Options {
  constructor(options) {
    super(options);
    this.passingOptionsNames.push('parent');
  }
}
export default OneOfVisitor;