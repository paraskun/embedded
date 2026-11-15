"""Check two clean, independent extractions and checked-in generated-file drift."""
import argparse
import pathlib
import shutil
import subprocess
import sys
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument("--development", action="store_true")
parser.add_argument("--lmt", default="lmt")
args = parser.parse_args()
root = pathlib.Path(__file__).resolve().parent
expected = {"stepper.qnt", "stepper_test.qnt"}
with tempfile.TemporaryDirectory(prefix=".extraction-", dir=root) as temp:
    results = []
    for index in range(2):
        destination = pathlib.Path(temp) / str(index)
        destination.mkdir()
        if args.development:
            subprocess.run([sys.executable, str(root / "dev_extract.py"), str(destination)], check=True)
        else:
            shutil.copyfile(root / "stepper.md", destination / "stepper.md")
            subprocess.run([args.lmt, "stepper.md"], cwd=destination, check=True)
        names = {p.name for p in destination.glob("*.qnt")}
        if names != expected:
            raise SystemExit(f"unexpected generated outputs: {names}")
        results.append({name: (destination / name).read_bytes() for name in names})
    if results[0] != results[1]:
        raise SystemExit("independent extractions differ")
    for name, content in results[0].items():
        if (root / name).read_bytes() != content:
            raise SystemExit(f"generated-file drift: {name}")
print("PASS: clean extraction, deterministic repeat, no generated drift" +
      (" (DEVELOPMENT EXTRACTOR ONLY; lmt gate not checked)" if args.development else " (lmt)"))
