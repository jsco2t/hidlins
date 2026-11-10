#!/usr/bin/env python3
"""Exercise one actual TUI cleanup path on an owned controlling terminal."""

import errno
import fcntl
import os
import pty
import select
import signal
import struct
import sys
import tempfile
import termios
import time


def drain(master, output, timeout):
    if select.select([master], [], [], timeout)[0]:
        try:
            output.extend(os.read(master, 65536))
        except OSError as error:
            if error.errno != errno.EIO:
                raise


def check(binary, action, redirected):
    with tempfile.TemporaryDirectory(prefix="hidlins-tui-pty-") as directory:
        ready_read, ready_write = os.pipe()
        master, slave = pty.openpty()
        pid = os.fork()
        if pid == 0:
            os.close(master)
            os.setsid()
            fcntl.ioctl(slave, termios.TIOCSCTTY, 0)
            for descriptor in (0, 1, 2):
                os.dup2(slave, descriptor)
            os.close(slave)
            os.close(ready_write)
            os.read(ready_read, 1)
            os.close(ready_read)
            if redirected:
                null = os.open(os.devnull, os.O_RDONLY)
                os.dup2(null, 0)
                os.close(null)
            environment = dict(os.environ)
            environment.update(HOME=directory, XDG_CONFIG_HOME=directory, TERM="xterm-256color")
            environment.pop("DISPLAY", None)
            environment.pop("WAYLAND_DISPLAY", None)
            os.execve(binary, [binary, "--no-mouse"], environment)

        os.close(ready_read)
        status = None
        output = bytearray()
        try:
            fcntl.ioctl(master, termios.TIOCSWINSZ, struct.pack("HHHH", 24, 80, 0, 0))
            original = termios.tcgetattr(master)
            os.write(ready_write, b"1")
            os.close(ready_write)
            ready_write = None
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                drain(master, output, 0.01)
                attrs = termios.tcgetattr(master)
                if b"\x1b[?1049h" in output and not attrs[3] & (termios.ICANON | termios.ECHO):
                    break
                ended, candidate = os.waitpid(pid, os.WNOHANG)
                if ended:
                    status = candidate
                    raise AssertionError(f"TUI exited before raw-mode readiness: {output[-400:]!r}")
            else:
                raise AssertionError(f"TUI did not enter raw mode/alternate screen: {output[-400:]!r}")

            # The first-run input screen deliberately shows its editing cursor.
            # Reproduce the hidden-cursor state of a workspace frame on this
            # same owned terminal, without requiring secret-bearing vault data.
            # Pause the child so no redraw can overwrite the precondition.
            os.kill(pid, signal.SIGSTOP)
            stopped, stop_status = os.waitpid(pid, os.WUNTRACED)
            assert stopped == pid and os.WIFSTOPPED(stop_status)
            os.write(slave, b"\x1b[?25l")
            drain(master, output, 0.01)

            if action == "quit":
                os.write(master, b"\x11")  # Ctrl-Q through the actual event loop.
            else:
                os.kill(pid, getattr(signal, "SIG" + action))
            os.kill(pid, signal.SIGCONT)

            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                drain(master, output, 0.01)
                ended, candidate = os.waitpid(pid, os.WNOHANG)
                if ended:
                    status = candidate
                    break
            else:
                raise AssertionError("owned TUI did not exit after the requested action")
            # Read any cleanup bytes queued immediately before process death.
            for _ in range(4):
                drain(master, output, 0.005)

            problems = []
            if action == "quit":
                if not os.WIFEXITED(status) or os.WEXITSTATUS(status) != 0:
                    problems.append(f"normal quit status was {status}")
                if b"terminal I/O error:" in output:
                    problems.append("normal quit reported a terminal input/output error")
            elif not os.WIFSIGNALED(status) or os.WTERMSIG(status) != getattr(signal, "SIG" + action):
                problems.append(f"fatal signal status was {status}")
            if termios.tcgetattr(master) != original:
                problems.append("cooked terminal attributes were not restored")
            if output.rfind(b"\x1b[?25h") < output.rfind(b"\x1b[?25l"):
                problems.append("terminal cursor remained hidden")
            if b"\x1b[?1049l" not in output or b"\x1b[?1000l" not in output:
                problems.append("alternate screen/mouse cleanup was missing")
            if problems:
                raise AssertionError("; ".join(problems))
        finally:
            if ready_write is not None:
                os.close(ready_write)
            if status is None:
                try:
                    os.kill(pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                os.waitpid(pid, 0)
            os.close(master)
            os.close(slave)


if __name__ == "__main__":
    check(sys.argv[1], sys.argv[2], sys.argv[3] == "redirected")
