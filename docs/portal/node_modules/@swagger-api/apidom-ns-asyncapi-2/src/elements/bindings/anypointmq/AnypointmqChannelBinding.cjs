"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class AnypointmqChannelBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'anypointmqChannelBinding';
    this.classes.push('channel-binding');
  }
  get destination() {
    return this.get('destination');
  }
  set destination(destination) {
    this.set('destination', destination);
  }
  get destinationType() {
    return this.get('destinationType');
  }
  set destinationType(destinationType) {
    this.set('destinationType', destinationType);
  }
  get bindingVersion() {
    return this.get('bindingVersion');
  }
  set bindingVersion(bindingVersion) {
    this.set('bindingVersion', bindingVersion);
  }
}
var _default = exports.default = AnypointmqChannelBinding;