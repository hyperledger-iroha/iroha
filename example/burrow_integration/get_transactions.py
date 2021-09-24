import os
import binascii
from iroha import IrohaCrypto
from iroha import Iroha, IrohaGrpc
from iroha.primitive_pb2 import can_set_my_account_detail
import sys
from Crypto.Hash import keccak
import integration_helpers
if sys.version_info[0] < 3:
    raise Exception("Python 3 or a more recent version is required.")

# Here is the information about the environment and admin account information:
IROHA_HOST_ADDR = os.getenv("IROHA_HOST_ADDR", "127.0.0.1")
IROHA_PORT = os.getenv("IROHA_PORT", "50051")
ADMIN_ACCOUNT_ID = os.getenv("ADMIN_ACCOUNT_ID", "admin@test")
ADMIN_PRIVATE_KEY = os.getenv(
    "ADMIN_PRIVATE_KEY",
    "f101537e319568c765b2cc89698325604991dca57b9716b58016b253506cab70",
)

iroha = Iroha(ADMIN_ACCOUNT_ID)
net = IrohaGrpc("{}:{}".format(IROHA_HOST_ADDR, IROHA_PORT))

test_private_key = IrohaCrypto.private_key()
test_public_key = IrohaCrypto.derive_public_key(test_private_key).decode("utf-8")

@integration_helpers.trace
def create_contract():
    bytecode = "608060405234801561001057600080fd5b5073a6abc17819738299b3b2c1ce46d55c74f04e290c6000806101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908373ffffffffffffffffffffffffffffffffffffffff1602179055506111dd806100746000396000f3fe608060405234801561001057600080fd5b50600436106100575760003560e01c8063ac2ccd071461005c578063ae44f0c21461008c578063bf010d56146100bc578063d4e804ab146100ec578063d8f7441a1461010a575b600080fd5b610076600480360381019061007191906107b7565b61013a565b6040516100839190610d6a565b60405180910390f35b6100a660048036038101906100a19190610800565b6102a6565b6040516100b39190610d6a565b60405180910390f35b6100d660048036038101906100d19190610907565b61041e565b6040516100e39190610d6a565b60405180910390f35b6100f461059f565b6040516101019190610d4f565b60405180910390f35b610124600480360381019061011f9190610a9d565b6105c3565b6040516101319190610d6a565b60405180910390f35b606060008260405160240161014f9190610d8c565b6040516020818303038152906040527fac2ccd07000000000000000000000000000000000000000000000000000000007bffffffffffffffffffffffffffffffffffffffffffffffffffffffff19166020820180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff8381831617835250505050905060008060008054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16836040516102169190610d38565b600060405180830381855af49150503d8060008114610251576040519150601f19603f3d011682016040523d82523d6000602084013e610256565b606091505b50915091508161029b576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161029290610fa6565b60405180910390fd5b809350505050919050565b6060600086868686866040516024016102c3959493929190610dae565b6040516020818303038152906040527fae44f0c2000000000000000000000000000000000000000000000000000000007bffffffffffffffffffffffffffffffffffffffffffffffffffffffff19166020820180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff8381831617835250505050905060008060008054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168360405161038a9190610d38565b600060405180830381855af49150503d80600081146103c5576040519150601f19603f3d011682016040523d82523d6000602084013e6103ca565b606091505b50915091508161040f576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161040690610fa6565b60405180910390fd5b80935050505095945050505050565b606060008989898989898989604051602401610441989796959493929190610e24565b6040516020818303038152906040527fbf010d56000000000000000000000000000000000000000000000000000000007bffffffffffffffffffffffffffffffffffffffffffffffffffffffff19166020820180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff8381831617835250505050905060008060008054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16836040516105089190610d38565b600060405180830381855af49150503d8060008114610543576040519150601f19603f3d011682016040523d82523d6000602084013e610548565b606091505b50915091508161058d576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161058490610fa6565b60405180910390fd5b80935050505098975050505050505050565b60008054906101000a900473ffffffffffffffffffffffffffffffffffffffff1681565b606060008a8a8a8a8a8a8a8a8a6040516024016105e899989796959493929190610eda565b6040516020818303038152906040527fd8f7441a000000000000000000000000000000000000000000000000000000007bffffffffffffffffffffffffffffffffffffffffffffffffffffffff19166020820180517bffffffffffffffffffffffffffffffffffffffffffffffffffffffff8381831617835250505050905060008060008054906101000a900473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16836040516106af9190610d38565b600060405180830381855af49150503d80600081146106ea576040519150601f19603f3d011682016040523d82523d6000602084013e6106ef565b606091505b509150915081610734576040517f08c379a000000000000000000000000000000000000000000000000000000000815260040161072b90610fa6565b60405180910390fd5b8093505050509998505050505050505050565b600061075a61075584610feb565b610fc6565b90508281526020810184848401111561077657610775611138565b5b610781848285611091565b509392505050565b600082601f83011261079e5761079d611133565b5b81356107ae848260208601610747565b91505092915050565b6000602082840312156107cd576107cc611142565b5b600082013567ffffffffffffffff8111156107eb576107ea61113d565b5b6107f784828501610789565b91505092915050565b600080600080600060a0868803121561081c5761081b611142565b5b600086013567ffffffffffffffff81111561083a5761083961113d565b5b61084688828901610789565b955050602086013567ffffffffffffffff8111156108675761086661113d565b5b61087388828901610789565b945050604086013567ffffffffffffffff8111156108945761089361113d565b5b6108a088828901610789565b935050606086013567ffffffffffffffff8111156108c1576108c061113d565b5b6108cd88828901610789565b925050608086013567ffffffffffffffff8111156108ee576108ed61113d565b5b6108fa88828901610789565b9150509295509295909350565b600080600080600080600080610100898b03121561092857610927611142565b5b600089013567ffffffffffffffff8111156109465761094561113d565b5b6109528b828c01610789565b985050602089013567ffffffffffffffff8111156109735761097261113d565b5b61097f8b828c01610789565b975050604089013567ffffffffffffffff8111156109a05761099f61113d565b5b6109ac8b828c01610789565b965050606089013567ffffffffffffffff8111156109cd576109cc61113d565b5b6109d98b828c01610789565b955050608089013567ffffffffffffffff8111156109fa576109f961113d565b5b610a068b828c01610789565b94505060a089013567ffffffffffffffff811115610a2757610a2661113d565b5b610a338b828c01610789565b93505060c089013567ffffffffffffffff811115610a5457610a5361113d565b5b610a608b828c01610789565b92505060e089013567ffffffffffffffff811115610a8157610a8061113d565b5b610a8d8b828c01610789565b9150509295985092959890939650565b60008060008060008060008060006101208a8c031215610ac057610abf611142565b5b60008a013567ffffffffffffffff811115610ade57610add61113d565b5b610aea8c828d01610789565b99505060208a013567ffffffffffffffff811115610b0b57610b0a61113d565b5b610b178c828d01610789565b98505060408a013567ffffffffffffffff811115610b3857610b3761113d565b5b610b448c828d01610789565b97505060608a013567ffffffffffffffff811115610b6557610b6461113d565b5b610b718c828d01610789565b96505060808a013567ffffffffffffffff811115610b9257610b9161113d565b5b610b9e8c828d01610789565b95505060a08a013567ffffffffffffffff811115610bbf57610bbe61113d565b5b610bcb8c828d01610789565b94505060c08a013567ffffffffffffffff811115610bec57610beb61113d565b5b610bf88c828d01610789565b93505060e08a013567ffffffffffffffff811115610c1957610c1861113d565b5b610c258c828d01610789565b9250506101008a013567ffffffffffffffff811115610c4757610c4661113d565b5b610c538c828d01610789565b9150509295985092959850929598565b610c6c8161105f565b82525050565b6000610c7d8261101c565b610c878185611032565b9350610c978185602086016110a0565b610ca081611147565b840191505092915050565b6000610cb68261101c565b610cc08185611043565b9350610cd08185602086016110a0565b80840191505092915050565b6000610ce782611027565b610cf1818561104e565b9350610d018185602086016110a0565b610d0a81611147565b840191505092915050565b6000610d2260278361104e565b9150610d2d82611158565b604082019050919050565b6000610d448284610cab565b915081905092915050565b6000602082019050610d646000830184610c63565b92915050565b60006020820190508181036000830152610d848184610c72565b905092915050565b60006020820190508181036000830152610da68184610cdc565b905092915050565b600060a0820190508181036000830152610dc88188610cdc565b90508181036020830152610ddc8187610cdc565b90508181036040830152610df08186610cdc565b90508181036060830152610e048185610cdc565b90508181036080830152610e188184610cdc565b90509695505050505050565b6000610100820190508181036000830152610e3f818b610cdc565b90508181036020830152610e53818a610cdc565b90508181036040830152610e678189610cdc565b90508181036060830152610e7b8188610cdc565b90508181036080830152610e8f8187610cdc565b905081810360a0830152610ea38186610cdc565b905081810360c0830152610eb78185610cdc565b905081810360e0830152610ecb8184610cdc565b90509998505050505050505050565b6000610120820190508181036000830152610ef5818c610cdc565b90508181036020830152610f09818b610cdc565b90508181036040830152610f1d818a610cdc565b90508181036060830152610f318189610cdc565b90508181036080830152610f458188610cdc565b905081810360a0830152610f598187610cdc565b905081810360c0830152610f6d8186610cdc565b905081810360e0830152610f818185610cdc565b9050818103610100830152610f968184610cdc565b90509a9950505050505050505050565b60006020820190508181036000830152610fbf81610d15565b9050919050565b6000610fd0610fe1565b9050610fdc82826110d3565b919050565b6000604051905090565b600067ffffffffffffffff82111561100657611005611104565b5b61100f82611147565b9050602081019050919050565b600081519050919050565b600081519050919050565b600082825260208201905092915050565b600081905092915050565b600082825260208201905092915050565b600061106a82611071565b9050919050565b600073ffffffffffffffffffffffffffffffffffffffff82169050919050565b82818337600083830152505050565b60005b838110156110be5780820151818401526020810190506110a3565b838111156110cd576000848401525b50505050565b6110dc82611147565b810181811067ffffffffffffffff821117156110fb576110fa611104565b5b80604052505050565b7f4e487b7100000000000000000000000000000000000000000000000000000000600052604160045260246000fd5b600080fd5b600080fd5b600080fd5b600080fd5b6000601f19601f8301169050919050565b7f4572726f722063616c6c696e67207365727669636520636f6e7472616374206660008201527f756e6374696f6e0000000000000000000000000000000000000000000000000060208201525056fea2646970667358221220d5ebccd22b2486fc09a09162d8f8d21d68acd6ff578bfa975cdd9fce4bcb0fd864736f6c63430008070033"
    """Bytecode was generated using remix editor  https://remix.ethereum.org/ from file get_transactions.sol. """
    tx = iroha.transaction(
        [iroha.command("CallEngine", caller=ADMIN_ACCOUNT_ID, input=bytecode)]
    )
    IrohaCrypto.sign_transaction(tx, ADMIN_PRIVATE_KEY)
    net.send_tx(tx)
    hex_hash = binascii.hexlify(IrohaCrypto.hash(tx))
    for status in net.tx_status_stream(tx):
        print(status)
    return hex_hash

@integration_helpers.trace
def get_account_transactions(address, first_tx_height = None, last_tx_height = None):
    params = integration_helpers.get_first_four_bytes_of_keccak(b'getAccountTransactions(string,string,string,string,string,string,string,string)')
    no_of_param = 8
    for x in range(no_of_param):
        params = params + integration_helpers.left_padded_address_of_param(x, no_of_param)
    params = params + integration_helpers.argument_encoding('admin@test')  # account id
    params = params + integration_helpers.argument_encoding('10')  # page size
    params = params + integration_helpers.argument_encoding('')  # first_tx_hash
    params = params + integration_helpers.argument_encoding('')  # ordering
    params = params + integration_helpers.argument_encoding('')  # first_tx_time
    params = params + integration_helpers.argument_encoding('')  # last_tx_time
    params = params + integration_helpers.argument_encoding(str(last_tx_height))  # first_tx_height
    params = params + integration_helpers.argument_encoding(str(first_tx_height))  # last_tx_height
    tx = iroha.transaction(
        [
            iroha.command(
                "CallEngine", caller=ADMIN_ACCOUNT_ID, callee=address, input=params
            )
        ]
    )
    IrohaCrypto.sign_transaction(tx, ADMIN_PRIVATE_KEY)
    response = net.send_tx(tx)
    for status in net.tx_status_stream(tx):
        print(status)
    hex_hash = binascii.hexlify(IrohaCrypto.hash(tx))
    return hex_hash

@integration_helpers.trace
def get_account_asset_transactions(address, first_tx_time = None, last_tx_time = None):
    params = integration_helpers.get_first_four_bytes_of_keccak(b'getAccountAssetTransactions(string,string,string,string,string,string,string,string,string)')
    no_of_param = 9
    for x in range(no_of_param):
        params = params + integration_helpers.left_padded_address_of_param(x, no_of_param)
    params = params + integration_helpers.argument_encoding(ADMIN_ACCOUNT_ID)  # account id
    params = params + integration_helpers.argument_encoding("coin#test")  # asset id
    params = params + integration_helpers.argument_encoding('10')  # page size
    params = params + integration_helpers.argument_encoding('')  # first_tx_hash
    params = params + integration_helpers.argument_encoding('')  # ordering
    params = params + integration_helpers.argument_encoding('')  # first_tx_time
    params = params + integration_helpers.argument_encoding('')  # last_tx_time
    params = params + integration_helpers.argument_encoding('')  # first_tx_height
    params = params + integration_helpers.argument_encoding('')  # last_tx_height
    tx = iroha.transaction(
        [
            iroha.command(
                "CallEngine", caller=ADMIN_ACCOUNT_ID, callee=address, input=params
            )
        ]
    )
    IrohaCrypto.sign_transaction(tx, ADMIN_PRIVATE_KEY)
    response = net.send_tx(tx)
    print(response)
    for status in net.tx_status_stream(tx):
        print(status)
    hex_hash = binascii.hexlify(IrohaCrypto.hash(tx))
    return hex_hash

@integration_helpers.trace
def get_pending_transactions(address, first_tx_time, last_tx_time):
    params = integration_helpers.get_first_four_bytes_of_keccak(b'getPendingTransactions(string,string,string,string,string)')
    no_of_param = 5
    for x in range(no_of_param):
        params = params + integration_helpers.left_padded_address_of_param(x, no_of_param)
    params = params + integration_helpers.argument_encoding('10')  # page size
    params = params + integration_helpers.argument_encoding('')  # first_tx_hash
    params = params + integration_helpers.argument_encoding('')  # ordering
    params = params + integration_helpers.argument_encoding(str(first_tx_time))  # first_tx_time
    params = params + integration_helpers.argument_encoding(str(last_tx_time))  # last_tx_time
    tx = iroha.transaction(
        [
            iroha.command(
                "CallEngine", caller=ADMIN_ACCOUNT_ID, callee=address, input=params
            )
        ]
    )
    IrohaCrypto.sign_transaction(tx, ADMIN_PRIVATE_KEY)
    response = net.send_tx(tx)
    print(response)
    for status in net.tx_status_stream(tx):
        print(status)
    hex_hash = binascii.hexlify(IrohaCrypto.hash(tx))
    return hex_hash

@integration_helpers.trace
def get_transaction(address, tx_hash: str):
    params = integration_helpers.get_first_four_bytes_of_keccak(b'getTransaction(string)')
    no_of_param = 1
    for x in range(no_of_param):
        params = params + integration_helpers.left_padded_address_of_param(x, no_of_param)
    params = params + integration_helpers.argument_encoding(tx_hash)  # transaction hash 
    tx = iroha.transaction(
        [
            iroha.command(
                "CallEngine", caller=ADMIN_ACCOUNT_ID, callee=address, input=params
            )
        ]
    )
    IrohaCrypto.sign_transaction(tx, ADMIN_PRIVATE_KEY)
    response = net.send_tx(tx)
    print(response)
    for status in net.tx_status_stream(tx):
        print(status)
    hex_hash = binascii.hexlify(IrohaCrypto.hash(tx))
    return hex_hash

@integration_helpers.trace
def send_transaction_and_print_status(transaction):
    hex_hash = binascii.hexlify(IrohaCrypto.hash(transaction))
    print('Transaction hash = {}, creator = {}'.format(
        hex_hash, transaction.payload.reduced_payload.creator_account_id))
    net.send_tx(transaction)
    for status in net.tx_status_stream(transaction):
        print(status)

@integration_helpers.trace
def make_initial_transactions():

    tx = iroha.transaction([
        iroha.command('AddAssetQuantity',
                      asset_id='coin#test', amount='1000.00')
    ])
    IrohaCrypto.sign_transaction(tx, ADMIN_PRIVATE_KEY)
    send_transaction_and_print_status(tx)
    hex_hash = binascii.hexlify(IrohaCrypto.hash(tx))
    # pending transaction
    tx1 = iroha.transaction([
        iroha.command('AddAssetQuantity',
                      asset_id='coin#test', amount='1000.00')
    ],quorum=8)
    IrohaCrypto.sign_transaction(tx1, ADMIN_PRIVATE_KEY)
    send_transaction_and_print_status(tx1)
    tx_tms = tx.payload.reduced_payload.created_time
    first_time, last_time = tx_tms - 1, tx_tms + 1
    first_time_pending = tx1.payload.reduced_payload.created_time-1
    last_time_pending = tx1.payload.reduced_payload.created_time+1
    # transfer assets
    tx1 = iroha.transaction([
        iroha.command('TransferAsset', src_account_id = 'admin@test',
                        dest_account_id = 'test@test', asset_id='coin#test',
                         amount='1000.00')
    ])
    IrohaCrypto.sign_transaction(tx1, ADMIN_PRIVATE_KEY)
    send_transaction_and_print_status(tx1)
    return hex_hash, first_time, last_time, first_time_pending, last_time_pending

tx_hash, first_time, last_time, ft_p, lt_p=make_initial_transactions()
hash = create_contract()
address = integration_helpers.get_engine_receipts_address(hash)
integration_helpers.get_engine_receipts_result(hash)
print('get account transactions results: ')
hash = get_account_transactions(address, 3, 3)
integration_helpers.get_engine_receipts_result(hash)
print('get account asset transactions result')
hash = get_account_asset_transactions(address)
integration_helpers.get_engine_receipts_result(hash)
print('get pending transactions result')
hash = get_pending_transactions(address, ft_p, lt_p)
integration_helpers.get_engine_receipts_result(hash)
print('done')