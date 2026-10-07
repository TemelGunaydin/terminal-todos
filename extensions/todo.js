import { spawnSync } from "node:child_process";
import { appSpec, installedApp, installApp } from "./managed-app.js";

function installWithUI(ctx, spec) {
  return ctx.ui.custom((_tui, theme, keybindings, done) => {
    const controller = new AbortController();
    installApp(spec, { signal: controller.signal }).then(
      (binary) => done({ binary }),
      (error) => done({ error, cancelled: controller.signal.aborted }),
    );
    return {
      render: (width) => [
        theme.fg("accent", "Installing Terminal Todos...".slice(0, width)),
        theme.fg("dim", "Esc / Ctrl+C to cancel".slice(0, width)),
      ],
      handleInput: (data) => {
        if (keybindings.matches(data, "tui.select.cancel")) controller.abort();
      },
      invalidate() {},
      dispose() { controller.abort(); },
    };
  });
}

/** Follow Pi's interactive-shell example, including restoration when spawn fails. No shell is involved. */
export function runDashboard(ctx, binary, run = spawnSync) {
  return ctx.ui.custom((tui, _theme, _keybindings, done) => {
    let result;
    try {
      tui.stop();
      process.stdout.write("\x1b[2J\x1b[H");
      result = run(binary, ["tui"], { stdio: "inherit", cwd: ctx.cwd, env: process.env });
    } catch (error) {
      result = { error };
    } finally {
      tui.start();
      tui.requestRender(true);
    }
    done(result);
    return { render: () => [], invalidate() {} };
  });
}

/** @param {import("@earendil-works/pi-coding-agent").ExtensionAPI} pi */
export default function (pi) {
  let busy = false;
  async function open(ctx) {
    if (ctx.mode !== "tui" || !process.stdin.isTTY || !process.stdout.isTTY || process.env.TERM === "dumb") {
      ctx.ui.notify("/todo requires Pi's interactive terminal mode.", "warning");
      return;
    }
    if (busy) {
      ctx.ui.notify("Terminal Todos is already opening or open.", "warning");
      return;
    }
    busy = true;
    try {
      if (ctx.hasPendingMessages()) {
        ctx.ui.notify("Pi has queued messages. Finish them, or restore them with Alt+Up, then run /todo.", "warning");
        return;
      }
      if (!ctx.isIdle()) {
        // Only slash-command contexts can wait safely; shortcuts have no waitForIdle API.
        if (typeof ctx.waitForIdle !== "function") {
          ctx.ui.notify("Pi is still working. Wait for the reply, or press Esc to stop it, then run /todo.", "warning");
          return;
        }
        const wait = await ctx.ui.confirm("Pi is still working",
          "Wait for the current turn to finish, then open Terminal Todos?\nPi will not be interrupted. Queued messages will not be removed.");
        if (!wait) return;
        ctx.ui.notify("Waiting for Pi to finish before opening Terminal Todos...", "info");
        await ctx.waitForIdle();
      }
      if (!ctx.isIdle() || ctx.hasPendingMessages()) {
        ctx.ui.notify("Pi has more work pending. Finish or restore queued messages, then run /todo again.", "warning");
        return;
      }
      const spec = appSpec();
      let binary = await installedApp(spec);
      if (!binary) {
        const approved = await ctx.ui.confirm("Install Terminal Todos?",
          `Download v${spec.version} (${spec.target}) from GitHub and verify SHA-256.\nInstall to ${spec.directory}\nNo Rust required. Homebrew and task data will not be changed by installation.`);
        if (!approved) return;
        const result = await installWithUI(ctx, spec);
        if (result.cancelled) {
          ctx.ui.notify("Terminal Todos installation cancelled.", "info");
          return;
        }
        if (result.error) throw result.error;
        binary = result.binary;
      }
      // A different extension may have started work while the install dialog was open.
      if (!ctx.isIdle() || ctx.hasPendingMessages()) {
        ctx.ui.notify("Terminal Todos is ready, but Pi has more work pending. Finish it, then run /todo again.", "info");
        return;
      }
      const result = await runDashboard(ctx, binary);
      if (result.error) throw result.error;
      if (result.status !== 0 && result.status !== 130 && result.status !== 143) {
        throw new Error(`Terminal Todos exited with ${result.signal || result.status || "an error"}.`);
      }
    } catch (error) {
      ctx.ui.notify(error.message || String(error), "error");
    } finally {
      busy = false;
    }
  }
  pi.registerCommand("todo", {
    description: "Open Terminal Todos (installs with approval on first use)",
    handler: async (args, ctx) => {
      if (args.trim()) {
        ctx.ui.notify("Usage: /todo", "warning");
        return;
      }
      await open(ctx);
    },
  });
  pi.registerShortcut("ctrl+alt+t", { description: "Open Terminal Todos", handler: open });
}
