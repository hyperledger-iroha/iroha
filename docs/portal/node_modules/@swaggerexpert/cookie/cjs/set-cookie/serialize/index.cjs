"use strict";

exports.__esModule = true;
exports.default = void 0;
var _cookieValueStrictBase64url = _interopRequireDefault(require("./encoders/cookie-value-strict-base64url.cjs"));
var _cookieNameStrict = _interopRequireDefault(require("./validators/cookie-name-strict.cjs"));
var _cookieValueStrict = _interopRequireDefault(require("./validators/cookie-value-strict.cjs"));
var _utils = require("../../utils.cjs");
function _interopRequireDefault(e) { return e && e.__esModule ? e : { default: e }; }
const defaultOptions = {
  encoders: {
    name: _utils.identity,
    value: _cookieValueStrictBase64url.default
  },
  validators: {
    name: _cookieNameStrict.default,
    value: _cookieValueStrict.default
  }
};
const serialize = (name, value, options = {}) => {
  const mergedOptions = {
    ...defaultOptions,
    ...options,
    encoders: {
      ...defaultOptions.encoders,
      ...options.encoders
    },
    validators: {
      ...defaultOptions.validators,
      ...options.validators
    }
  };
  const encodedName = mergedOptions.encoders.name(name);
  const encodedValue = mergedOptions.encoders.value(value);
  mergedOptions.validators.name(encodedName);
  mergedOptions.validators.value(encodedValue);
  return `${encodedName}=${encodedValue}`;
};
var _default = exports.default = serialize;