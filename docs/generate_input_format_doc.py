#!/usr/bin/env python3
#
# A script to generate markdown documentation from table schemas.

from pathlib import Path
import sys

from jinja2 import Environment, FileSystemLoader


if __name__ == "__main__":
    DOCS_DIR = Path(__file__).parent
    SCHEMA_DIR = DOCS_DIR.parent / "schemas" / "input"
    FILE_ORDER = {
        "Time slices": ["time_slices"],
        "Regions": ["regions"],
        "Agents": ["agents", "agent_*"],
        "Assets": ["assets"],
        "Commodities": ["commodities", "commodity_levies", "demand", "demand_slicing"],
        "Processes": ["processes", "process_*"],
    }

    sys.path.append(str(DOCS_DIR))
    from format_docs import generate_for_csv

    env = Environment(loader=FileSystemLoader(Path(__file__).parent / "templates"))
    csv_sections = generate_for_csv(FILE_ORDER, SCHEMA_DIR, env)

    template = env.get_template("input_format.md.jinja")
    out = template.render(csv_sections=csv_sections, script_name=Path(__file__).name)

    output_path = DOCS_DIR / "input_format.md"
    output_path.write_text(out, encoding="utf-8")
