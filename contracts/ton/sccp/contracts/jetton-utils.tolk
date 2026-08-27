import "constants"
import "storage"

fun emptyWalletReplayState(): Cell<WalletReplayState> {
    return WalletReplayState {
        mints: [],
        burns: [],
        refunds: [],
    }.toCell();
}

fun calcDeployedJettonWallet(
    ownerAddress: address,
    masterAddress: address,
    jettonWalletCode: cell,
): AutoDeployAddress {
    val emptyWalletStorage: WalletStorage = {
        jettonBalance: 0,
        ownerAddress,
        masterAddress,
        replay: emptyWalletReplayState(),
    };
    return {
        workchain: SCCP_WORKCHAIN,
        stateInit: { code: jettonWalletCode, data: emptyWalletStorage.toCell() },
    };
}

fun calcAddressOfJettonWallet(
    ownerAddress: address,
    masterAddress: address,
    jettonWalletCode: cell,
): address {
    return calcDeployedJettonWallet(
        ownerAddress,
        masterAddress,
        jettonWalletCode,
    ).calculateAddress();
}
