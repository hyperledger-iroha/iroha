"use strict";

exports.__esModule = true;
exports.default = void 0;
var _index = require("../../identity/index.cjs");
/**
 * Plugin for decorating every element in ApiDOM tree with UUID.
 * @public
 */

const plugin = ({
  length = 6
} = {}) => () => {
  let identityManager;
  return {
    pre() {
      identityManager = new _index.IdentityManager({
        length
      });
    },
    visitor: {
      enter(element) {
        element.id = identityManager.identify(element); // eslint-disable-line no-param-reassign
      }
    },
    post() {
      identityManager = null;
    }
  };
};
var _default = exports.default = plugin;