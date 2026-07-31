"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class KafkaServerBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'kafkaServerBinding';
    this.classes.push('server-binding');
  }
  get schemaRegistryUrl() {
    return this.get('schemaRegistryUrl');
  }
  set schemaRegistryUrl(schemaRegistryUrl) {
    this.set('schemaRegistryUrl', schemaRegistryUrl);
  }
  get schemaRegistryVendor() {
    return this.get('schemaRegistryVendor');
  }
  set schemaRegistryVendor(schemaRegistryVendor) {
    this.set('schemaRegistryVendor', schemaRegistryVendor);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
var _default = exports.default = KafkaServerBinding;