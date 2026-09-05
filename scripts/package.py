#!/usr/bin/env python3
"""Package the already-built native binary without publishing or installing it."""
import argparse
from pathlib import Path
import platform
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib
import zipfile

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--format", choices=["archive", "deb", "rpm", "all"], default="archive")
    args = parser.parse_args()
    version = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    windows = platform.system() == "Windows"
    architecture = "aarch64" if platform.machine().lower() in ("arm64", "aarch64") else "x86_64"
    binary = ROOT / "target" / "release" / ("kervesh.exe" if windows else "kervesh")
    if not binary.is_file():
        raise SystemExit("Run cargo build --release -p kervesh first")
    output = ROOT / "artifacts"
    output.mkdir(exist_ok=True)
    name = f"kervesh-{version}-{'windows' if windows else 'linux'}-{architecture}"
    if args.format in ("archive", "all"):
        if windows:
            with zipfile.ZipFile(output / f"{name}.zip", "w", zipfile.ZIP_DEFLATED) as archive:
                for file in (binary, ROOT / "LICENSE", ROOT / "README.md"):
                    archive.write(file, f"{name}/{file.name}")
        else:
            with tarfile.open(output / f"{name}.tar.gz", "w:gz") as archive:
                for file in (binary, ROOT / "LICENSE", ROOT / "README.md"):
                    archive.add(file, arcname=f"{name}/{file.name}")
        print(f"Created {name} archive")
    if windows and args.format != "archive":
        raise SystemExit("Windows supports archive packaging; use Inno Setup for installer")
    if args.format in ("deb", "all"):
        if not shutil.which("dpkg-deb"):
            raise SystemExit("dpkg-deb is required for Debian packages")
        with tempfile.TemporaryDirectory(prefix="kervesh-deb-") as directory:
            staging = Path(directory)
            install_payload(staging, binary)
            (staging / "DEBIAN").mkdir()
            deb_arch = "arm64" if architecture == "aarch64" else "amd64"
            symbols = subprocess.check_output(["objdump", "-T", str(binary)], text=True)
            versions = [tuple(map(int, match)) for match in re.findall(r"GLIBC_(\d+)\.(\d+)", symbols)]
            if not versions:
                raise SystemExit("Could not determine binary glibc requirement")
            glibc = ".".join(map(str, max(versions)))
            (staging / "DEBIAN" / "control").write_text(f"""Package: kervesh
Version: {version}
Section: net
Priority: optional
Architecture: {deb_arch}
Maintainer: Kernovae contributors
Depends: libc6 (>= {glibc}), libgcc-s1, libx11-6, libxkbcommon0, libxkbcommon-x11-0, libwayland-client0, libegl1, libgl1
Recommends: gnome-keyring | kwalletmanager
Description: Native SSH, SFTP and remote monitoring workspace
 Local-first Rust desktop client with embedded SSH and no cloud requirement.
""")
            subprocess.run(["dpkg-deb", "--root-owner-group", "--build", str(staging), str(output / f"kervesh_{version}_{deb_arch}.deb")], check=True)
    if args.format in ("rpm", "all"):
        if not shutil.which("rpmbuild"):
            raise SystemExit("rpmbuild is required for RPM packages")
        with tempfile.TemporaryDirectory(prefix="kervesh-rpm-") as directory:
            staging = Path(directory)
            for folder in ("BUILD", "BUILDROOT", "RPMS", "SOURCES", "SPECS", "SRPMS", "payload"):
                (staging / folder).mkdir()
            install_payload(staging / "payload", binary)
            spec = staging / "SPECS" / "kervesh.spec"
            spec.write_text(f"""Name: kervesh
Version: {version}
Release: 1
Summary: Native SSH and SFTP workspace
License: MIT
BuildArch: {architecture}
AutoReqProv: yes
%global debug_package %{{nil}}
%description
Local-first native SSH, SFTP and remote monitoring workspace.
%install
mkdir -p %{{buildroot}}
cp -a {staging}/payload/. %{{buildroot}}/
%files
/usr/bin/kervesh
/usr/share/applications/org.kernovae.Kervesh.desktop
/usr/share/icons/hicolor/scalable/apps/org.kernovae.Kervesh.svg
%license /usr/share/doc/kervesh/LICENSE
""")
            subprocess.run(["rpmbuild", "--define", f"_topdir {staging}", "-bb", str(spec)], check=True)
            for rpm in (staging / "RPMS").rglob("*.rpm"):
                shutil.copy2(rpm, output / rpm.name)


def install_payload(staging, binary):
    mapping = {
        binary: "usr/bin/kervesh",
        ROOT / "LICENSE": "usr/share/doc/kervesh/LICENSE",
        ROOT / "packaging/linux/org.kernovae.Kervesh.desktop": "usr/share/applications/org.kernovae.Kervesh.desktop",
        ROOT / "assets/org.kernovae.Kervesh.svg": "usr/share/icons/hicolor/scalable/apps/org.kernovae.Kervesh.svg",
    }
    for source, relative in mapping.items():
        destination = staging / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
    (staging / "usr/bin/kervesh").chmod(0o755)


if __name__ == "__main__":
    main()
