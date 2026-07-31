import { AllOfVisitor as AllOfJSONSchema202012Options } from '@swagger-api/apidom-ns-json-schema-2020-12';
/**
 * @public
 */
class AllOfVisitor extends AllOfJSONSchema202012Options {
  constructor(options) {
    super(options);
    this.passingOptionsNames.push('parent');
  }
}
export default AllOfVisitor;