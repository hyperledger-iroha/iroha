"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
/**
 * @public
 */
class RedisOperationBinding extends _apidomCore.ObjectElement {
  constructor(content, meta, attributes) {
    super(content, meta, attributes);
    this.element = 'redisOperationBinding';
    this.classes.push('operation-binding');
  }
}
var _default = exports.default = RedisOperationBinding;