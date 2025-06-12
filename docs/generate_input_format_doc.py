#!/usr/bin/env python3
#
# A script to generate markdown documentation from table schemas.

from typing import Iterable
from table2md import MarkdownTable
import yaml
from pathlib import Path
from dataclasses import dataclass
from jinja2 import Environment, FileSystemLoader

_DOCS_DIR = Path(__file__).parent
_SCHEMA_DIR = _DOCS_DIR.parent / "schemas" / "input"
_FILE_ORDER = {
    "Time slices": ["time_slices"],
    "Regions": ["regions"],
    "Agents": ["agents", "agent_*"],
    "Assets": ["assets"],
    "Commodities": ["commodities", "commodity_levies", "demand", "demand_slicing"],
    "Processes": ["processes", "process_*"],
}


@dataclass
class Notes:
    description: str | None
    table: str | None


@dataclass
class File:
    name: str
    description: str
    table: str
    notes: Notes | None


@dataclass
class Section:
    title: str
    files: Iterable[File]


def generate_markdown() -> str:
    """Generate markdown from Jinja template using metadata in schemas."""
    env = Environment(loader=FileSystemLoader(_DOCS_DIR))
    template = env.get_template("input_format.md.jinja")
    return template.render(script_name=Path(__file__).name, sections=load_sections())


def load_sections() -> Iterable[Section]:
    for title, patterns in _FILE_ORDER.items():
        for pattern in patterns:
            paths = map(str, _SCHEMA_DIR.glob(f"{pattern}.yaml"))
            files = (load_file(Path(path)) for path in sorted(paths))
            yield Section(title, files)


def load_file(path: Path) -> File:
    with path.open() as f:
        data = yaml.safe_load(f)

    try:
        table, notes_table = fields2table(data["fields"])
    except KeyError:
        print(f"MISSING VALUE IN {path}")
        raise

    name = f"{path.stem}.csv"
    title = add_full_stop(data["title"])
    if desc := data.get("description", None):
        desc = add_full_stop(desc)
    notes = Notes(desc, notes_table) if desc or notes_table else None
    return File(name, title, table, notes)


def add_full_stop(s: str) -> str:
    s = s.rstrip()
    if s == "" or s.endswith("."):
        return s
    else:
        return f"{s}."


def fields2table(fields: list[dict[str, str]]) -> tuple[str, str | None]:
    data = []
    notes = []
    for f in fields:
        row = {"Field": f"`{f['name']}`", "Description": f["title"]}
        data.append(row)

        if desc := f.get("description", ""):
            # MarkdownTable can't handle newlines, so replace with HTML equivalent
            desc = desc.replace("\n\n", "<br /><br />").replace("\n", " ")
            row = {"Field": f"`{f['name']}`", "Notes": desc}
            notes.append(row)

    data = [
        {
            "Field": f"`{f['name']}`",
            "Description": f["title"],
        }
        for f in fields
    ]

    table = str(MarkdownTable.from_dicts(data))
    notes_table = str(MarkdownTable.from_dicts(notes)) if notes else None
    return table, notes_table


if __name__ == "__main__":
    output_path = _DOCS_DIR / "input_format.md"
    output_path.write_text(generate_markdown(), encoding="utf-8")
