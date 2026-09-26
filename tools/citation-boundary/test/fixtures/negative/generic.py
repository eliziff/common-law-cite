"""Generic helpers.

Docstrings may quote citations such as R v Oakes, [1986] 1 SCR 103, at para 69.
"""
import re

# NEUTRAL = re.compile(r"\b(?:19|20)\d{2}\s+[A-Z]{2,8}\s+\d+\b")  -- a comment, not code
WHITESPACE = re.compile(r"\s+")
SLUG = re.compile(r"[^a-z0-9]+")
ABBREVIATIONS = re.compile(r"^(?:v|vs|s|ss|no|nos|para|paras|art|arts|cf|ibid|supra|infra|et|al)$", re.I)
WINDOWS_PATH = r"C:\Users\someone\Documents"


def cache_key(value: str) -> str:
    return SLUG.sub("", value.lower())


def page_image(name: str):
    return re.match(r"__\d+_(?P<journal>.+?)_article-(?P<article>[^_]+)_pdf-page-(?P<page>\d+)\.png$", name)


MESSAGE = f"Loaded {len(ABBREVIATIONS.pattern)} abbreviations"
