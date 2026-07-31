/* global __webpack_dev_server_client__ */

import WebSocketClient from "./clients/WebSocketClient.js";
import { log } from "./utils/log.js";

/** @typedef {import("./index.js").EXPECTED_ANY} EXPECTED_ANY */
/** @typedef {import("./clients/SockJSClient")} SockJSClient */

// this WebsocketClient is here as a default fallback, in case the client is not injected
/** @type {CommunicationClientConstructor} */
var Client = typeof __webpack_dev_server_client__ !== "undefined" ? typeof (/** @type {{ default: CommunicationClientConstructor }} */
__webpack_dev_server_client__.default) !== "undefined" ? /** @type {{ default: CommunicationClientConstructor }} */
__webpack_dev_server_client__.default : (/** @type {CommunicationClientConstructor} */
__webpack_dev_server_client__) : WebSocketClient;
var retries = 0;
var maxRetries = 10;

// Initialized client is exported so external consumers can utilize the same instance
// It is mutable to enforce singleton
/** @type {CommunicationClient | null} */
// eslint-disable-next-line import/no-mutable-exports
export var client = null;

/** @type {ReturnType<typeof setTimeout> | undefined} */
var timeout;

/**
 * @param {string} url url
 * @param {{ [handler: string]: (data?: EXPECTED_ANY, params?: EXPECTED_ANY) => EXPECTED_ANY }} handlers handlers
 * @param {number=} reconnect count of reconnections
 */
function socket(url, handlers, reconnect) {
  client = new Client(url);
  client.onOpen(function () {
    retries = 0;
    if (timeout) {
      clearTimeout(timeout);
    }
    if (typeof reconnect !== "undefined") {
      maxRetries = reconnect;
    }
  });
  client.onClose(function () {
    if (retries === 0) {
      handlers.close();
    }

    // Try to reconnect.
    client = null;

    // After 10 retries stop trying, to prevent logspam.
    if (retries < maxRetries) {
      // Exponentially increase timeout to reconnect.
      // Respectfully copied from the package `got`.
      var retryInMs = 1000 * Math.pow(2, retries) + Math.random() * 100;
      retries += 1;
      log.info("Trying to reconnect...");
      timeout = setTimeout(function () {
        socket(url, handlers, reconnect);
      }, retryInMs);
    }
  });
  client.onMessage(
  /**
   * @param {EXPECTED_ANY} data data
   */
  function (data) {
    var message = JSON.parse(data);
    if (handlers[message.type]) {
      handlers[message.type](message.data, message.params);
    }
  });
}
export default socket;