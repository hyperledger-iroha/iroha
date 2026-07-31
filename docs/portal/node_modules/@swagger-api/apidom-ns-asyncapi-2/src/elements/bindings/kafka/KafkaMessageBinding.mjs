import { ObjectElement } from '@swagger-api/apidom-core';
/**
 * @public
 */
class KafkaMessageBinding extends ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'kafkaMessageBinding';
    this.classes.push('message-binding');
  }
  get key() {
    return this.get('key');
  }
  set key(key) {
    this.set('key', key);
  }
  get schemaIdLocation() {
    return this.get('schemaIdLocation');
  }
  set schemaIdLocation(schemaIdLocation) {
    this.set('schemaIdLocation', schemaIdLocation);
  }
  get schemaIdPayloadEncoding() {
    return this.get('schemaIdPayloadEncoding');
  }
  set schemaIdPayloadEncoding(schemaIdPayloadEncoding) {
    this.set('schemaIdPayloadEncoding', schemaIdPayloadEncoding);
  }
  get schemaLookupStrategy() {
    return this.get('schemaLookupStrategy');
  }
  set schemaLookupStrategy(schemaLookupStrategy) {
    this.set('schemaLookupStrategy', schemaLookupStrategy);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
export default KafkaMessageBinding;