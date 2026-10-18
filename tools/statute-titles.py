#!/usr/bin/env python3
"""Regenerate crates/legal-citations/registry/statute-titles.txt from an A2AJ
store's statutes (its `document` table, doc_type 'laws', datasets LEGISLATION-*).

    python3 tools/statute-titles.py PATH/TO/a2aj.sqlite

One title per line, English and French, sorted: each statute's name without a
leading article ("The Highway Traffic Act" is read as "Highway Traffic Act", so
a text's "the Highway Traffic Act" and "The Highway Traffic Act" both end on it).
A publisher's citator links these names in the judgments it shows; the citation
engine reads where they end (reference_names.rs). Output is deterministic.
"""

import re
import sqlite3
import sys
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "crates/legal-citations/registry/statute-titles.txt"


def main() -> None:
    store = sqlite3.connect(f"file:{sys.argv[1]}?mode=ro", uri=True)
    titles = set()
    for names in store.execute(
            "select name_en, name_fr from document where doc_type = 'laws' and dataset like 'LEGISLATION-%'"):
        for name in names:
            title = re.sub(r"\s+", " ", name or "").strip()
            # A row that runs its heading and chapter into the name ("2020 LOI DE 2020 … c 2 Loi de 2020 sur …"): the
            # name after the chapter.
            title = re.sub(r"^\d.*?\bc \d+[\w.-]* (?=\S)", "", title)
            title = re.sub(r"^(?:The|the)\s+", "", title)
            letters = [character for character in title if character.isalpha()]
            # A name set in capitals ("ACCESSIBILITY ACT", "LOI SUR L’ADOPTION") in lower case: names are matched
            # without regard to ASCII case, and a text's accented capitals are as rare as its lower-case ones are not.
            if sum(character.isupper() for character in letters) * 2 > len(letters):
                title = title.lower()
            if " " in title and len(title) <= 200 and title[0].isalpha():
                titles.add(title)
    # One spelling of each name, matched without regard to case: its mixed-case one where a store row has it.
    spelled = {}
    for title in sorted(titles):
        if title.lower() not in spelled or title != title.lower():
            spelled[title.lower()] = title
    titles = set(spelled.values())
    OUT.write_text("".join(f"{title}\n" for title in sorted(titles)), encoding="utf-8", newline="\n")
    print(f"{len(titles)} titles -> {OUT}")


if __name__ == "__main__":
    main()
