#!/usr/bin/env python3
"""Real Pi -> native Todo -> Pi proof, with isolated HOME/data and fixture-only downloads.

uv run --no-project --with pyte scripts/test_pi_tui.py /absolute/path/to/pi/dist/bundle/cli.js
Optional second argument: a compatible built todo binary (default target/release/todo).
No model calls, real credentials, global Pi settings, releases or user tasks are used.
"""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import tempfile
import termios
import time

from test_tui import Session

ROOT = Path(__file__).resolve().parent.parent
PI = Path(sys.argv[1]).resolve()
BINARY = Path(sys.argv[2] if len(sys.argv) > 2 else ROOT / "target/release/todo").resolve()
VERSION = json.loads((ROOT / "package.json").read_text())["version"]
NODE = shutil.which("node")

# Production URLs are not configurable. Intercept the transport only in this test process.
PRELOAD = r'''
import { appendFileSync, readFileSync } from "node:fs";
import { createHash } from "node:crypto";
const bytes = readFileSync(process.env.FIXTURE_BINARY);
const hash = createHash("sha256").update(bytes).digest("hex");
export function installTransportFixture() {
globalThis.fetch = async (input, { signal } = {}) => {
  const url = String(input);
  appendFileSync(process.env.FIXTURE_REQUESTS, url + "\n");
  const base = `https://github.com/TemelGunaydin/terminal-todos/releases/download/v${process.env.FIXTURE_VERSION}/`;
  if (!url.startsWith(base)) throw new Error(`Unexpected network request: ${url}`);
  if (process.env.FIXTURE_MODE === "hold") {
    return new Promise((resolve, reject) => {
      const abort = () => reject(signal.reason);
      if (signal.aborted) abort(); else signal.addEventListener("abort", abort, { once: true });
    });
  }
  if (process.env.FIXTURE_MODE === "missing") return new Response("not published", { status: 404 });
  if (url.endsWith(".sha256")) {
    const asset = url.slice(base.length, -7);
    const digest = process.env.FIXTURE_MODE === "checksum" ? "0".repeat(64) : hash;
    return new Response(`${digest}  ${asset}\n`);
  }
  return new Response(bytes);
};
}
installTransportFixture();
'''


class PiSession(Session):
    def finish(self):
        self.keys(b"\x15\x04")  # Clear editor, then Ctrl+D exits Pi (never send an LLM prompt).
        deadline = time.monotonic() + 8
        while self.process.poll() is None and time.monotonic() < deadline:
            self.pump()
        assert self.process.wait(timeout=1) == 0, self.text
        self.pump()
        assert termios.tcgetattr(self.slave) == self.original, "Pi did not restore raw mode/echo"
        assert self.raw.count(b"\x1b[?1049h") == self.raw.count(b"\x1b[?1049l"), "Unbalanced alternate screens"
        assert b"\x1b[?25h" in self.raw and b"\x1b[?2004l" in self.raw


def run_case(root, mode="ok", tui_mode="fullscreen"):
    root.mkdir()
    preload = root / "transport.mjs"
    preload.write_text(PRELOAD)
    ready = root / "ready.js"
    # Pi configures its HTTP dispatcher at CLI startup, so reapply the fixture during extension load.
    ready.write_text('import { installTransportFixture } from "./transport.mjs"; export default function (pi) { installTransportFixture(); pi.on("session_start", (_event, ctx) => ctx.ui.setStatus("pi-todo-test", "PI-TODO-READY")); }\n')
    agent = root / "agent"
    agent.mkdir()
    (agent / "settings.json").write_text(json.dumps({"quietStartup": False}))
    # A dummy offline model keeps Pi's editor available without any real credentials or inference.
    (agent / "models.json").write_text(json.dumps({"providers": {"pi-todo-test": {
        "baseUrl": "http://127.0.0.1:1/v1", "api": "openai-completions", "apiKey": "unused-fixture",
        "models": [{"id": "unused", "contextWindow": 128000, "maxTokens": 8192}],
    }}}))
    fake_bin = root / "bin"
    fake_bin.mkdir()
    trap = root / "legacy-was-executed"
    legacy = fake_bin / "todo"
    legacy.write_text(f'#!/bin/sh\ntouch "{trap}"\nexit 1\n')
    legacy.chmod(0o700)
    env = dict(os.environ, HOME=str(root), XDG_DATA_HOME=str(root / "data"),
               PI_CODING_AGENT_DIR=str(agent), PI_OFFLINE="1", PI_SKIP_VERSION_CHECK="1",
               PI_TELEMETRY="0", TERM="xterm-256color", PATH=str(fake_bin) + os.pathsep + os.environ["PATH"],
               FIXTURE_BINARY=str(BINARY), FIXTURE_VERSION=VERSION, FIXTURE_MODE=mode,
               FIXTURE_REQUESTS=str(root / "requests.log"))
    env.pop("NO_COLOR", None)
    # Use only a reviewed explicit package, no personal/project extensions or API credentials.
    command = [NODE, "--import", str(preload), str(PI), "--no-session", "--offline",
               "--no-extensions", "--no-skills", "--no-prompt-templates", "--no-context-files",
               "--no-themes", "--no-approve", "--tui-mode", tui_mode, "--model", "pi-todo-test/unused",
               "-e", str(ROOT), "-e", str(ready)]
    session = PiSession(env, command=command, cwd=root)
    try:
        session.wait(lambda text: "PI-TODO-READY" in text, timeout=15)
    except BaseException:
        session.close()
        raise
    return session, env, trap


def requests(root):
    path = root / "requests.log"
    return path.read_text().splitlines() if path.exists() else []


def launch(session):
    session.keys(b"/todo\r")


def dashboard(session):
    session.wait(lambda text: "TERMINAL TODOS" in text and "TASKS" in text)


def back_to_pi(session, key=b"q"):
    session.keys(key)
    session.wait(lambda text: "TERMINAL TODOS" not in text and "q quit" not in text)
    assert session.process.poll() is None


def native_pid(session):
    output = subprocess.check_output(["ps", "-axo", "pid=,ppid=,comm="], text=True)
    for line in output.splitlines():
        parts = line.split(None, 2)
        if len(parts) == 3 and int(parts[1]) == session.process.pid and Path(parts[2]).name == "todo":
            return int(parts[0])
    raise AssertionError("Native Todo child was not found")


def suite():
    with tempfile.TemporaryDirectory(prefix="terminal-todos-pi-pty-") as temporary:
        base = Path(temporary)
        for tui_mode in ["fullscreen", "regular"]:
            root = base / tui_mode
            session, env, trap = run_case(root, tui_mode=tui_mode)
            try:
                assert not requests(root), "Loading Pi must not fetch anything"
                assert not (root / "agent/tools").exists(), "Loading Pi must not install anything"
                subprocess.run([str(BINARY), "add", "Shared CLI task"], env=env, check=True, capture_output=True)
                data = root / "data/terminal-todos/todos.json"
                original = data.read_bytes()
                launch(session)
                session.wait(lambda text: "Install Terminal Todos?" in text)
                session.keys(b"\x1b")
                session.wait(lambda text: "Install Terminal Todos?" not in text)
                assert not requests(root) and not (root / "agent/tools").exists()
                assert data.read_bytes() == original
                launch(session)
                session.wait(lambda text: "Install Terminal Todos?" in text)
                session.keys(b"\r")
                dashboard(session)
                session.wait(lambda text: "Shared CLI task" in text)
                assert len(requests(root)) == 2
                session.keys(b"a")
                session.wait(lambda text: "ADD TASK" in text)
                session.paste("Added inside Pi")
                session.keys(b"\r")
                session.wait(lambda text: "Added #2" in text)
                back_to_pi(session)
                tasks = json.loads(data.read_text())["tasks"]
                assert [task["title"] for task in tasks] == ["Shared CLI task", "Added inside Pi"]
                cli = subprocess.run([str(BINARY), "list"], env=env, check=True, capture_output=True, text=True)
                assert "Added inside Pi" in cli.stdout
                # Keep unsent editor text through shortcut -> Todo -> Pi.
                session.keys(b"draft preserved")
                session.keys(b"\x1b\x14")  # Legacy Ctrl+Alt+T encoding.
                dashboard(session)
                back_to_pi(session)
                session.wait(lambda text: "draft preserved" in text)
                assert len(requests(root)) == 2, "Cached launch should be offline"
                session.keys(b"\x15")
                launch(session)
                dashboard(session)
                back_to_pi(session, b"\x03")
                for sig in [signal.SIGINT, signal.SIGTERM]:
                    launch(session)
                    dashboard(session)
                    os.kill(native_pid(session), sig)
                    session.wait(lambda text: "TERMINAL TODOS" not in text)
                # A failed native launch must return input ownership and leave data untouched.
                data.write_text("broken data")
                launch(session)
                session.wait(lambda text: "Terminal Todos exited with 1" in text)
                assert data.read_text() == "broken data"
                assert not trap.exists(), "Legacy PATH executable was run"
                session.finish()
            finally:
                session.close()
            print(f"PASS: {tui_mode}: approval/decline, verified install, shared CLI data, shortcut/draft, cached offline launch, Ctrl+C/SIGINT/SIGTERM, child failure, terminal restoration")

        for mode, expected in [("hold", "installation cancelled"), ("checksum", "SHA-256 mismatch"), ("missing", "not published")]:
            root = base / mode
            session, env, trap = run_case(root, mode)
            try:
                launch(session)
                session.wait(lambda text: "Install Terminal Todos?" in text)
                session.keys(b"\r")
                if mode == "hold":
                    session.wait(lambda text: "Installing Terminal Todos..." in text)
                    session.keys(b"\x1b")
                session.wait(lambda text: expected in text)
                assert not list((root / "agent").rglob("receipt.json"))
                assert not (root / "data").exists() and not trap.exists()
                session.finish()
            finally:
                session.close()
            print(f"PASS: {mode}: error/cancellation returns to Pi without installing or touching tasks")


if __name__ == "__main__":
    suite()
