import { Mixin } from 'ts-mixer';
import { always } from 'ramda';
import RedisMessageBindingElement from "../../../../../../elements/bindings/redis/RedisMessageBinding.mjs";
import FixedFieldsVisitor from "../../../../generics/FixedFieldsVisitor.mjs";
import FallbackVisitor from "../../../../FallbackVisitor.mjs";
/**
 * @public
 */
/**
 * @public
 */
class RedisMessageBindingVisitor extends Mixin(FixedFieldsVisitor, FallbackVisitor) {
  constructor(options) {
    super(options);
    this.element = new RedisMessageBindingElement();
    this.specPath = always(['document', 'objects', 'bindings', 'redis', 'MessageBinding']);
    this.canSupportSpecificationExtensions = false;
  }
}
export default RedisMessageBindingVisitor;