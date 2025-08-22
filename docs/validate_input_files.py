#!/usr/bin/env python
import yaml
from frictionless import Package
import sys
from pathlib import Path

ROOT_DIR = Path(__file__).parent.parent
EXAMPLES_DIR = ROOT_DIR / "examples"

# Path to package schema for models
schema_path = ROOT_DIR / "schemas" / "input" / "package.yaml"


def main() -> int:
    ret = 0

    with open(schema_path) as f:
        schema_dict = yaml.safe_load(f)

    for data_dir in EXAMPLES_DIR.iterdir():
        if not data_dir.is_dir():
            continue

        print(f"\n🔍 Validating {data_dir}...")

        # Load the schema as a dict and set basepath to data_dir
        package = Package(schema_dict, basepath=str(data_dir))

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
    sys.exit(main())
