import type { AccountAddress, AccountAddressControllerInfo } from "@iroha/iroha-js";
import type {
  AccountAddress as SubpathAccountAddress,
  AccountAddressControllerInfo as SubpathControllerInfo,
} from "@iroha/iroha-js/address";

declare const address: AccountAddress;
declare const subpathAddress: SubpathAccountAddress;
const subpathController: AccountAddressControllerInfo = subpathAddress.controllerInfo();
const rootController: SubpathControllerInfo = address.controllerInfo();
void subpathController;
void rootController;
const controller: AccountAddressControllerInfo = address.controllerInfo();

if (controller.tag === 0) {
  const curve: number = controller.curve;
  controller.publicKey[0] = 1; // Caller-owned bytes are writable copies.
  // @ts-expect-error Snapshot curve metadata is readonly.
  controller.curve = 2;
  // @ts-expect-error Single-key controllers have no multisig member policy.
  controller.members;
  void curve;
} else {
  const version: number = controller.version;
  const threshold: number = controller.threshold;
  // @ts-expect-error Multisig policy metadata is readonly.
  controller.threshold = 1;
  // @ts-expect-error The canonical member sequence is readonly.
  controller.members.push({ curve: 1, weight: 1, publicKey: new Uint8Array(32) });
  const firstMember = controller.members[0];
  if (firstMember !== undefined) {
    firstMember.publicKey[0] = 1; // Nested bytes are also owned copies.
    // @ts-expect-error Member weights are readonly.
    firstMember.weight = 2;
    // @ts-expect-error Member key properties cannot be reassigned.
    firstMember.publicKey = new Uint8Array(32);
  }
  // @ts-expect-error Multisig controllers have no single public key.
  controller.publicKey;
  void version;
  void threshold;
}
// @ts-expect-error The obsolete property is not a public account API.
address._controller;
