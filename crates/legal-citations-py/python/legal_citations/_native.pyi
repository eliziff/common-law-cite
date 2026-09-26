__version__: str

def call(method: str, request_json: str) -> tuple[bool, str]:
    """Dispatch one JSON API call; ``(True, response)`` or ``(False, error)``."""
