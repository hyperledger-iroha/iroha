import _asyncToGenerator from "@babel/runtime/helpers/asyncToGenerator";
import _extends from "@babel/runtime/helpers/extends";
import _classCallCheck from "@babel/runtime/helpers/classCallCheck";
import _createClass from "@babel/runtime/helpers/createClass";
import _possibleConstructorReturn from "@babel/runtime/helpers/possibleConstructorReturn";
import _getPrototypeOf from "@babel/runtime/helpers/getPrototypeOf";
import _inherits from "@babel/runtime/helpers/inherits";
import _defineProperty from "@babel/runtime/helpers/defineProperty";
import _regeneratorRuntime from "@babel/runtime/regenerator";
function _callSuper(t, o, e) { return o = _getPrototypeOf(o), _possibleConstructorReturn(t, _isNativeReflectConstruct() ? Reflect.construct(o, e || [], _getPrototypeOf(t).constructor) : o.apply(t, e)); }
function _isNativeReflectConstruct() { try { var t = !Boolean.prototype.valueOf.call(Reflect.construct(Boolean, [], function () {})); } catch (t) {} return (_isNativeReflectConstruct = function _isNativeReflectConstruct() { return !!t; })(); }
import React from 'react';
import highlight from "./highlight.js";
export default (function (options) {
  var _ReactAsyncHighlighter;
  var loader = options.loader,
    isLanguageRegistered = options.isLanguageRegistered,
    registerLanguage = options.registerLanguage,
    languageLoaders = options.languageLoaders,
    noAsyncLoadingLanguages = options.noAsyncLoadingLanguages;
  var ReactAsyncHighlighter = /*#__PURE__*/function (_React$PureComponent) {
    function ReactAsyncHighlighter() {
      _classCallCheck(this, ReactAsyncHighlighter);
      return _callSuper(this, ReactAsyncHighlighter, arguments);
    }
    _inherits(ReactAsyncHighlighter, _React$PureComponent);
    return _createClass(ReactAsyncHighlighter, [{
      key: "componentDidUpdate",
      value: function componentDidUpdate() {
        if (!ReactAsyncHighlighter.isRegistered(this.props.language) && languageLoaders) {
          this.loadLanguage();
        }
      }
    }, {
      key: "componentDidMount",
      value: function componentDidMount() {
        var _this = this;
        if (!ReactAsyncHighlighter.astGeneratorPromise) {
          ReactAsyncHighlighter.loadAstGenerator();
        }
        if (!ReactAsyncHighlighter.astGenerator) {
          ReactAsyncHighlighter.astGeneratorPromise.then(function () {
            _this.forceUpdate();
          });
        }
        if (!ReactAsyncHighlighter.isRegistered(this.props.language) && languageLoaders) {
          this.loadLanguage();
        }
      }
    }, {
      key: "loadLanguage",
      value: function loadLanguage() {
        var _this2 = this;
        var language = this.props.language;
        if (language === 'text') {
          return;
        }
        ReactAsyncHighlighter.loadLanguage(language).then(function () {
          return _this2.forceUpdate();
        })["catch"](function () {});
      }
    }, {
      key: "normalizeLanguage",
      value: function normalizeLanguage(language) {
        return ReactAsyncHighlighter.isSupportedLanguage(language) ? language : 'text';
      }
    }, {
      key: "render",
      value: function render() {
        return /*#__PURE__*/React.createElement(ReactAsyncHighlighter.highlightInstance, _extends({}, this.props, {
          language: this.normalizeLanguage(this.props.language),
          astGenerator: ReactAsyncHighlighter.astGenerator
        }));
      }
    }], [{
      key: "preload",
      value: function preload() {
        return ReactAsyncHighlighter.loadAstGenerator();
      }
    }, {
      key: "loadLanguage",
      value: function () {
        var _loadLanguage = _asyncToGenerator(/*#__PURE__*/_regeneratorRuntime.mark(function _callee(language) {
          var languageLoader;
          return _regeneratorRuntime.wrap(function (_context) {
            while (1) switch (_context.prev = _context.next) {
              case 0:
                languageLoader = languageLoaders[language];
                if (!(typeof languageLoader === 'function')) {
                  _context.next = 1;
                  break;
                }
                return _context.abrupt("return", languageLoader(ReactAsyncHighlighter.registerLanguage));
              case 1:
                throw new Error("Language ".concat(language, " not supported"));
              case 2:
              case "end":
                return _context.stop();
            }
          }, _callee);
        }));
        function loadLanguage(_x) {
          return _loadLanguage.apply(this, arguments);
        }
        return loadLanguage;
      }()
    }, {
      key: "isSupportedLanguage",
      value: function isSupportedLanguage(language) {
        return ReactAsyncHighlighter.isRegistered(language) || typeof languageLoaders[language] === 'function';
      }
    }, {
      key: "loadAstGenerator",
      value: function loadAstGenerator() {
        ReactAsyncHighlighter.astGeneratorPromise = loader().then(function (astGenerator) {
          ReactAsyncHighlighter.astGenerator = astGenerator;
          if (registerLanguage) {
            ReactAsyncHighlighter.languages.forEach(function (language, name) {
              return registerLanguage(astGenerator, name, language);
            });
          }
        });
        return ReactAsyncHighlighter.astGeneratorPromise;
      }
    }]);
  }(React.PureComponent);
  _ReactAsyncHighlighter = ReactAsyncHighlighter;
  _defineProperty(ReactAsyncHighlighter, "astGenerator", null);
  _defineProperty(ReactAsyncHighlighter, "highlightInstance", highlight(null, {}));
  _defineProperty(ReactAsyncHighlighter, "astGeneratorPromise", null);
  _defineProperty(ReactAsyncHighlighter, "languages", new Map());
  _defineProperty(ReactAsyncHighlighter, "supportedLanguages", options.supportedLanguages || Object.keys(languageLoaders || {}));
  _defineProperty(ReactAsyncHighlighter, "isRegistered", function (language) {
    if (noAsyncLoadingLanguages) {
      return true;
    }
    if (!registerLanguage) {
      throw new Error("Current syntax highlighter doesn't support registration of languages");
    }
    if (!_ReactAsyncHighlighter.astGenerator) {
      // Ast generator not available yet, but language will be registered once it is.
      return _ReactAsyncHighlighter.languages.has(language);
    }
    return isLanguageRegistered(_ReactAsyncHighlighter.astGenerator, language);
  });
  _defineProperty(ReactAsyncHighlighter, "registerLanguage", function (name, language) {
    if (!registerLanguage) {
      throw new Error("Current syntax highlighter doesn't support registration of languages");
    }
    if (_ReactAsyncHighlighter.astGenerator) {
      return registerLanguage(_ReactAsyncHighlighter.astGenerator, name, language);
    } else {
      _ReactAsyncHighlighter.languages.set(name, language);
    }
  });
  return ReactAsyncHighlighter;
});