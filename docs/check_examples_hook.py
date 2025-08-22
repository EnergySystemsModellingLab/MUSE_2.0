#!/usr/bin/env python
#
# This script is run by pre-commit to check that the examples validate correctly with the supplied
# schemas
from collections.abc import Iterable
from typing import Any
import yaml
from frictionless import Package
import sys
from pathlib import Path

ROOT_DIR = Path(__file__).parent.parent
EXAMPLES_DIR = ROOT_DIR / "examples"

# Path to package schema for models
SCHEMA_PATH = ROOT_DIR / "schemas" / "input" / "package.yaml"


def get_examples(paths: Iterable[str]) -> Iterable[Path]:
    seen = set()
    for path in map(Path, paths):
        if not path.name.endswith(".csv"):
            continue
        example = path.parent
        if not example.parent.samefile(EXAMPLES_DIR) or example in seen:
            continue
        seen.add(example)
        yield example


def main(schema: dict[str, Any], data_dirs: Iterable[Path]) -> int:
    ret = 0

    for data_dir in data_dirs:
        print(f"\n🔍 Validating {data_dir}...")

        # Load the schema as a dict and set basepath to data_dir
        package = Package(schema, basepath=str(data_dir))

        # Validate against the schema
        report = package.validate()

        if report.valid:
            print(f"✅ {data_dir} is valid!")
        else:
            ret = 1
            print(f"❌ {data_dir} has errors:")
            for task in report.tasks:
                if task.errors:
                    print(f"\n❌ Errors in {data_dir / task.place}:")
                    for error in task.errors:
                        print(f"   - {error.message}")

    return ret


if __name__ == "__main__":
    with open(SCHEMA_PATH) as f:
        schema = yaml.safe_load(f)
    examples = get_examples(sys.argv[1:])
    sys.exit(main(schema, examples))
