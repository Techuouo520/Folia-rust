# Fetches and verifies release model weights and the supported Steam Audio runtime from pinned manifests.
import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import sys
import tempfile
import tomllib
import urllib.request
import zipfile


def verified(path, size, digest):
    if not path.is_file() or (size is not None and path.stat().st_size != size):
        return False
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest() == digest.lower()


def fetch(urls, path, size, digest):
    if verified(path, size, digest):
        return path
    path.parent.mkdir(parents=True, exist_ok=True)
    failures = []
    for url in urls:
        temporary = None
        try:
            with tempfile.NamedTemporaryFile(dir=path.parent, prefix=path.name + ".", suffix=".download", delete=False) as output:
                temporary = Path(output.name)
                with urllib.request.urlopen(url, timeout=45) as response:
                    received = 0
                    while block := response.read(1024 * 1024):
                        received += len(block)
                        if size is not None and received > size:
                            raise ValueError("download exceeds its pinned size")
                        output.write(block)
            if not verified(temporary, size, digest):
                raise ValueError("download failed length or SHA256 verification")
            os.replace(temporary, path)
            print(f"Verified {path.name}")
            return path
        except Exception as error:
            failures.append(f"{url}: {error}")
            print(f"Skipped runtime mirror {url}: {error}", file=sys.stderr)
        finally:
            if temporary is not None:
                temporary.unlink(missing_ok=True)
    raise RuntimeError("No verified runtime asset was available\n" + "\n".join(failures))


def copy_asset(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=destination.parent, prefix=destination.name + ".", delete=False) as output:
        temporary = Path(output.name)
    try:
        shutil.copyfile(source, temporary)
        temporary.chmod(0o644)
        os.replace(temporary, destination)
    finally:
        temporary.unlink(missing_ok=True)


def extract_asset(archive, member, destination):
    temporary_path = None
    try:
        with zipfile.ZipFile(archive) as bundle:
            with bundle.open(member) as stream:
                with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as temporary:
                    temporary_path = Path(temporary.name)
                    shutil.copyfileobj(stream, temporary)
            temporary_path.chmod(0o755)
            os.replace(temporary_path, destination)
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)


def models(root, output, cache):
    source = root / "assets" / "models"
    manifest = tomllib.loads((source / "manifest.toml").read_text(encoding="utf-8"))
    directory = output / "models"
    directory.mkdir(parents=True, exist_ok=True)
    for entry in manifest["models"]:
        destination = directory / entry["file"]
        if verified(destination, entry["bytes"], entry["sha256"]):
            continue
        urls = [template.replace("{file}", entry["file"]) for template in manifest["mirrors"]]
        packed = fetch(urls, cache / entry["file"], entry["bytes"], entry["sha256"])
        copy_asset(packed, destination)
    for name in ["manifest.toml", "NOTICE.txt", "LICENSE.beat-this", "LICENSE.demucs"]:
        copy_asset(source / name, directory / name)


def acoustics(root, target, output, cache):
    source = root / "assets" / "acoustics"
    directory = output / "acoustics"
    directory.mkdir(parents=True, exist_ok=True)
    for name in ["SOURCE.txt", "LICENSE.md", "THIRDPARTY.md"]:
        copy_asset(source / name, directory / name)
    member = None
    if target == "x86_64-pc-windows-msvc":
        member = "steamaudio/lib/windows-x64/phonon.dll"
    elif target == "x86_64-unknown-linux-gnu":
        member = "steamaudio/lib/linux-x64/libphonon.so"
    elif target.endswith("apple-darwin"):
        member = "steamaudio/lib/osx/libphonon.dylib"
    if member is None:
        return
    notice = (source / "SOURCE.txt").read_text(encoding="utf-8")
    url = re.search(r"^SDK archive (https://\S+)$", notice, re.MULTILINE)
    digest = re.search(r"^SHA256 ([a-fA-F0-9]{64})$", notice, re.MULTILINE)
    if url is None or digest is None:
        raise ValueError("Steam Audio source notice has no pinned URL and SHA256")
    archive = fetch([url[1]], cache / Path(url[1]).name, None, digest[1])
    extract_asset(archive, member, directory / Path(member).name)


def analysis_runtime(root, target, output, cache):
    source = root / "assets" / "analysis"
    directory = output / "analysis"
    directory.mkdir(parents=True, exist_ok=True)
    for name in ["manifest.toml", "NOTICE.txt", "LICENSE-LLVM.txt"]:
        copy_asset(source / name, directory / name)
    if not target.endswith("pc-windows-msvc"):
        return
    manifest = tomllib.loads((source / "manifest.toml").read_text(encoding="utf-8"))
    entry = manifest["windows"][target.split("-", 1)[0]]
    destination = directory / "dxcompiler.dll"
    if verified(destination, entry["bytes"], entry["sha256"]):
        return
    archive = fetch([manifest["archive_url"]], cache / Path(manifest["archive_url"]).name, manifest["archive_bytes"], manifest["archive_sha256"])
    extract_asset(archive, entry["member"], destination)
    if not verified(destination, entry["bytes"], entry["sha256"]):
        raise ValueError("extracted analysis compiler failed verification")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cache-dir", type=Path, required=True)
    arguments = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    models(root, arguments.output, arguments.cache_dir)
    acoustics(root, arguments.target, arguments.output, arguments.cache_dir)
    analysis_runtime(root, arguments.target, arguments.output, arguments.cache_dir)


if __name__ == "__main__":
    main()
