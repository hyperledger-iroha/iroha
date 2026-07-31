import { isNonEmptyString } from 'ramda-adjunct';
import PatternedFieldsVisitor from "./PatternedFieldsVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class MapVisitor extends PatternedFieldsVisitor {
  constructor(options) {
    super(options);
    this.fieldPatternPredicate = isNonEmptyString;
  }
}
export default MapVisitor;