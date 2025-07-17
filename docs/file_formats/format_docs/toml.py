from pathlib import Path

import yaml
from .table import fields2table
from jinja2 import Environment
from dataclasses import dataclass


@dataclass
class TOMLInfo:
    heading: str
    description: str
    table: str


def generate_for_toml(
    schema_dir: Path, file_name: str, env: Environment, heading_level: int
) -> str:
    """Generate markdown from Jinja template using metadata in schema for TOML file."""

    file_name_stem, _, _ = file_name.rpartition(".")
    schema_path = schema_dir / f"{file_name_stem}.yaml"

    template = env.get_template("toml.md.jinja")
    toml_info = _load_toml_info(file_name, schema_path)
    return template.render(
        title=toml_info.heading,
        heading_level=heading_level * "#",
        description=toml_info.description,
        table=toml_info.table,
    )


def _load_toml_info(file_name: str, schema_path: Path) -> TOMLInfo:
    with schema_path.open() as f:
        data = yaml.safe_load(f)

    title = data.get("title", None)
    heading = f"{title}: `{file_name}`" if title else f"`{file_name}`"
    assert data["type"] == "object"

    properties = []
    for key, value in data["properties"].items():
        assert value["type"] != "object", "Subsections in TOML files not supported yet"
        value["name"] = key
        properties.append(value)

    table = fields2table(properties)

    return TOMLInfo(heading, data.get("description", ""), table)
