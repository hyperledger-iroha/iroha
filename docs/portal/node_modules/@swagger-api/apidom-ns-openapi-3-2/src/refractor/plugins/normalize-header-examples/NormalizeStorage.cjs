"use strict";

exports.__esModule = true;
exports.default = void 0;
var _apidomCore = require("@swagger-api/apidom-core");
class NormalizeStorage {
  internalStore;
  constructor(storageElement, storageField, storageSubField) {
    this.storageElement = storageElement;
    this.storageField = storageField;
    this.storageSubField = storageSubField;
  }
  get store() {
    if (!this.internalStore) {
      let rootStore = this.storageElement.get(this.storageField);
      if (!(0, _apidomCore.isObjectElement)(rootStore)) {
        rootStore = new _apidomCore.ObjectElement();
        this.storageElement.set(this.storageField, rootStore);
      }
      let store = rootStore.get(this.storageSubField);
      if (!(0, _apidomCore.isArrayElement)(store)) {
        store = new _apidomCore.ArrayElement();
        rootStore.set(this.storageSubField, store);
      }
      this.internalStore = store;
    }
    return this.internalStore;
  }
  append(pointer) {
    if (!this.includes(pointer)) {
      this.store.push(pointer);
    }
  }
  includes(pointer) {
    return this.store.includes(pointer);
  }
}
var _default = exports.default = NormalizeStorage;