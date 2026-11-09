#!/usr/bin/env python3
"""Run the shipping attachment channel against real GTK on private Unix sockets."""

import os
from pathlib import Path
import shlex
import shutil
import subprocess
import tempfile
import time


def main():
    repo = Path(__file__).resolve().parents[2]
    flutter = shutil.which("flutter")
    broadway = shutil.which("broadwayd")
    if not flutter or not broadway:
        raise SystemExit("Requires the pinned Flutter SDK and GTK3 broadwayd; no tools are installed automatically")
    sdk = Path(flutter).resolve().parent.parent
    engine = sdk / "bin/cache/artifacts/engine/linux-x64"
    if not (engine / "libflutter_linux_gtk.so").is_file():
        raise SystemExit("Requires the installed Flutter Linux x64 engine")
    with tempfile.TemporaryDirectory(prefix="hidlins-gtk-attachment-") as temp:
        root = Path(temp)
        source = root / "source fixture.bin"
        payload = b"Native chooser must return a path without reading these bytes.\x00\xff"
        source.write_bytes(payload)
        destination = root / "saved attachment.bin"
        binary = root / "attachment-test"
        gtk = shlex.split(subprocess.check_output(["pkg-config", "--cflags", "--libs", "gtk+-3.0"], text=True))
        subprocess.run([
            "c++", "-std=c++17", "-Wall", "-Wextra", "-Werror", "-Wno-unused-parameter",
            '-DAPPLICATION_ID="app.hidlins.test"',
            "-I" + str(repo / "app/linux"), "-I" + str(engine),
            str(repo / "tools/linux-native/attachment_source_test.cc"),
            str(repo / "app/linux/flutter/generated_plugin_registrant.cc"),
            "-L" + str(engine), "-Wl,-rpath," + str(engine), "-lflutter_linux_gtk",
            *gtk, "-o", str(binary),
        ], check=True, timeout=60)
        env = dict(os.environ, XDG_RUNTIME_DIR=temp,
                   XDG_CONFIG_HOME=str(root / "config"), XDG_DATA_HOME=str(root / "data"),
                   XDG_CACHE_HOME=str(root / "cache"), GSETTINGS_BACKEND="memory",
                   NO_AT_BRIDGE="1", GDK_BACKEND="broadway", BROADWAY_DISPLAY=":53")
        with (root / "broadway.log").open("w+") as log:
            daemon = subprocess.Popen([broadway, "--unixsocket", str(root / "browser.sock"), ":53"],
                                      env=env, stdout=log, stderr=subprocess.STDOUT)
            try:
                deadline = time.monotonic() + 5
                # GTK uses an abstract Unix socket on Linux. The explicitly
                # requested browser socket is a filesystem readiness signal.
                while not (root / "browser.sock").exists():
                    if daemon.poll() is not None or time.monotonic() >= deadline:
                        log.seek(0)
                        raise RuntimeError("Private GTK display did not start: " + log.read())
                    time.sleep(.02)
                result = subprocess.run([str(binary), str(source), str(destination)], env=env, timeout=45)
                assert source.read_bytes() == payload, "Chooser modified the source file"
                assert not destination.exists(), "Chooser wrote an attachment instead of returning a destination"
                if result.returncode:
                    raise SystemExit(result.returncode)
            finally:
                daemon.terminate()
                try:
                    daemon.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    daemon.kill()
                    daemon.wait(timeout=5)


if __name__ == "__main__":
    main()
