#!/usr/bin/env python

from frictionless import validate, Schema
import sys
from pathlib import Path
import yaml
import argparse
from dataclasses import dataclass

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


@dataclass
class SchemaEntry:
    validator: Validator
    path: Path


class SchemaIndex:
    def __init__(self, index_path: Path) -> None:
        self.entries: list[SchemaEntry] = []

        basedir = index_path.parent
        with index_path.open() as f:
            data = yaml.safe_load(f)
        for entry in data["schemas"]:
            validator = Validator(basedir / entry["include"])
            self.entries.append(SchemaEntry(validator, Path(entry["path"])))

    def validate(self, data_dir: Path) -> dict[Path, list[str]]:
        errors = {}
        for entry in self.entries:
            path = data_dir / entry.path
            cur_errors = entry.validator.validate(path)
            if cur_errors:
                errors[path] = cur_errors
        return errors


def main() -> int:
    ret = 0

    parser = argparse.ArgumentParser()
    parser.add_argument("--schema-index", type=Path, required=True)
    parser.add_argument("data_dirs", type=Path, nargs=argparse.REMAINDER)
    args = parser.parse_args()

    index = SchemaIndex(args.schema_index)
    for data_dir in args.data_dirs:
        errors = index.validate(data_dir)
        if not errors:
            print(f"✅ {data_dir} is valid!")
        else:
            ret = 1
            for file_path, errors in errors.items():
                print(f"❌ {file_path} has errors:")
                for error in errors:
                    print(f"   - {error}")

    return ret


if __name__ == "__main__":
    sys.exit(main())
