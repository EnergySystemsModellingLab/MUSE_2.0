#!/usr/bin/env python

from frictionless import validate, Schema
import sys
from pathlib import Path
import yaml
import argparse

SCHEMA_PATH = Path(__file__).parent / "input" / "assets.yaml"


class Validator:
    def __init__(self, schema_path: Path) -> None:
        with schema_path.open() as f:
            self.schema = Schema(yaml.safe_load(f))

    def validate(self, file_path: Path) -> list[str]:
        report = validate(
            source=str(file_path),
            schema=self.schema,
        )

        errors = []
        if not report.valid:
            for task in report.tasks:
                for error in task.errors:
                    errors.append(error.message)

        return errors


def main() -> int:
    ret = 0

    parser = argparse.ArgumentParser()
    parser.add_argument("--schema", type=Path, required=True)
    parser.add_argument("file_paths", type=Path, nargs=argparse.REMAINDER)
    args = parser.parse_args()

    # Load schema
    v = Validator(args.schema)

    # Process files
    for file_path in args.file_paths:
        errors = v.validate(file_path)
        if not errors:
            print(f"✅ {file_path} is valid!")
        else:
            ret = 1
            print(f"❌ {file_path} has errors:")
            for error in errors:
                print(f"   - {error}")

    return ret


if __name__ == "__main__":
    sys.exit(main())
