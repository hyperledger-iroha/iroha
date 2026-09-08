// Shared throwing boundaries preserve native JavaScript error classes and causes
// without repeating constructor sequences in every browser validation branch.
export function rejectType(...arguments_) { throw new TypeError(...arguments_); }
export function rejectRange(...arguments_) { throw new RangeError(...arguments_); }
export function rejectError(...arguments_) { throw new Error(...arguments_); }
