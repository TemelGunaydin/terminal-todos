#!/usr/bin/env python3
"""Isolated real-terminal tests: uv run --no-project --with pyte scripts/test_tui.py"""
import codecs
import fcntl
import json
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import sys
import tempfile
import termios
import time

import pyte

BINARY = Path(sys.argv[1] if len(sys.argv) > 1 else "target/debug/todo").resolve()


class Session:
    def __init__(self, env, args=(), columns=120, rows=32):
        self.master, self.slave = pty.openpty()
        self.original = termios.tcgetattr(self.slave)
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        self.screen = pyte.Screen(columns, rows)
        self.stream = pyte.Stream(self.screen)
        self.decoder = codecs.getincrementaldecoder("utf-8")("replace")
        self.raw = b""
        self.process = subprocess.Popen(
            [str(BINARY), *args], stdin=self.slave, stdout=self.slave,
            stderr=self.slave, env=env, start_new_session=True,
        )

    @property
    def text(self):
        return "\n".join(self.screen.display)

    def pump(self, timeout=0.1):
        if select.select([self.master], [], [], timeout)[0]:
            chunk = os.read(self.master, 65536)
            self.raw += chunk
            if b"\x1b[6n" in chunk:
                os.write(self.master, b"\x1b[1;1R")
            self.stream.feed(self.decoder.decode(chunk))

    def wait(self, predicate, timeout=6):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            self.pump()
            if predicate(self.text):
                return
            if self.process.poll() is not None:
                raise AssertionError(f"Exited {self.process.returncode}\n{self.text}")
        raise AssertionError(f"Timed out\n{self.text}")

    def keys(self, keys):
        os.write(self.master, keys)

    def paste(self, text):
        self.keys(b"\x1b[200~" + text.encode() + b"\x1b[201~")

    def resize(self, columns, rows):
        self.screen.resize(lines=rows, columns=columns)
        fcntl.ioctl(self.slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, columns, 0, 0))
        os.kill(self.process.pid, signal.SIGWINCH)

    def finish(self, expected=0, key=b"q", sig=None):
        if sig is None:
            self.keys(key)
        else:
            os.kill(self.process.pid, sig)
        deadline = time.monotonic() + 4
        while self.process.poll() is None and time.monotonic() < deadline:
            self.pump()
        assert self.process.wait(timeout=1) == expected, self.text
        self.pump()
        assert self.raw.count(b"\x1b[?1049h") == 1, "Alternate screen should be entered once"
        assert b"\x1b[?1049l" in self.raw, "Alternate screen not restored"
        assert b"\x1b[?25h" in self.raw, "Cursor not restored"
        assert b"\x1b[?2004l" in self.raw, "Bracketed paste not disabled"
        assert termios.tcgetattr(self.slave) == self.original, "Raw mode/echo not restored"

    def close(self):
        if self.process.poll() is None:
            self.process.kill()
            self.process.wait()
        os.close(self.master)
        os.close(self.slave)


def run_suite():
    with tempfile.TemporaryDirectory(prefix="terminal-todos-tui-") as temporary:
        root = Path(temporary)
        env = dict(os.environ, HOME=str(root), XDG_DATA_HOME=str(root / "data"), TERM="xterm-256color")
        env.pop("NO_COLOR", None)
        data_file = root / "data/terminal-todos/todos.json"

        def cli(*args):
            return subprocess.run([str(BINARY), *args], env=env, check=True, capture_output=True)

        def tasks():
            return json.loads(data_file.read_text())["tasks"]

        for title in ["Plan the Rust migration", "Polish the dashboard", "Write integration tests"]:
            cli("add", title)
        session = Session(env)
        try:
            session.wait(lambda text: "TERMINAL TODOS" in text and "1 / 3" in text)
            cells = [cell for row in session.screen.buffer.values() for cell in row.values()]
            assert any(cell.fg == "a78bfa" for cell in cells), "Expected the violet accent"
            assert any(cell.bg == "111020" for cell in cells), "Expected the midnight-indigo background"
            assert all(cell.fg != "52dbaf" for cell in cells), "The old mint accent must not remain"
            session.keys(b"\x1b[B")
            session.wait(lambda text: "2 / 3" in text)
            session.keys(b"\x1b[F")
            session.wait(lambda text: "3 / 3" in text)
            session.keys(b"a")
            session.wait(lambda text: "ADD TASK" in text)
            session.paste("Türkçe görev 🦀 q")
            session.keys(b"\r")
            session.wait(lambda text: "Added #4" in text and "ADD TASK" not in text)
            assert tasks()[-1]["title"] == "Türkçe görev 🦀 q"
            session.keys(b"a\r")
            session.wait(lambda text: "cannot be empty" in text and "ADD TASK" in text)
            assert len(tasks()) == 4
            session.keys(b"\x1b")
            session.wait(lambda text: "ADD TASK" not in text)
            session.keys(b"e")
            session.wait(lambda text: "EDIT TASK" in text)
            session.keys(b"\x15")  # Ctrl+U
            session.paste("Edited Türkçe task")
            session.keys(b"\r")
            session.wait(lambda text: "Updated #4" in text and "EDIT TASK" not in text)
            assert tasks()[-1]["title"] == "Edited Türkçe task"
            session.keys(b" ")
            session.wait(lambda _: tasks()[-1]["completed"])
            session.keys(b"2")
            session.wait(lambda text: "1 / 1" in text and "Edited Türkçe task" in text)
            session.keys(b" ")
            session.wait(lambda _: not tasks()[-1]["completed"])
            session.keys(b"3/Polish")
            session.wait(lambda text: "SEARCH" in text and "1 / 1" in text)
            session.keys(b"\r")
            session.wait(lambda text: "SEARCH" not in text and "1 / 1" in text)
            session.keys(b"d")
            session.wait(lambda text: "DELETE TASK" in text)
            session.keys(b"n")
            session.wait(lambda text: "DELETE TASK" not in text)
            assert len(tasks()) == 4
            session.keys(b"d\r")
            session.wait(lambda text: "Deleted #2" in text)
            assert [task["id"] for task in tasks()] == [1, 3, 4]
            session.keys(b"\x1b")  # Clear search
            session.wait(lambda text: "1 / 3" in text)
            cli("update", "1", "Changed from the CLI")
            cli("add", "Concurrent CLI task")
            session.wait(lambda text: "Changed from the CLI" in text and "1 / 4" in text)
            session.keys(b" ")
            session.wait(lambda _: tasks()[0]["completed"])
            assert len(tasks()) == 4 and tasks()[-1]["title"] == "Concurrent CLI task"
            Path("/tmp/terminal-todos-wide.txt").write_text(session.text)
            session.resize(80, 24)
            session.wait(lambda text: "DETAILS" in text and "q quit" in text and "Changed from the CLI" in text)
            Path("/tmp/terminal-todos-narrow.txt").write_text(session.text)
            session.resize(60, 20)
            session.wait(lambda text: "DETAILS" in text and "TASKS" in text and "q quit" in text)
            session.resize(40, 10)
            session.wait(lambda text: "Resize the terminal" in text)
            session.finish()
        finally:
            session.close()
        print("PASS: CRUD, Unicode/paste, empty validation, search, delete confirmation, concurrent CLI edits, responsive layout, terminal restoration")

        for key, sig, code in [(b"\x03", None, 130), (None, signal.SIGINT, 130), (None, signal.SIGTERM, 143)]:
            session = Session(env, ["--color", "never"])
            try:
                session.wait(lambda text: "TERMINAL TODOS" in text)
                assert b"\x1b[38;2;" not in session.raw and b"\x1b[48;2;" not in session.raw
                session.finish(code, key=key, sig=sig)
            finally:
                session.close()
        print("PASS: monochrome mode and Ctrl+C/SIGINT/SIGTERM restore terminal state")

        # Paging and details scrolling are exercised separately with a larger list.
        for i in range(30):
            cli("add", f"Task {i:02}")
        session = Session(env)
        try:
            session.wait(lambda text: "1 / 33" in text)
            session.keys(b"\x1b[F")
            session.wait(lambda text: "33 / 33" in text and "Task 29" in text)
            session.keys(b"\x1b[5~")
            session.wait(lambda text: "33 / 33" not in text)
            session.keys(b"\x1b[H")
            session.wait(lambda text: "1 / 33" in text)
            session.resize(80, 24)
            session.wait(lambda text: "DETAILS" in text)
            session.keys(b"\t\x1b[F")
            session.wait(lambda text: "delete with confirmation" in text)
            session.finish()
        finally:
            session.close()
        print("PASS: list paging, Home/End, and focused details scrolling")


if __name__ == "__main__":
    run_suite()
