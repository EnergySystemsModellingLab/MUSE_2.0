#!/usr/bin/env python3
#
# A script to generate markdown documentation from table schemas.

from typing import Any, Iterable
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
    schema_path: Path, sections: dict[str, list[str]], env: Environment
) -> str:
    """Generate markdown from Jinja template using metadata in table schema for CSV files."""
    template = env.get_template("csv.md.jinja")
    with schema_path.open() as f:
        data = yaml.safe_load(f)

    return template.render(sections=_get_sections(data["resources"], sections))


def _get_sections(
    resources: Iterable[dict[str, Any]], sections: dict[str, list[str]]
) -> Iterable[Section]:
    for title, names in sections.items():
        files = _parse_resources(resources, names)
        yield Section(title, files)


def _parse_resources(
    resources: Iterable[dict[str, Any]], names: Iterable[str]
) -> Iterable[File]:
    for name in names:
        resource = next(res for res in resources if res["name"] == name)

        desc = _add_full_stop(resource["description"])
        if note := resource.get("notes", None):
            note = _format_notes(note)
        table = fields2table(resource["schema"]["fields"])
        yield File(name, desc, table, note)


def _add_full_stop(s: str) -> str:
    s = s.rstrip()
    if s == "" or s.endswith("."):
        return s
    else:
        return f"{s}."


def _format_notes(notes) -> str:
    if isinstance(notes, list):
        items = [_add_full_stop(item) for item in notes]
    elif isinstance(notes, str):
        items = [_add_full_stop(notes)]
    else:
        return ""
    return "\n".join(f"- {item}" for item in items)
