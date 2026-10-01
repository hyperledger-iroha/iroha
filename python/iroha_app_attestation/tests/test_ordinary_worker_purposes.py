"""Closed private worker purpose grammar; no Native holder or signer fixture."""
import unittest
from iroha_app_attestation.attestation import AttestationRejected
from iroha_app_attestation.ordinary_worker import _command_path, REQUEST_SCHEMA
from iroha_app_attestation.ordinary_service import RAW_PATH, PATH, REFRESH_PATH

class OrdinaryWorkerPurposeTests(unittest.TestCase):
    def test_actual_called_phase_gate_selects_only_three_exact_routes(self):
        # These are untrusted syntax projections, not admitted parent packets.
        command={"schema":REQUEST_SCHEMA,"request_id":"11"*32,
                 "phase":"raw","body_base64":""}
        self.assertEqual(_command_path(command),RAW_PATH)
        self.assertEqual(_command_path(dict(command,phase="credential")),PATH)
        self.assertEqual(_command_path(dict(command,phase="refresh")),REFRESH_PATH)

    def test_old_missing_unknown_and_path_substitution_are_rejected(self):
        command={"schema":REQUEST_SCHEMA,"request_id":"11"*32,
                 "phase":"raw","body_base64":""}
        missing=dict(command);del missing["phase"]
        for malformed in (missing,dict(missing,purpose="raw_admission"),dict(missing,purpose="credential"),
                          dict(command,phase="raw_admission"),dict(command,phase=RAW_PATH),
                          dict(command,path=RAW_PATH),dict(command,authority_public_key="11"*32),
                          dict(command,schema=REQUEST_SCHEMA+"-alternate")):
            with self.assertRaises(AttestationRejected):_command_path(malformed)

if __name__=="__main__":unittest.main()
