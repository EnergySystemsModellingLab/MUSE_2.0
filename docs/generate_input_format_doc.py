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
class File:
    name: str
    description: str
    table: str
    notes: str | None


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
        paths = []
        for pattern in patterns:
            paths.extend(map(str, _SCHEMA_DIR.glob(f"{pattern}.yaml")))
        files = (load_file(Path(path)) for path in sorted(paths))
        yield Section(title, files)


def load_file(path: Path) -> File:
    with path.open() as f:
        data = yaml.safe_load(f)

    try:
        table = fields2table(data["fields"])
    except KeyError:
        print(f"MISSING VALUE IN {path}")
        raise

    name = f"{path.stem}.csv"
    desc = add_full_stop(data["description"])
    if note := data.get("notes", None):
        note = format_notes(note)
    return File(name, desc, table, note)


def add_full_stop(s: str) -> str:
    s = s.rstrip()
    if s == "" or s.endswith("."):
        return s
    else:
        return f"{s}."


def fields2table(fields: list[dict[str, str]]) -> tuple[str, str | None]:
    data = []
    for f in fields:
        # MarkdownTable can't handle newlines, so replace with HTML equivalent
        notes = f.get("notes", "")
        notes = notes.replace("\n\n", "<br /><br />").replace("\n", " ")
        row = {
            "Field": f"`{f['name']}`",
            "Description": f["description"],
            "Notes": notes,
        }
        data.append(row)
    table = str(MarkdownTable.from_dicts(data))
    return table


def format_notes(notes) -> str:
    if isinstance(notes, list):
        items = [add_full_stop(item) for item in notes]
    elif isinstance(notes, str):
        items = [add_full_stop(notes)]
    else:
        return ""
    return "\n".join(f"- {item}" for item in items)


if __name__ == "__main__":
    output_path = _DOCS_DIR / "input_format.md"
    output_path.write_text(generate_markdown(), encoding="utf-8")
