#!/usr/bin/env python3
#
# A script to check that a given file (in TOML format) validates with the given JSON schema (in YAML
# format).
import tomllib
from typing import Iterable
import yaml
from jsonschema.validators import Draft202012Validator
from pathlib import Path
import sys


def main(
    schema_path: Path,
    file_paths: Iterable[Path],
):
    with schema_path.open() as f:
        schema = yaml.safe_load(f)
    Draft202012Validator.check_schema(schema)
    validator = Draft202012Validator(schema=schema)

    error = False
    for file_path in file_paths:
        with file_path.open("rb") as f:
            data = tomllib.load(f)
        try:
            validator.validate(data)
        except Exception as e:
            print(f"Error validating {file_path}: {e}")
            error = True
        else:
            print(f"Validated {file_path} successfully")

    if error:
        sys.exit(1)


if __name__ == "__main__":
    main(Path(sys.argv[1]), map(Path, sys.argv[2:]))
