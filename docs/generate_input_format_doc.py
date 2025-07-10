#!/usr/bin/env python3
#
# A script to generate markdown documentation from table schemas.

from pathlib import Path
import sys


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
    TEMPLATE_FILE_NAME = "input_format.md.jinja"

    sys.path.append(str(DOCS_DIR))
    from format_docs import generate_markdown

    output_path = DOCS_DIR / "input_format.md"
    output_path.write_text(
        generate_markdown(FILE_ORDER, SCHEMA_DIR, TEMPLATE_FILE_NAME), encoding="utf-8"
    )
