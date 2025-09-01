#!/usr/bin/env python

from frictionless import validate, Schema
import sys
from pathlib import Path
import yaml
import argparse

SCHEMA_PATH = Path(__file__).parent / "input" / "assets.yaml"


def process_file(schema: Schema, file_path: Path) -> bool:
    report = validate(
        source=str(file_path),
        schema=schema,
    )

    success = report.valid
    if success:
        print(f"✅ {file_path} is valid!")
    else:
        print(f"❌ {file_path} has errors:")
        for task in report.tasks:
            if task.errors:
                for error in task.errors:
                    print(f"   - {error.message}")

    return success


def main() -> int:
    ret = 0

    parser = argparse.ArgumentParser()
    parser.add_argument("--schema", type=Path, required=True)
    parser.add_argument("file_paths", type=Path, nargs=argparse.REMAINDER)
    args = parser.parse_args()

    with args.schema.open() as f:
        schema = Schema(yaml.safe_load(f))

    for file_path in args.file_paths:
        if not process_file(schema, file_path):
            ret = 1

    return ret


if __name__ == "__main__":
    sys.exit(main())
