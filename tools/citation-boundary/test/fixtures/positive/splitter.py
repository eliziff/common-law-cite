# expect: regex/legislation, regex/short-form, regex/pinpoint, regex/history, regex/signal, regex/case-name, regex/reporter, code/citation-function, data/series-table, regex/neutral
import re

_STATUTE_RE = re.compile(
    r"\b(?:RSC|RSO|RSA|RSBC|SC|SO|SA)\b"
    r"\s*[, ]\s*\d{4}(?:\s*,?\s*c\s+[A-Za-z0-9.-]+)?",
    re.I,
)
_REF_TOKEN_RE = re.compile(r"\b(?:supra|ibid)\b", re.I)
_PIN = rf"(?:at\s+(?:paras?\.?|pp?\.?)\s*\d+)"
_HISTORY_RE = re.compile(r"\b(?:aff['’]?d|rev['’]?d|leave\s+to\s+appeal\s+refused)\b", re.I)
_SIGNAL_RE = re.compile(r"^\s*(?:see\s+also|but\s+see|cf\.?|contra|citing|quoting)\s+", re.I)
_CASE_RE = re.compile(r"(?<!\w)(?:R\.?\s+v\.?|Reference\s+re|[A-Z][A-Za-z'’ .&-]{1,70}\s+(?:v\.?|c))\s+")
_JOURNAL_RE = re.compile(r"\(?(?:17|18|19|20)\d{2}\)?\s+\d{1,4}\s+[A-Z][A-Za-z&. -]{1,100}?\s+\d{1,5}\b")
CANLII_RE = re.compile(r"\b(?:17|18|19|20)\d{2}\s+CanLII\s+\d+\b", re.I)

SERIES_JURISDICTIONS = {
    "RSC": "ca", "RSO": "on", "RSBC": "bc", "RSA": "ab", "RSNS": "ns", "RSNB": "nb", "CCSM": "mb",
}


def citation_lookup_key(value: str) -> str:
    return re.sub(r"[^a-z0-9]+", "", value.casefold())
