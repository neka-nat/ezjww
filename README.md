# ezjww

`ezjww` is a JWW parser and DXF conversion library.
The core parser/writer is implemented in Rust and exposed to Python with PyO3.

## Current Features

- Validate and parse `.jww` files.
- Read document/header data from Python.
- Convert parsed JWW entities to DXF intermediate entities.
- Write ASCII DXF files.
- Emit DXF handles, `BLOCK_RECORD` table, and `OBJECTS` section for better CAD compatibility.

## Python API

```python
from ezjww import (
    audit,
    bbox,
    is_jww_file,
    readfile,
    plot_jww,
    read_document,
    read_dxf_document,
    report,
    stats,
    to_dxf_string,
    write_dxf,
)

ok = is_jww_file("sample.jww")
doc = read_document("sample.jww")
dxf_doc = read_dxf_document("sample.jww")
dxf_text = to_dxf_string("sample.jww")
write_dxf("sample.jww", "sample.dxf")
plot_jww("sample.jww", save_path="sample.png")

drawing = readfile("sample.jww")
msp = drawing.modelspace()
lines = msp.query("LINE", layer="#lv4")
mix = msp.query('LINE POINT[layer=="#lv4", color==5]')  # ezdxf-like selector
extents = drawing.bbox(explode_inserts=True)
raw_dxf = drawing.to_dxf_string()
dist = drawing.stats()
health = drawing.audit()  # or: audit("sample.jww")
full = report("sample.jww", explode_inserts=True)

# expand INSERT references (nested block aware)
flat = drawing.to_dxf(explode_inserts=True, max_block_nesting=32)
flat_count = len(flat["entities"])
# max_block_nesting must be >= 1
```

## CLI

The package installs the `ezjww` command.

```bash
# show summary
ezjww info jww_samples/Test1.jww

# run health checks
ezjww audit jww_samples/Test1.jww --json

# calculate drawing extents
ezjww bbox jww_samples/Test1.jww --json --explode-inserts

# show entity distribution
ezjww stats jww_samples/Test1.jww --json

# show combined audit+bbox+stats
ezjww report jww_samples/Test1.jww --json --explode-inserts

# convert one file
ezjww to-dxf jww_samples/Test1.jww -o /tmp/Test1.dxf

# convert one file + JSON report
ezjww to-dxf jww_samples/Test1.jww -o /tmp/Test1.dxf --report json

# convert one file with INSERT expansion
ezjww to-dxf jww_samples/Test1.jww -o /tmp/Test1.dxf --explode-inserts

# convert directory (recursive)
ezjww to-dxf-dir jww_samples -o /tmp/dxf_out -r

# render with matplotlib
ezjww plot jww_samples/Test1.jww -o /tmp/Test1.png --explode-inserts
```

## Development

```bash
cargo fmt --all
cargo test
maturin develop
```

## Optional plotting dependency

```bash
pip install "ezjww[plot]"
```
