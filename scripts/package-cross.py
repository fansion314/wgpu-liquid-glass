#!/usr/bin/env python3
"""Package only the self-contained executable and resource license notices."""
import shutil
import sys
import tarfile
import zipfile
from pathlib import Path

root = Path(__file__).resolve().parent.parent
platform = sys.argv[1]
if platform not in ("windows", "linux"):
    raise SystemExit("Usage: package-cross.py windows|linux")
target = "x86_64-pc-windows-gnu" if platform == "windows" else "x86_64-unknown-linux-gnu"
filename = "hello-wgpu.exe" if platform == "windows" else "hello-wgpu"
directory = root / "outputs" / f"{platform}-x86_64"
directory.mkdir(parents=True, exist_ok=True)
shutil.copy2(root / "target" / target / "release" / filename, directory / filename)
shutil.copy2(root / "README.md", directory / "README.md")
shutil.copy2(root / "LICENSE", directory / "LICENSE")
assets = directory / "assets"
assets.mkdir(exist_ok=True)
for name in ("OFL.txt", "SOURCES.md"):
    shutil.copy2(root / "assets" / name, assets / name)
# Enumerate the intended package content so stale files are never shipped.
files = [directory / name for name in (filename, "README.md", "LICENSE")]
files += [assets / name for name in ("OFL.txt", "SOURCES.md")]
if platform == "windows":
    archive = root / "outputs" / "hello-wgpu-windows-x86_64.zip"
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as output:
        for file in files:
            output.write(file, file.relative_to(directory.parent))
else:
    archive = root / "outputs" / "hello-wgpu-linux-x86_64.tar.gz"
    with tarfile.open(archive, "w:gz") as output:
        for file in files:
            output.add(file, arcname=str(file.relative_to(directory.parent)))
print(archive)
