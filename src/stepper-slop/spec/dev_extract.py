"""Development-only extractor. Does not substitute for the required lmt gate."""
import pathlib
import re
import sys

source = pathlib.Path(__file__).with_name("stepper.md")
outputs = {}
for name, fragment in re.findall(r"^```quint ([a-z_]+\.qnt) \+=\n(.*?)^```", source.read_text(), re.M | re.S):
    outputs[name] = outputs.get(name, "") + fragment
if not outputs or len(re.findall(r"^```quint", source.read_text(), re.M)) != len(re.findall(r"^```quint ([a-z_]+\.qnt) \+=", source.read_text(), re.M)):
    raise SystemExit("Every executable fence must name an intended output")
destination = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else source.parent
destination.mkdir(parents=True, exist_ok=True)
for name, text in outputs.items():
    (destination / name).write_text(text)
