#!/usr/bin/env python3
"""Inspect the canonical compiled privacy API without JVM reflection or native loading."""

from __future__ import annotations

import argparse
import importlib.util
import json
from pathlib import Path
import sys


def main() -> int:
    """Execute the existing Kotlin classfile owner's scoped privacy contract."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--classes", required=True, type=Path)
    arguments = parser.parse_args()
    owner = Path(__file__).with_name("check_kotlin_jni.py")
    spec = importlib.util.spec_from_file_location("privacy_jvm_class_contract_owner", owner)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load canonical Kotlin classfile owner")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    try:
        report = module.audit_privacy_classfiles(arguments.classes)
    except (module.AuditError, module.JVM.ClassFileError, OSError) as error:
        print(f"Privacy JVM compiled API contract failed: {error}", file=sys.stderr)
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
