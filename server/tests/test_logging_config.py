"""Tests for structured JSON logging."""

import json
import logging
import sys

from server.logging_config import ColoredFormatter, JsonFormatter, setup_logging


class TestJsonFormatter:

    def test_basic_format(self):
        formatter = JsonFormatter()
        record = logging.LogRecord(
            name="aether-server",
            level=logging.INFO,
            pathname="test.py",
            lineno=1,
            msg="Hello world",
            args=(),
            exc_info=None,
        )
        output = formatter.format(record)
        data = json.loads(output)
        assert data["level"] == "INFO"
        assert data["logger"] == "aether-server"
        assert data["message"] == "Hello world"
        assert "timestamp" in data

    def test_extra_fields(self):
        formatter = JsonFormatter()
        record = logging.LogRecord(
            name="aether-server",
            level=logging.INFO,
            pathname="test.py",
            lineno=1,
            msg="Test",
            args=(),
            exc_info=None,
        )
        record.request_id = "abc-123"
        record.custom_field = 42
        output = formatter.format(record)
        data = json.loads(output)
        assert data["request_id"] == "abc-123"
        assert data["custom_field"] == 42

    def test_exception_info(self):
        formatter = JsonFormatter()
        try:
            raise ValueError("test error")
        except ValueError:
            exc_info = sys.exc_info()

        record = logging.LogRecord(
            name="aether-server",
            level=logging.ERROR,
            pathname="test.py",
            lineno=1,
            msg="Something failed",
            args=(),
            exc_info=exc_info,
        )
        output = formatter.format(record)
        data = json.loads(output)
        assert data["level"] == "ERROR"
        assert "exception" in data
        assert "ValueError" in data["exception"]

    def test_valid_json_output(self):
        formatter = JsonFormatter()
        record = logging.LogRecord(
            name="aether-server.test",
            level=logging.DEBUG,
            pathname="test.py",
            lineno=1,
            msg="Debug msg",
            args=(),
            exc_info=None,
        )
        output = formatter.format(record)
        # Should be valid JSON
        json.loads(output)

    def test_timestamp_is_iso_format(self):
        formatter = JsonFormatter()
        record = logging.LogRecord(
            name="aether-server",
            level=logging.INFO,
            pathname="test.py",
            lineno=1,
            msg="Test",
            args=(),
            exc_info=None,
        )
        output = formatter.format(record)
        data = json.loads(output)
        assert "T" in data["timestamp"]  # ISO format contains T


class TestColoredFormatter:

    def test_basic_format(self):
        formatter = ColoredFormatter()
        record = logging.LogRecord(
            name="aether-server",
            level=logging.INFO,
            pathname="test.py",
            lineno=1,
            msg="Hello",
            args=(),
            exc_info=None,
        )
        output = formatter.format(record)
        assert "INFO" in output
        assert "aether-server" in output
        assert "Hello" in output

    def test_includes_request_id(self):
        formatter = ColoredFormatter()
        record = logging.LogRecord(
            name="aether-server",
            level=logging.INFO,
            pathname="test.py",
            lineno=1,
            msg="Test",
            args=(),
            exc_info=None,
        )
        record.request_id = "req-123"
        output = formatter.format(record)
        assert "req=req-123" in output

    def test_includes_trace_id(self):
        formatter = ColoredFormatter()
        record = logging.LogRecord(
            name="aether-server",
            level=logging.INFO,
            pathname="test.py",
            lineno=1,
            msg="Test",
            args=(),
            exc_info=None,
        )
        record.trace_id = "trace-456"
        output = formatter.format(record)
        assert "trace=trace-456" in output

    def test_has_ansi_colors(self):
        formatter = ColoredFormatter()
        record = logging.LogRecord(
            name="aether-server",
            level=logging.ERROR,
            pathname="test.py",
            lineno=1,
            msg="Error!",
            args=(),
            exc_info=None,
        )
        output = formatter.format(record)
        assert "\033[" in output  # ANSI escape sequence


class TestSetupLogging:

    def test_setup_logging_json(self):
        setup_logging(level="DEBUG", json_enabled=True)
        logger = logging.getLogger("aether-server.test_setup_json")
        logger.setLevel(logging.DEBUG)
        assert len(logger.parent.handlers) > 0
        formatter = logger.parent.handlers[0].formatter
        assert isinstance(formatter, JsonFormatter)

    def test_setup_logging_colored(self):
        setup_logging(level="INFO", json_enabled=False)
        logger = logging.getLogger("aether-server.test_setup_colored")
        assert len(logger.parent.handlers) > 0
        formatter = logger.parent.handlers[0].formatter
        assert isinstance(formatter, ColoredFormatter)

    def test_setup_logging_level(self):
        setup_logging(level="WARNING", json_enabled=False)
        logger = logging.getLogger("aether-server")
        assert logger.level == logging.WARNING

    def test_log_output_is_valid_json_when_enabled(self, capsys):
        setup_logging(level="INFO", json_enabled=True)
        logger = logging.getLogger("aether-server.test_json_output")
        logger.info("Structured message", extra={"key": "value"})
        # Flush
        for handler in logger.parent.handlers:
            handler.flush()
        captured = capsys.readouterr()
        if captured.out.strip():
            data = json.loads(captured.out.strip().split("\n")[-1])
            assert data["message"] == "Structured message"
            assert data["key"] == "value"
