#!/usr/bin/env python3
#
# A script to generate markdown documentation from table schemas.

from pathlib import Path
import sys
from typing import Iterable

from jinja2 import Environment, FileSystemLoader

FILE_FORMAT_DOCS_DIR = Path(__file__).parent
SCHEMA_DIR = FILE_FORMAT_DOCS_DIR.parent.parent / "schemas"

sys.path.append(str(FILE_FORMAT_DOCS_DIR))
from format_docs import generate_for_csv, generate_for_toml  # noqa: E402


def generate_settings_docs(env: Environment) -> None:
    toml_file_name = "settings.toml"
    out = generate_for_toml(SCHEMA_DIR, toml_file_name, env, heading_level=1)
    output_path = FILE_FORMAT_DOCS_DIR / "program_settings.md"
    output_path.write_text(out, encoding="utf-8")


def generate_input_docs(env: Environment) -> None:
    INPUT_SCHEMA_DIR = SCHEMA_DIR / "input"
    FILE_ORDER = {
        "Time slices": ["time_slices"],
        "Regions": ["regions"],
        "Agents": ["agents", "agent_*"],
        "Assets": ["assets"],
        "Commodities": ["commodities", "commodity_levies", "demand", "demand_slicing"],
        "Processes": ["processes", "process_*"],
    }
    csv_sections = generate_for_csv(FILE_ORDER, INPUT_SCHEMA_DIR, env)

    toml_file_name = "model.toml"
    toml_info = generate_for_toml(
        INPUT_SCHEMA_DIR, toml_file_name, env, heading_level=2
    )

    template = env.get_template("input_files.md.jinja")
    out = template.render(
        csv_sections=csv_sections, toml_info=toml_info, script_name=Path(__file__).name
    )

    output_path = FILE_FORMAT_DOCS_DIR / "input_files.md"
    output_path.write_text(out, encoding="utf-8")


generators = {"settings": generate_settings_docs, "input": generate_input_docs}


def main(options: Iterable[str]) -> None:
    env = Environment(loader=FileSystemLoader(FILE_FORMAT_DOCS_DIR / "templates"))

    for option in options:
        try:
            fun = generators[option]
        except KeyError:
            print(f'Unknown option "{option}"')
        else:
            fun(env)


if __name__ == "__main__":
    options: Iterable[str] = sys.argv[1:]
    if not options:
        options = generators.keys()

    main(options)
