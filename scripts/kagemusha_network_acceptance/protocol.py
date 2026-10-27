"""Closed controller DATA grammar for the current six native network handoffs.

No proof, credential, consensus, signing or transaction authority is implemented
here. The selected current Rust owners authenticate every referenced original.
Pure functions accept a bounded original-selection callback for synthetic tests.
"""
import re

SCHEMA = 'iroha.kagemusha.real-network-command.v1'
PHASE_FIELDS = {
    'registration': {'scheme_id', 'manifest_digest', 'verifier_pack'},
    'issuer': {'peer_index', 'configuration'},
    'funding': {'target'},
    'activation-retained': {'retention'},
    'settlement': {'exchange', 'target', 'verifier_pack', 'adversarial'},
    'confirmed': {'confirmation'},
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def pin(value):
    require(type(value) is str and re.fullmatch('[0-9a-f]{64}', value)
            and value != '0' * 64, 'nonzero lowercase SHA256 required')
    return value


def command(phase, setup, previous, inputs, select):
    """Construct one exact native command after observing each selected original."""
    require(phase in PHASE_FIELDS, 'unknown monetary phase')
    require(type(inputs) is dict and set(inputs) == PHASE_FIELDS[phase],
            'exact ' + phase + ' command fields required')
    for field, value in inputs.items():
        if field in {'scheme_id', 'manifest_digest'}:
            pin(value)
        elif field == 'peer_index':
            require(type(value) is int and 0 <= value < 4, 'one of four retained validators required')
        elif field == 'adversarial':
            require(type(value) is dict and value.get('kind') in {'none', 'software_bad_sigma'},
                    'explicit adversarial selection required')
            if value['kind'] == 'none':
                require(set(value) == {'kind'}, 'exact no-adversarial selection required')
            else:
                require(set(value) == {'kind', 'claim'}, 'exact software adversarial selection required')
                select(value['claim'])
        else:
            select(value)
    return {'schema': SCHEMA, 'phase': phase, 'setup_sha256': pin(setup),
            'previous_sha256': pin(previous), **inputs}


def issuer_selection(value, setup, registered, select):
    """Consume a new owner-only provisioner handoff bound to this registration."""
    require(type(value) is dict and set(value) == {
        'schema', 'setup_sha256', 'registered_sha256', 'peer_index', 'configuration'}
        and value['schema'] == 'iroha.kagemusha.network-controller-issuer-selection.v1',
        'exact runtime issuer selection required')
    require(value['setup_sha256'] == pin(setup)
            and value['registered_sha256'] == pin(registered), 'issuer selection belongs to another journey')
    inputs = {key: value[key] for key in ('peer_index', 'configuration')}
    command('issuer', setup, registered, inputs, select)
    return inputs


def issuer_composer_input(value, issuer_request, select):
    """Select explicit private composer inputs for this exact controller handoff."""
    require(type(value) is dict and set(value) == {'schema', 'issuer_request', 'request'}
            and value['schema'] == 'iroha.kagemusha.network-controller-issuer-composer-input.v1',
            'exact issuer composer input required')
    require(value['issuer_request'] == issuer_request,
            'issuer composer input belongs to another handoff')
    select(value['request'])
    return value['request']


def registered(value, setup, setup_value, pins, pack):
    """Join source-selected Scheme/pack and the next real certified height."""
    before = setup_value['live_height']
    require(type(before) is int and 2 <= before <= 61, 'setup cut leaves no bounded funding prefix')
    require(value.get('schema') == 'iroha.kagemusha.network-registered.v1'
            and value.get('setup_sha256') == setup
            and value.get('scheme_id') == pins['KAGEMUSHA_WALLET_SCHEME_ID']
            and value.get('manifest_digest') == pins['KAGEMUSHA_WALLET_MANIFEST_DIGEST']
            and value.get('verifier_pack_sha256') == pack['sha256']
            and type(value.get('predecessor_height')) is int and value['predecessor_height'] == before
            and type(value.get('certified_height')) is int and value['certified_height'] == before + 1,
            'registration source or certified height differs')
    pin(value['registration_transaction_hash_hex'])
    pin(value['registration_source_sha256'])


def issuer_ready(value, setup, registered_ref, registered_value, selection, genesis_sha):
    """Join the one actual issuer restart to the unchanged registered history."""
    require(value.get('schema') == 'iroha.kagemusha.network-issuer-ready.v1'
            and value.get('setup_sha256') == setup
            and value.get('registered_sha256') == registered_ref['sha256']
            and value.get('configuration_sha256') == selection['configuration']['sha256']
            and type(value.get('peer_index')) is int and value['peer_index'] == selection['peer_index']
            and type(value.get('certified_height')) is int
            and value['certified_height'] == registered_value['certified_height']
            and value.get('signed_genesis_sha256') == genesis_sha,
            'issuer restart or retained history differs')


def prepared(value, setup, registration, funding_command, target, wire):
    """Join the durable signed activation wire before asking native A to retain it."""
    require(set(value) == {'schema', 'setup_sha256', 'registration_sha256',
            'funding_command_sha256', 'target_sha256', 'transaction_hash_hex', 'signed_transaction'}
            and value['schema'] == 'iroha.kagemusha.network-activation-prepared.v1'
            and value['setup_sha256'] == setup and value['registration_sha256'] == registration
            and value['funding_command_sha256'] == funding_command and value['target_sha256'] == target,
            'activation preparation belongs to another occurrence')
    pin(value['transaction_hash_hex'])
    require(value['signed_transaction'] == {'name': 'funding/activate-signed.wire',
            'bytes': wire['bytes'], 'sha256': wire['sha256']}, 'prepared activation wire differs')


def funding(value, setup, registration_ref, registration_value, target, pins, prepared_ref, acknowledgment, retention, wire):
    """Join Load to exact native retention and dynamic registration/activation heights."""
    height = registration_value['certified_height']
    require(value.get('schema') == 'iroha.kagemusha.network-funded-load.v1'
            and value.get('setup_sha256') == setup
            and value.get('registration_sha256') == registration_ref['sha256']
            and value.get('target_sha256') == target and value.get('source_pins') == pins,
            'funding source bindings differ')
    for name, expected in [('registration_height', height), ('activation_height', height + 1), ('certified_height', height + 2)]:
        require(type(value.get(name)) is int and value[name] == expected and expected <= 64,
                'funding certified height differs: ' + name)
    joined = value['activation_retention']
    for name, expected in [('prepared_sha256', prepared_ref['sha256']),
            ('acknowledgment_sha256', acknowledgment['sha256']), ('retention_sha256', retention['sha256']),
            ('signed_wire_sha256', wire['sha256'])]:
        require(joined.get(name) == expected, 'funding activation custody differs: ' + name)


def funding_handshake(setup, registration_ref, registration_value, issuer_ref, target, pins, publish, wait, observe_wire, retain):
    """Drive funding's nested retention handshake, never submitting activation early.

    Callbacks are existing custody/execution boundaries. Tests use a strict fake
    native owner to reject the retired direct-funding and extra-field protocol.
    """
    sent = publish('funding', issuer_ref['sha256'], target=target)
    prepared_ref, value = wait('activation-prepared.json')
    wire = observe_wire()
    prepared(value, setup, registration_ref['sha256'], sent['sha256'], target['sha256'], wire)
    retention = retain(prepared_ref, wire)
    acknowledgment = publish('activation-retained', prepared_ref['sha256'], retention=retention)
    funded_ref, funded = wait('funded.json')
    funding(funded, setup, registration_ref, registration_value, target['sha256'], pins,
            prepared_ref, acknowledgment, retention, wire)
    return funded_ref, funded, prepared_ref, acknowledgment, retention, wire


def peer_restarts(peer_runs):
    """Four retained stores: DKG restart, one issuer restart, then final restart.

    The current Rust owner verifies which peer is the selected issuer. This
    independent process census verifies the resulting exact startup multiset.
    """
    return len(peer_runs) == 4 and sorted(sorted(runs) for runs in peer_runs.values()) == [
        [1, 2, 3], [1, 2, 3], [1, 2, 3], [1, 2, 3, 4]]
