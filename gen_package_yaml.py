from pathlib import Path
import yaml

IN_DIR = Path("schemas/input")
OUT_PATH = IN_DIR / "package.yaml"

resources = []
for path in sorted(IN_DIR.iterdir()):
    if path.name in ("model.yaml", "package.yaml"):
        continue
    resource = {
        "name": path.stem,
        "path": f"{path.stem}.csv",
        "schema": f"{path.stem}.yaml",
    }
    resources.append(resource)

with OUT_PATH.open("w") as f:
    out = {"name": "muse2-input", "resources": resources}
    yaml.dump(out, f)
