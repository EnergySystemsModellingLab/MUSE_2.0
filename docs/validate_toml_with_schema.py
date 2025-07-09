#!/usr/bin/env python3
#
# A script to check that a given file (in TOML format) validates with the given JSON schema (in YAML
# format).
import tomllib
import yaml
from jsonschema.validators import Draft202012Validator
from pathlib import Path


def main(file_path: Path, schema_path: Path):
    with file_path.open("rb") as f:
        data = tomllib.load(f)
    with schema_path.open() as f:
        schema = yaml.safe_load(f)

    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema=schema)
    validator.validate(data)

    print("Validated successfully")


if __name__ == "__main__":
    import sys

    main(Path(sys.argv[1]), Path(sys.argv[2]))
