"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.FsEvent = void 0;
class FsEvent {
    constructor(type, steps, node, link, oldSteps = void 0) {
        this.type = type;
        this.steps = steps;
        this.node = node;
        this.link = link;
        this.oldSteps = oldSteps;
    }
}
exports.FsEvent = FsEvent;
//# sourceMappingURL=FsEvent.js.map