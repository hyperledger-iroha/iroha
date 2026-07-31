"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.Nlm4FreeAllRequest = exports.Nlm4NmLockRequest = exports.Nlm4UnshareRequest = exports.Nlm4ShareResponse = exports.Nlm4ShareRequest = exports.Nlm4ShareArgs = exports.Nlm4GrantedRequest = exports.Nlm4UnlockRequest = exports.Nlm4UnlockArgs = exports.Nlm4CancelRequest = exports.Nlm4CancelArgs = exports.Nlm4Response = exports.Nlm4LockRequest = exports.Nlm4LockArgs = exports.Nlm4TestResponse = exports.Nlm4TestDenied = exports.Nlm4TestRequest = exports.Nlm4TestArgs = void 0;
/**
 * TEST request arguments
 */
class Nlm4TestArgs {
    constructor(cookie, exclusive, lock) {
        this.cookie = cookie;
        this.exclusive = exclusive;
        this.lock = lock;
    }
}
exports.Nlm4TestArgs = Nlm4TestArgs;
/**
 * TEST request
 */
class Nlm4TestRequest {
    constructor(args) {
        this.args = args;
    }
}
exports.Nlm4TestRequest = Nlm4TestRequest;
/**
 * TEST response - denied case
 */
class Nlm4TestDenied {
    constructor(holder) {
        this.holder = holder;
    }
}
exports.Nlm4TestDenied = Nlm4TestDenied;
/**
 * TEST response
 */
class Nlm4TestResponse {
    constructor(cookie, stat, holder) {
        this.cookie = cookie;
        this.stat = stat;
        this.holder = holder;
    }
}
exports.Nlm4TestResponse = Nlm4TestResponse;
/**
 * LOCK request arguments
 */
class Nlm4LockArgs {
    constructor(cookie, block, exclusive, lock, reclaim, state) {
        this.cookie = cookie;
        this.block = block;
        this.exclusive = exclusive;
        this.lock = lock;
        this.reclaim = reclaim;
        this.state = state;
    }
}
exports.Nlm4LockArgs = Nlm4LockArgs;
/**
 * LOCK request
 */
class Nlm4LockRequest {
    constructor(args) {
        this.args = args;
    }
}
exports.Nlm4LockRequest = Nlm4LockRequest;
/**
 * Generic NLM response
 */
class Nlm4Response {
    constructor(cookie, stat) {
        this.cookie = cookie;
        this.stat = stat;
    }
}
exports.Nlm4Response = Nlm4Response;
/**
 * CANCEL request arguments
 */
class Nlm4CancelArgs {
    constructor(cookie, block, exclusive, lock) {
        this.cookie = cookie;
        this.block = block;
        this.exclusive = exclusive;
        this.lock = lock;
    }
}
exports.Nlm4CancelArgs = Nlm4CancelArgs;
/**
 * CANCEL request
 */
class Nlm4CancelRequest {
    constructor(args) {
        this.args = args;
    }
}
exports.Nlm4CancelRequest = Nlm4CancelRequest;
/**
 * UNLOCK request arguments
 */
class Nlm4UnlockArgs {
    constructor(cookie, lock) {
        this.cookie = cookie;
        this.lock = lock;
    }
}
exports.Nlm4UnlockArgs = Nlm4UnlockArgs;
/**
 * UNLOCK request
 */
class Nlm4UnlockRequest {
    constructor(args) {
        this.args = args;
    }
}
exports.Nlm4UnlockRequest = Nlm4UnlockRequest;
/**
 * GRANTED request
 */
class Nlm4GrantedRequest {
    constructor(args) {
        this.args = args;
    }
}
exports.Nlm4GrantedRequest = Nlm4GrantedRequest;
/**
 * SHARE request arguments
 */
class Nlm4ShareArgs {
    constructor(cookie, share, reclaim) {
        this.cookie = cookie;
        this.share = share;
        this.reclaim = reclaim;
    }
}
exports.Nlm4ShareArgs = Nlm4ShareArgs;
/**
 * SHARE request
 */
class Nlm4ShareRequest {
    constructor(args) {
        this.args = args;
    }
}
exports.Nlm4ShareRequest = Nlm4ShareRequest;
/**
 * SHARE response
 */
class Nlm4ShareResponse {
    constructor(cookie, stat, sequence) {
        this.cookie = cookie;
        this.stat = stat;
        this.sequence = sequence;
    }
}
exports.Nlm4ShareResponse = Nlm4ShareResponse;
/**
 * UNSHARE request
 */
class Nlm4UnshareRequest {
    constructor(args) {
        this.args = args;
    }
}
exports.Nlm4UnshareRequest = Nlm4UnshareRequest;
/**
 * NM_LOCK request
 */
class Nlm4NmLockRequest {
    constructor(args) {
        this.args = args;
    }
}
exports.Nlm4NmLockRequest = Nlm4NmLockRequest;
/**
 * FREE_ALL request
 */
class Nlm4FreeAllRequest {
    constructor(notify) {
        this.notify = notify;
    }
}
exports.Nlm4FreeAllRequest = Nlm4FreeAllRequest;
//# sourceMappingURL=messages.js.map