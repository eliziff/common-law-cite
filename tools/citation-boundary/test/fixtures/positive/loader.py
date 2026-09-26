# expect: vendored/corpus
import json
from pathlib import Path

CORPUS = Path(__file__).with_name("data") / "grammar-corpus.json"


def load():
    value = json.loads(CORPUS.read_text())
    if value.get("format") != "legal-grammar-corpus:v1":
        raise ValueError("unsupported")
    return value
