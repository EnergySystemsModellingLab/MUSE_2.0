#!/usr/bin/env python3
#
# A script to generate markdown documentation from table schemas.

from typing import Iterable
import yaml
from .table import fields2table
from pathlib import Path
from dataclasses import dataclass
from jinja2 import Environment


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
    file_order: dict[str, list[str]], schema_dir: Path, env: Environment
) -> str:
    """Generate markdown from Jinja template using metadata in schemas for CSV files."""
    template = env.get_template("csv.md.jinja")
    return template.render(sections=_load_sections(file_order, schema_dir))


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


def format_notes(notes) -> str:
    if isinstance(notes, list):
        items = [add_full_stop(item) for item in notes]
    elif isinstance(notes, str):
        items = [add_full_stop(notes)]
    else:
        return ""
    return "\n".join(f"- {item}" for item in items)
