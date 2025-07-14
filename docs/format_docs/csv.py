#!/usr/bin/env python3
#
# A script to generate markdown documentation from table schemas.

from typing import Iterable
from table2md import MarkdownTable
import yaml
from pathlib import Path
from dataclasses import dataclass
from jinja2 import Environment, FileSystemLoader


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


def generate_for_csv(
    file_order: dict[str, list[str]], schema_dir: Path, template_file_name: str
) -> str:
    """Generate markdown from Jinja template using metadata in schemas for CSV files."""
    env = Environment(loader=FileSystemLoader(Path(__file__).parent))
    template = env.get_template(template_file_name)
    return template.render(
        script_name=Path(__file__).name, sections=_load_sections(file_order, schema_dir)
    )


def _load_sections(
    file_order: dict[str, list[str]], schema_dir: Path
) -> Iterable[Section]:
    for title, patterns in file_order.items():
        paths: list[str] = []
        for pattern in patterns:
            paths.extend(map(str, schema_dir.glob(f"{pattern}.yaml")))
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


def fields2table(fields: list[dict[str, str]]) -> str:
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
    return str(MarkdownTable.from_dicts(data))


def format_notes(notes) -> str:
    if isinstance(notes, list):
        items = [add_full_stop(item) for item in notes]
    elif isinstance(notes, str):
        items = [add_full_stop(notes)]
    else:
        return ""
    return "\n".join(f"- {item}" for item in items)
