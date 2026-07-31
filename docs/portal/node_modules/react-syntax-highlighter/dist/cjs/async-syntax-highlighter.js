"use strict";

var _interopRequireDefault = require("@babel/runtime/helpers/interopRequireDefault");
Object.defineProperty(exports, "__esModule", {
  value: true
});
exports["default"] = void 0;
var _regenerator = _interopRequireDefault(require("@babel/runtime/regenerator"));
var _asyncToGenerator2 = _interopRequireDefault(require("@babel/runtime/helpers/asyncToGenerator"));
var _extends2 = _interopRequireDefault(require("@babel/runtime/helpers/extends"));
var _classCallCheck2 = _interopRequireDefault(require("@babel/runtime/helpers/classCallCheck"));
var _createClass2 = _interopRequireDefault(require("@babel/runtime/helpers/createClass"));
var _possibleConstructorReturn2 = _interopRequireDefault(require("@babel/runtime/helpers/possibleConstructorReturn"));
var _getPrototypeOf2 = _interopRequireDefault(require("@babel/runtime/helpers/getPrototypeOf"));
var _inherits2 = _interopRequireDefault(require("@babel/runtime/helpers/inherits"));
var _defineProperty2 = _interopRequireDefault(require("@babel/runtime/helpers/defineProperty"));
var _react = _interopRequireDefault(require("react"));
var _highlight = _interopRequireDefault(require("./highlight"));
function _callSuper(t, o, e) { return o = (0, _getPrototypeOf2["default"])(o), (0, _possibleConstructorReturn2["default"])(t, _isNativeReflectConstruct() ? Reflect.construct(o, e || [], (0, _getPrototypeOf2["default"])(t).constructor) : o.apply(t, e)); }
function _isNativeReflectConstruct() { try { var t = !Boolean.prototype.valueOf.call(Reflect.construct(Boolean, [], function () {})); } catch (t) {} return (_isNativeReflectConstruct = function _isNativeReflectConstruct() { return !!t; })(); }
var _default = exports["default"] = function _default(options) {
  var _ReactAsyncHighlighter;
  var loader = options.loader,
    isLanguageRegistered = options.isLanguageRegistered,
    registerLanguage = options.registerLanguage,
    languageLoaders = options.languageLoaders,
    noAsyncLoadingLanguages = options.noAsyncLoadingLanguages;
  var ReactAsyncHighlighter = /*#__PURE__*/function (_React$PureComponent) {
    function ReactAsyncHighlighter() {
      (0, _classCallCheck2["default"])(this, ReactAsyncHighlighter);
      return _callSuper(this, ReactAsyncHighlighter, arguments);
    }
    (0, _inherits2["default"])(ReactAsyncHighlighter, _React$PureComponent);
    return (0, _createClass2["default"])(ReactAsyncHighlighter, [{
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
        return /*#__PURE__*/_react["default"].createElement(ReactAsyncHighlighter.highlightInstance, (0, _extends2["default"])({}, this.props, {
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
        var _loadLanguage = (0, _asyncToGenerator2["default"])(/*#__PURE__*/_regenerator["default"].mark(function _callee(language) {
          var languageLoader;
          return _regenerator["default"].wrap(function (_context) {
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
  }(_react["default"].PureComponent);
  _ReactAsyncHighlighter = ReactAsyncHighlighter;
  (0, _defineProperty2["default"])(ReactAsyncHighlighter, "astGenerator", null);
  (0, _defineProperty2["default"])(ReactAsyncHighlighter, "highlightInstance", (0, _highlight["default"])(null, {}));
  (0, _defineProperty2["default"])(ReactAsyncHighlighter, "astGeneratorPromise", null);
  (0, _defineProperty2["default"])(ReactAsyncHighlighter, "languages", new Map());
  (0, _defineProperty2["default"])(ReactAsyncHighlighter, "supportedLanguages", options.supportedLanguages || Object.keys(languageLoaders || {}));
  (0, _defineProperty2["default"])(ReactAsyncHighlighter, "isRegistered", function (language) {
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
  (0, _defineProperty2["default"])(ReactAsyncHighlighter, "registerLanguage", function (name, language) {
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
};