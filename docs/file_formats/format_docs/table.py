from table2md import MarkdownTable


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
