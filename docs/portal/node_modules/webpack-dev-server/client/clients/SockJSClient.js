function _typeof(o) { "@babel/helpers - typeof"; return _typeof = "function" == typeof Symbol && "symbol" == typeof Symbol.iterator ? function (o) { return typeof o; } : function (o) { return o && "function" == typeof Symbol && o.constructor === Symbol && o !== Symbol.prototype ? "symbol" : typeof o; }, _typeof(o); }
function _classCallCheck(a, n) { if (!(a instanceof n)) throw new TypeError("Cannot call a class as a function"); }
function _defineProperties(e, r) { for (var t = 0; t < r.length; t++) { var o = r[t]; o.enumerable = o.enumerable || !1, o.configurable = !0, "value" in o && (o.writable = !0), Object.defineProperty(e, _toPropertyKey(o.key), o); } }
function _createClass(e, r, t) { return r && _defineProperties(e.prototype, r), t && _defineProperties(e, t), Object.defineProperty(e, "prototype", { writable: !1 }), e; }
function _toPropertyKey(t) { var i = _toPrimitive(t, "string"); return "symbol" == _typeof(i) ? i : i + ""; }
function _toPrimitive(t, r) { if ("object" != _typeof(t) || !t) return t; var e = t[Symbol.toPrimitive]; if (void 0 !== e) { var i = e.call(t, r || "default"); if ("object" != _typeof(i)) return i; throw new TypeError("@@toPrimitive must return a primitive value."); } return ("string" === r ? String : Number)(t); }
import SockJS from "../modules/sockjs-client/index.js";
import { log } from "../utils/log.js";

/** @typedef {import("../index").EXPECTED_ANY} EXPECTED_ANY */

/**
 * @implements {CommunicationClient}
 */
var SockJSClient = /*#__PURE__*/function () {
  /**
   * @param {string} url url
   */
  function SockJSClient(url) {
    _classCallCheck(this, SockJSClient);
    // SockJS requires `http` and `https` protocols
    this.sock = new SockJS(url.replace(/^ws:/i, "http:").replace(/^wss:/i, "https:"));
    this.sock.onerror = function (error) {
      log.error(error);
    };
  }

  /**
   * @param {(...args: EXPECTED_ANY[]) => void} fn function
   */
  return _createClass(SockJSClient, [{
    key: "onOpen",
    value: function onOpen(fn) {
      this.sock.onopen = fn;
    }

    /**
     * @param {(...args: EXPECTED_ANY[]) => void} fn function
     */
  }, {
    key: "onClose",
    value: function onClose(fn) {
      this.sock.onclose = fn;
    }

    // call f with the message string as the first argument
    /**
     * @param {(...args: EXPECTED_ANY[]) => void} fn function
     */
  }, {
    key: "onMessage",
    value: function onMessage(fn) {
      this.sock.onmessage = function (err) {
        fn(err.data);
      };
    }
  }]);
}();
export { SockJSClient as default };