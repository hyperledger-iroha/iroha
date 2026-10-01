"""Closed private worker purpose grammar; no Native holder or signer fixture."""
import unittest
from iroha_app_attestation.attestation import AttestationRejected
from iroha_app_attestation.ordinary_worker import _command_path, REQUEST_SCHEMA
from iroha_app_attestation.ordinary_service import RAW_PATH, PATH

class OrdinaryWorkerPurposeTests(unittest.TestCase):
    def test_actual_called_purpose_gate_selects_only_two_exact_routes(self):
        # These are untrusted syntax projections, not admitted parent packets.
        command={"schema":REQUEST_SCHEMA,"request_id":"11"*32,
                 "purpose":"raw_admission","body_base64":""}
        self.assertEqual(_command_path(command),RAW_PATH)
        self.assertEqual(_command_path(dict(command,purpose="credential")),PATH)

    def test_old_missing_unknown_and_path_substitution_are_rejected(self):
        command={"schema":REQUEST_SCHEMA,"request_id":"11"*32,
                 "purpose":"raw_admission","body_base64":""}
        missing=dict(command);del missing["purpose"]
        for malformed in (missing,dict(missing,phase="raw"),dict(missing,phase="credential"),
                          dict(command,purpose="raw"),dict(command,purpose=RAW_PATH),
                          dict(command,path=RAW_PATH),dict(command,authority_public_key="11"*32),
                          dict(command,schema=REQUEST_SCHEMA+"-alternate")):
            with self.assertRaises(AttestationRejected):_command_path(malformed)

if __name__=="__main__":unittest.main()
