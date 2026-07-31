"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Nlm4Notify = exports.Nlm4Share = exports.Nlm4Lock = exports.Nlm4Holder = void 0;
/**
 * Network Lock Manager (NLM) protocol structures (Appendix II)
 */
/**
 * NLM lock holder structure
 */
class Nlm4Holder {
    constructor(exclusive, svid, oh, offset, length) {
        this.exclusive = exclusive;
        this.svid = svid;
        this.oh = oh;
        this.offset = offset;
        this.length = length;
    }
}
exports.Nlm4Holder = Nlm4Holder;
/**
 * NLM lock structure
 */
class Nlm4Lock {
    constructor(callerName, fh, oh, svid, offset, length) {
        this.callerName = callerName;
        this.fh = fh;
        this.oh = oh;
        this.svid = svid;
        this.offset = offset;
        this.length = length;
    }
}
exports.Nlm4Lock = Nlm4Lock;
/**
 * NLM share structure
 */
class Nlm4Share {
    constructor(callerName, fh, oh, mode, access) {
        this.callerName = callerName;
        this.fh = fh;
        this.oh = oh;
        this.mode = mode;
        this.access = access;
    }
}
exports.Nlm4Share = Nlm4Share;
/**
 * NLM notify structure
 */
class Nlm4Notify {
    constructor(name, state) {
        this.name = name;
        this.state = state;
    }
}
exports.Nlm4Notify = Nlm4Notify;
//# sourceMappingURL=structs.js.map