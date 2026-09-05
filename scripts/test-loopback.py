#!/usr/bin/env python3
"""Exercise the client against an unprivileged disposable loopback sshd."""
import getpass
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time


def main():
    sshd = shutil.which("sshd") or "/usr/sbin/sshd"
    if not Path(sshd).exists():
        raise SystemExit("Install OpenSSH server for this integration test")
    with tempfile.TemporaryDirectory(prefix="kervesh-test-") as directory:
        root = Path(directory)
        for name in ("host_key", "client_key"):
            subprocess.run(["ssh-keygen", "-q", "-t", "ed25519", "-N", "fixture-passphrase" if name == "client_key" else "", "-f", str(root / name)], check=True)
        shutil.copyfile(root / "client_key.pub", root / "authorized_keys")
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            port = listener.getsockname()[1]
        config = root / "sshd_config"
        config.write_text(f"""ListenAddress 127.0.0.1
Port {port}
HostKey {root}/host_key
AuthorizedKeysFile {root}/authorized_keys
PidFile {root}/sshd.pid
StrictModes no
PasswordAuthentication no
KbdInteractiveAuthentication no
PubkeyAuthentication yes
UsePAM no
PermitRootLogin yes
AllowUsers {getpass.getuser()}
Subsystem sftp internal-sftp
LogLevel ERROR
""")
        env = os.environ.copy()
        env.update(KERVESH_TEST_PORT=str(port), KERVESH_TEST_KEY=str(root / "client_key"),
                   KERVESH_TEST_USER=getpass.getuser(), KERVESH_TEST_REMOTE_DIR=str(root))
        with (root / "sshd.log").open("w+") as log:
            server = subprocess.Popen([sshd, "-D", "-e", "-f", str(config)], stdout=log, stderr=log)
            try:
                for _ in range(100):
                    if server.poll() is not None:
                        log.seek(0)
                        raise RuntimeError(log.read())
                    try:
                        with socket.create_connection(("127.0.0.1", port), timeout=0.1):
                            break
                    except OSError:
                        time.sleep(0.05)
                else:
                    raise RuntimeError("Disposable sshd failed to listen")
                subprocess.run(["cargo", "test", "-p", "kervesh-ssh", "--test", "loopback", "--", "--ignored", "--nocapture"], env=env, check=True, timeout=180)
            finally:
                server.terminate()
                try:
                    server.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    server.kill()
                    server.wait()
                log.seek(0)
                output = log.read()
                if output:
                    print(output)


if __name__ == "__main__":
    main()
