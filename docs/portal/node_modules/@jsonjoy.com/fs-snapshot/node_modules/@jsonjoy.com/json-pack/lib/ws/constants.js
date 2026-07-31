"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.WsFrameOpcode = void 0;
var WsFrameOpcode;
(function (WsFrameOpcode) {
    // Continuation fragment of a data frame
    WsFrameOpcode[WsFrameOpcode["CONTINUE"] = 0] = "CONTINUE";
    // Data frames
    WsFrameOpcode[WsFrameOpcode["TEXT"] = 1] = "TEXT";
    WsFrameOpcode[WsFrameOpcode["BINARY"] = 2] = "BINARY";
    // Control frames
    // eslint-disable-next-line
    WsFrameOpcode[WsFrameOpcode["MIN_CONTROL_OPCODE"] = 8] = "MIN_CONTROL_OPCODE";
    // eslint-disable-next-line
    WsFrameOpcode[WsFrameOpcode["CLOSE"] = 8] = "CLOSE";
    WsFrameOpcode[WsFrameOpcode["PING"] = 9] = "PING";
    WsFrameOpcode[WsFrameOpcode["PONG"] = 10] = "PONG";
})(WsFrameOpcode || (exports.WsFrameOpcode = WsFrameOpcode = {}));
//# sourceMappingURL=constants.js.map