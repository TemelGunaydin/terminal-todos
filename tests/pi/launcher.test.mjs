import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdtemp, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import todoExtension, { runDashboard } from "../../extensions/todo.js";
import { APP_VERSION, appSpec, installedApp } from "../../extensions/managed-app.js";

function mockUI(options = {}) {
  const calls = [];
  const notifications = [];
  const tui = {
    stop: () => calls.push("stop"), start: () => calls.push("start"),
    requestRender: (force) => calls.push(["render", force]),
  };
  return {
    calls, notifications,
    notify: (...args) => notifications.push(args),
    confirm: async () => { calls.push("confirm"); return options.approve ?? false; },
    custom: (factory) => new Promise((resolve) => {
      let component;
      component = factory(tui, { fg: (_color, text) => text }, {
        matches: (data, action) => action === "tui.select.cancel" && data === "\x1b",
      }, (result) => {
        calls.push("done");
        resolve(result);
        queueMicrotask(() => component?.dispose?.());
      });
      if (component.handleInput && options.cancel) component.handleInput("\x1b");
    }),
  };
}
function register() {
  const commands = new Map();
  const shortcuts = new Map();
  todoExtension({
    registerCommand: (name, command) => commands.set(name, command),
    registerShortcut: (key, shortcut) => shortcuts.set(key, shortcut),
  });
  return { command: commands.get("todo").handler, shortcut: shortcuts.get("ctrl+alt+t").handler, commands, shortcuts };
}
async function interactiveSandbox(t) {
  const root = await mkdtemp(join(tmpdir(), "terminal-todos-launcher-test-"));
  const env = { PI_CODING_AGENT_DIR: process.env.PI_CODING_AGENT_DIR, TERM: process.env.TERM };
  const stdin = Object.getOwnPropertyDescriptor(process.stdin, "isTTY");
  const stdout = Object.getOwnPropertyDescriptor(process.stdout, "isTTY");
  Object.defineProperty(process.stdin, "isTTY", { configurable: true, value: true });
  Object.defineProperty(process.stdout, "isTTY", { configurable: true, value: true });
  process.env.PI_CODING_AGENT_DIR = root;
  process.env.TERM = "xterm-256color";
  t.after(async () => {
    if (stdin) Object.defineProperty(process.stdin, "isTTY", stdin); else delete process.stdin.isTTY;
    if (stdout) Object.defineProperty(process.stdout, "isTTY", stdout); else delete process.stdout.isTTY;
    for (const [key, value] of Object.entries(env)) {
      if (value === undefined) delete process.env[key]; else process.env[key] = value;
    }
    await rm(root, { recursive: true, force: true });
  });
  return { root, context: (ui) => ({ mode: "tui", cwd: root, ui, isIdle: () => true, hasPendingMessages: () => false }) };
}
function mockFetch(t, implementation) {
  const original = globalThis.fetch;
  globalThis.fetch = implementation;
  t.after(() => { globalThis.fetch = original; });
}

test("loading only registers a command and shortcut, with no downloads or setup", (t) => {
  mockFetch(t, () => assert.fail("loading must not download"));
  const { commands, shortcuts } = register();
  assert.deepEqual([...commands.keys()], ["todo"]);
  assert.deepEqual([...shortcuts.keys()], ["ctrl+alt+t"]);
});

test("RPC/print/JSON and real non-TTY contexts never install or take the terminal", async (t) => {
  mockFetch(t, () => assert.fail("non-interactive launch must not fetch"));
  const { command } = register();
  for (const mode of ["rpc", "print", "json", "tui"]) {
    const ui = mockUI({ approve: true });
    await command("", { mode, ui });
    assert.deepEqual(ui.calls, []);
    assert.match(ui.notifications[0][0], /interactive terminal/);
  }
});

test("declined installation and invalid command arguments are side-effect free", async (t) => {
  const { root, context } = await interactiveSandbox(t);
  mockFetch(t, () => assert.fail("declined installation must not fetch"));
  const { command } = register();
  const ui = mockUI();
  await command("", context(ui));
  assert.deepEqual(ui.calls, ["confirm"]);
  assert.deepEqual(await readdir(root), []);
  await command("add not-supported", context(ui));
  assert.match(ui.notifications[0][0], /Usage: \/todo/);
});

test("streaming or queued work refuses both the command and shortcut", async (t) => {
  const { context } = await interactiveSandbox(t);
  const { command, shortcut } = register();
  for (const properties of [{ isIdle: () => false }, { hasPendingMessages: () => true }]) {
    const ui = mockUI({ approve: true });
    const ctx = { ...context(ui), ...properties };
    await command("", ctx);
    await shortcut(ctx);
    assert.deepEqual(ui.calls, []);
    assert.ok(ui.notifications.every(([message]) => /still working|queued messages/.test(message)));
  }
});

test("Escape cancels an approved install; busy state resets for the next invocation", async (t) => {
  const { context } = await interactiveSandbox(t);
  mockFetch(t, () => assert.fail("cancelled before download must not fetch"));
  const { command } = register();
  const ui = mockUI({ approve: true, cancel: true });
  await command("", context(ui));
  assert.match(ui.notifications[0][0], /cancelled/);
  assert.equal(await installedApp(appSpec()), undefined);
  const next = mockUI();
  await command("", context(next));
  assert.deepEqual(next.calls, ["confirm"]);
});

test("double invocation does not open competing dialogs", async (t) => {
  const { context } = await interactiveSandbox(t);
  const { command, shortcut } = register();
  let decline;
  let shown;
  const visible = new Promise((resolve) => { shown = resolve; });
  const ui = mockUI();
  ui.confirm = async () => { shown(); return new Promise((resolve) => { decline = resolve; }); };
  const first = command("", context(ui));
  await visible;
  await shortcut(context(ui));
  assert.match(ui.notifications[0][0], /already opening/);
  decline(false);
  await first;
});

test("declining to wait never interrupts Pi or installs Todo", async (t) => {
  const { context } = await interactiveSandbox(t);
  mockFetch(t, () => assert.fail("declined wait must not fetch"));
  const ui = mockUI();
  await register().command("", { ...context(ui), isIdle: () => false,
    waitForIdle: () => assert.fail("declined wait must not wait"),
    abort: () => assert.fail("Todo must not abort Pi"),
  });
  assert.deepEqual(ui.calls, ["confirm"]);
});

test("an approved busy command waits before first-use approval and resets its busy flag", async (t) => {
  const { context } = await interactiveSandbox(t);
  mockFetch(t, () => assert.fail("must not fetch before install approval"));
  const { command, shortcut } = register();
  const ui = mockUI();
  let confirms = 0;
  ui.confirm = async () => { ui.calls.push("confirm"); return ++confirms === 1; };
  let idle = false, release, waiting;
  const reached = new Promise(resolve => { waiting = resolve; });
  const first = command("", { ...context(ui), isIdle: () => idle,
    waitForIdle: () => { waiting(); return new Promise(resolve => { release = resolve; }); },
    abort: () => assert.fail("Todo must not abort Pi"),
  });
  await reached;
  assert.equal(confirms, 1);
  await shortcut(context(ui));
  assert.ok(ui.notifications.some(([message]) => message.includes("already opening")));
  idle = true; release(); await first;
  assert.equal(confirms, 2, "first-use download approval only happens after idle");
  const retry = mockUI();
  await command("", context(retry));
  assert.deepEqual(retry.calls, ["confirm"]);
});

test("a failed wait reports the error and permits a later invocation", async (t) => {
  const { context } = await interactiveSandbox(t);
  mockFetch(t, () => assert.fail("failed wait must not fetch"));
  const { command } = register();
  const ui = mockUI({ approve: true });
  await command("", { ...context(ui), isIdle: () => false,
    waitForIdle: async () => { throw new Error("fixture wait failed"); },
  });
  assert.ok(ui.notifications.some(([message, kind]) => kind === "error" && message.includes("fixture wait failed")));
  const retry = mockUI();
  await command("", context(retry));
  assert.deepEqual(retry.calls, ["confirm"]);
});

test("queued work appearing while waiting is preserved and blocks installation", async (t) => {
  const { context } = await interactiveSandbox(t);
  mockFetch(t, () => assert.fail("queued work must not fetch"));
  const ui = mockUI({ approve: true });
  let idle = false, pending = false;
  await register().command("", { ...context(ui), isIdle: () => idle,
    hasPendingMessages: () => pending,
    waitForIdle: async () => { idle = true; pending = true; },
    abort: () => assert.fail("Todo must not abort Pi"),
  });
  assert.deepEqual(ui.calls, ["confirm"]);
  assert.ok(ui.notifications.some(([message]) => message.includes("more work pending")));
  assert.equal(pending, true);
});

test("approved install validates in an isolated HOME and cache hits do not prompt/download", async (t) => {
  const { context } = await interactiveSandbox(t);
  // This owned test executable exercises the default version probe, never the user's PATH todo.
  const bytes = Buffer.from(`#!/bin/sh\nif [ "$1" = "--version" ]; then\n  case "$HOME" in */.install-*/probe-home) ;; *) exit 9 ;; esac\n  printf 'todo ${APP_VERSION}\\n'\nfi\n`);
  const hash = createHash("sha256").update(bytes).digest("hex");
  let requests = 0;
  mockFetch(t, async (url) => {
    requests++;
    const spec = appSpec();
    assert.ok(url.startsWith(spec.baseUrl));
    return new Response(url.endsWith(".sha256") ? `${hash}  ${spec.asset}\n` : bytes);
  });
  const { command, shortcut } = register();
  const ui = mockUI({ approve: true });
  await command("", context(ui));
  assert.equal(requests, 2);
  assert.deepEqual(ui.notifications, []);
  assert.equal(await installedApp(appSpec()), appSpec().binary);
  assert.ok(ui.calls.includes("stop") && ui.calls.includes("start"));
  const cached = mockUI();
  await shortcut(context(cached));
  assert.equal(requests, 2);
  assert.ok(!cached.calls.includes("confirm"));
  assert.deepEqual(cached.notifications, []);
});

test("an unavailable release is reported and never hands off the terminal", async (t) => {
  const { context } = await interactiveSandbox(t);
  mockFetch(t, async () => new Response("not found", { status: 404 }));
  const ui = mockUI({ approve: true });
  await register().command("", context(ui));
  assert.match(ui.notifications[0][0], /not published/);
  assert.ok(!ui.calls.includes("stop"));
});

test("native launch uses inherited stdio and no shell, and always restarts/repaints Pi", async () => {
  for (const failure of [false, "spawn", "stop"]) {
    const ui = mockUI();
    if (failure === "stop") {
      const custom = ui.custom;
      ui.custom = (factory) => custom((tui, ...args) => factory({ ...tui, stop: () => {
        ui.calls.push("stop");
        throw new Error("stop failed");
      } }, ...args));
    }
    const ctx = { ui, cwd: "/tmp/path with spaces" };
    const result = await runDashboard(ctx, "/tmp/app with spaces/todo", (binary, args, options) => {
      assert.equal(binary, "/tmp/app with spaces/todo");
      assert.deepEqual(args, ["tui"]);
      assert.equal(options.stdio, "inherit");
      assert.equal(options.cwd, ctx.cwd);
      assert.equal(options.shell, undefined);
      assert.equal(options.env, process.env);
      if (failure === "spawn") throw new Error("spawn failed");
      return { status: 0 };
    });
    assert.deepEqual(ui.calls, ["stop", "start", ["render", true], "done"]);
    if (failure) assert.match(result.error.message, /spawn failed|stop failed/);
    else assert.equal(result.status, 0);
  }
});
