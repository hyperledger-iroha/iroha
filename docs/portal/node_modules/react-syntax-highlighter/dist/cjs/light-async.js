"use strict";

var _interopRequireDefault = require("@babel/runtime/helpers/interopRequireDefault");
Object.defineProperty(exports, "__esModule", {
  value: true
});
exports["default"] = void 0;
var _typeof2 = _interopRequireDefault(require("@babel/runtime/helpers/typeof"));
var _asyncSyntaxHighlighter = _interopRequireDefault(require("./async-syntax-highlighter"));
var _hljs = _interopRequireDefault(require("./async-languages/hljs"));
var _checkForListedLanguage = _interopRequireDefault(require("./checkForListedLanguage"));
function _interopRequireWildcard(e, t) { if ("function" == typeof WeakMap) var r = new WeakMap(), n = new WeakMap(); return (_interopRequireWildcard = function _interopRequireWildcard(e, t) { if (!t && e && e.__esModule) return e; var o, i, f = { __proto__: null, "default": e }; if (null === e || "object" != (0, _typeof2["default"])(e) && "function" != typeof e) return f; if (o = t ? n : r) { if (o.has(e)) return o.get(e); o.set(e, f); } for (var _t in e) "default" !== _t && {}.hasOwnProperty.call(e, _t) && ((i = (o = Object.defineProperty) && Object.getOwnPropertyDescriptor(e, _t)) && (i.get || i.set) ? o(f, _t, i) : f[_t] = e[_t]); return f; })(e, t); }
var _default = exports["default"] = (0, _asyncSyntaxHighlighter["default"])({
  loader: function loader() {
    return Promise.resolve().then(function () {
      return _interopRequireWildcard(require(/* webpackChunkName:"react-syntax-highlighter/lowlight-import" */
      'lowlight/lib/core'));
    }).then(function (module) {
      // Webpack 3 returns module.exports as default as module, but webpack 4 returns module.exports as module.default
      return module["default"] || module;
    });
  },
  isLanguageRegistered: function isLanguageRegistered(instance, language) {
    return !!(0, _checkForListedLanguage["default"])(instance, language);
  },
  languageLoaders: _hljs["default"],
  registerLanguage: function registerLanguage(instance, name, language) {
    return instance.registerLanguage(name, language);
  }
});