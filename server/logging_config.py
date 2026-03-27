"""Structured logging configuration for the Aether server.

Provides JSON-formatted logging with request/trace ID correlation.
Uses only Python stdlib — no external dependencies required.

Usage::

    from server.logging_config import setup_logging

    setup_logging(level="INFO", json_enabled=True)

    logger = logging.getLogger("aether-server")
    logger.info("Server started", extra={"actor_count": 42})
    # {"timestamp":"2025-01-01T00:00:00Z","level":"INFO","logger":"aether-server","message":"Server started","actor_count":42}
"""

import json
import logging
import sys
from datetime import datetime, timezone


class JsonFormatter(logging.Formatter):
    """JSON log formatter with structured fields.

    Outputs one JSON object per log line with the following fields:
    - ``timestamp``: ISO 8601 UTC timestamp
    - ``level``: Log level (DEBUG, INFO, WARNING, ERROR, CRITICAL)
    - ``logger``: Logger name
    - ``message``: Log message
    - Any additional fields from ``extra`` on the log record
    """

    def format(self, record: logging.LogRecord) -> str:
        # Build the base log entry
        log_entry = {
            "timestamp": datetime.now(timezone.utc).isoformat(),
            "level": record.levelname,
            "logger": record.name,
            "message": record.getMessage(),
        }

        # Add exception info if present
        if record.exc_info and record.exc_info[0] is not None:
            log_entry["exception"] = self.formatException(record.exc_info)

        # Add any extra fields (skip standard LogRecord attributes)
        reserved = {
            "name", "msg", "args", "created", "relativeCreated",
            "exc_info", "exc_text", "stack_info", "lineno", "funcName",
            "pathname", "filename", "module", "thread", "threadName",
            "process", "processName", "levelname", "levelno", "message",
            "msecs", "taskName",
        }
        for key, value in record.__dict__.items():
            if key not in reserved and not key.startswith("_"):
                log_entry[key] = value

        return json.dumps(log_entry, default=str)


class ColoredFormatter(logging.Formatter):
    """Human-readable colored log formatter for development.

    Uses ANSI color codes for log level highlighting.
    """

    COLORS = {
        "DEBUG": "\033[36m",     # Cyan
        "INFO": "\033[32m",      # Green
        "WARNING": "\033[33m",   # Yellow
        "ERROR": "\033[31m",     # Red
        "CRITICAL": "\033[1;31m", # Bold Red
    }
    RESET = "\033[0m"

    def format(self, record: logging.LogRecord) -> str:
        color = self.COLORS.get(record.levelname, self.RESET)
        timestamp = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%S")
        level = f"{color}{record.levelname:<8}{self.RESET}"
        logger_name = record.name

        # Include request_id/trace_id if present
        request_id = getattr(record, "request_id", None)
        trace_id = getattr(record, "trace_id", None)
        context = ""
        if request_id:
            context = f" [req={request_id}]"
        if trace_id:
            context += f" [trace={trace_id}]"

        message = record.getMessage()

        # Add exception info if present
        if record.exc_info and record.exc_info[0] is not None:
            message += "\n" + self.formatException(record.exc_info)

        return f"{timestamp} {level} {logger_name}{context} {message}"


def setup_logging(
    level: str = "INFO",
    json_enabled: bool = False,
) -> None:
    """Configure structured logging for the Aether server.

    Sets up the ``aether-server`` logger and its children with either
    JSON or colored human-readable output.

    Args:
        level: Log level string (DEBUG, INFO, WARNING, ERROR, CRITICAL).
        json_enabled: If ``True``, output logs as JSON. Otherwise, use
            colored human-readable format.
    """
    log_level = getattr(logging, level.upper(), logging.INFO)

    # Get the aether-server logger
    root_logger = logging.getLogger("aether-server")
    root_logger.setLevel(log_level)

    # Remove any existing handlers
    root_logger.handlers.clear()

    # Create handler
    handler = logging.StreamHandler(sys.stdout)
    handler.setLevel(log_level)

    if json_enabled:
        handler.setFormatter(JsonFormatter())
    else:
        handler.setFormatter(ColoredFormatter())

    root_logger.addHandler(handler)
