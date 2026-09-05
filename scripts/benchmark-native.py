#!/usr/bin/env python3
"""Linux/X11 empty-workspace measurement; creates no persistent user data."""
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]


def ticks(pid):
    fields = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
    return int(fields[11]) + int(fields[12])


def window_for(pid):
    listing = subprocess.run(["xprop", "-root", "_NET_CLIENT_LIST"], capture_output=True, text=True, check=True).stdout
    for window in re.findall(r"0x[0-9a-f]+", listing):
        data = subprocess.run(["xprop", "-id", window, "_NET_WM_PID"], capture_output=True, text=True).stdout
        if re.search(r"= " + str(pid) + r"\b", data):
            return window
    return None


def main():
    binary = ROOT / "target/release/kervesh"
    with tempfile.TemporaryDirectory(prefix="kervesh-benchmark-") as directory:
        env = os.environ.copy()
        env["KERVESH_DATA_DIR"] = directory
        env.pop("WAYLAND_DISPLAY", None)
        env.pop("EFRAME_SCREENSHOT_TO", None)
        env["WINIT_UNIX_BACKEND"] = "x11"
        start = time.monotonic()
        app = subprocess.Popen([str(binary)], env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        try:
            window = None
            while time.monotonic() - start < 15:
                if app.poll() is not None:
                    raise RuntimeError(app.stderr.read().decode())
                window = window_for(app.pid)
                if window:
                    break
                time.sleep(0.05)
            if not window:
                raise RuntimeError("App did not map a native window")
            mapped = time.monotonic() - start
            time.sleep(10)
            before = ticks(app.pid)
            sample_start = time.monotonic()
            time.sleep(5)
            seconds = time.monotonic() - sample_start
            cpu_seconds = (ticks(app.pid) - before) / os.sysconf("SC_CLK_TCK")
            status = Path(f"/proc/{app.pid}/status").read_text()
            rss_kib = int(re.search(r"VmRSS:\s+(\d+)", status)[1])
            output = {"platform": "Linux/X11", "scenario": "empty workspace", "window_mapped_seconds": round(mapped, 3),
                      "rss_mib": round(rss_kib / 1024, 2), "settling_seconds": 10, "idle_sample_seconds": round(seconds, 3),
                      "idle_cpu_percent_one_core": round(cpu_seconds / seconds * 100, 2),
                      "binary_mib": round(binary.stat().st_size / 1024 / 1024, 2)}
            artifact = ROOT / "artifacts/benchmark-empty.json"
            artifact.parent.mkdir(exist_ok=True)
            artifact.write_text(json.dumps(output, indent=2) + "\n")
            print(json.dumps(output, indent=2))
        finally:
            app.terminate()
            try:
                app.wait(timeout=5)
            except subprocess.TimeoutExpired:
                app.kill()
                app.wait()


if __name__ == "__main__":
    main()
