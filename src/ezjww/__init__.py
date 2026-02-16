from __future__ import annotations

import argparse
import json
import math
import re
import sys
from collections import Counter
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from ezjww._core import (
    hello_from_bin,
    is_jww_file,
    read_document,
    read_dxf_document,
    read_dxf_string,
    read_header,
    write_dxf,
)
from ezjww.plot import plot_dxf_document, plot_jww

__all__ = [
    "Drawing",
    "Modelspace",
    "audit",
    "bbox",
    "hello_from_bin",
    "is_jww_file",
    "new",
    "readfile",
    "read_header",
    "read_document",
    "read_dxf_document",
    "read_dxf_string",
    "to_dxf_string",
    "write_dxf",
    "plot_dxf_document",
    "plot_jww",
    "report",
    "stats",
]


@dataclass
class Modelspace:
    entities: list[dict[str, Any]]

    def __iter__(self):
        return iter(self.entities)

    def __len__(self) -> int:
        return len(self.entities)

    def query(
        self,
        entity_type: str | None = None,
        *,
        layer: str | None = None,
        color: int | None = None,
    ) -> list[dict[str, Any]]:
        query_types, query_layer, query_color = _parse_query_selector(entity_type)
        if layer is None:
            layer = query_layer
        if color is None:
            color = query_color

        matched = self.entities
        if query_types is not None:
            matched = [e for e in matched if str(e.get("type", "")).upper() in query_types]
        if layer is not None:
            matched = [e for e in matched if str(e.get("layer", "")) == layer]
        if color is not None:
            matched = [e for e in matched if int(e.get("color", -1)) == color]
        return matched

    def bbox(self) -> dict[str, float | int] | None:
        return _dxf_bbox({"entities": self.entities})

    def stats(self) -> dict[str, Any]:
        return _dxf_stats({"entities": self.entities})


class Drawing:
    def __init__(
        self,
        *,
        source_path: str | None,
        jww_document: dict[str, Any] | None = None,
        dxf_document: dict[str, Any] | None = None,
    ) -> None:
        self._source_path = source_path
        self._jww_document = jww_document
        self._dxf_cache: dict[tuple[bool, int], dict[str, Any]] = {}
        if dxf_document is not None:
            self._dxf_cache[(False, 32)] = dxf_document

    @classmethod
    def from_file(cls, path: str | Path) -> "Drawing":
        source = str(path)
        jww_document = read_document(source)
        return cls(source_path=source, jww_document=jww_document)

    @classmethod
    def new(cls) -> "Drawing":
        return cls(
            source_path=None,
            jww_document=None,
            dxf_document={
                "layers": [],
                "entities": [],
                "blocks": [],
                "unsupported_entities": [],
            },
        )

    @property
    def source_path(self) -> str | None:
        return self._source_path

    @property
    def header(self) -> dict[str, Any] | None:
        if self._jww_document is None:
            return None
        return self._jww_document.get("header")

    @property
    def jww_document(self) -> dict[str, Any] | None:
        return self._jww_document

    def to_dxf(
        self,
        *,
        explode_inserts: bool = False,
        max_block_nesting: int = 32,
    ) -> dict[str, Any]:
        nesting = _normalize_max_block_nesting(max_block_nesting)
        key = (bool(explode_inserts), nesting)
        if key not in self._dxf_cache:
            if self._source_path is None:
                self._dxf_cache[key] = {
                    "layers": [],
                    "entities": [],
                    "blocks": [],
                    "unsupported_entities": [],
                }
            else:
                self._dxf_cache[key] = read_dxf_document(
                    self._source_path,
                    explode_inserts,
                    nesting,
                )
        return self._dxf_cache[key]

    def modelspace(
        self,
        *,
        explode_inserts: bool = False,
        max_block_nesting: int = 32,
    ) -> Modelspace:
        return Modelspace(
            self.to_dxf(
                explode_inserts=explode_inserts,
                max_block_nesting=max_block_nesting,
            ).get("entities", [])
        )

    def audit(
        self,
        *,
        explode_inserts: bool = False,
        max_block_nesting: int = 32,
    ) -> dict[str, Any]:
        dxf = self.to_dxf(
            explode_inserts=explode_inserts,
            max_block_nesting=max_block_nesting,
        )
        unsupported = list(dxf.get("unsupported_entities", []))

        unresolved: list[int] = []
        total_refs = 0
        resolved_refs = 0
        if self._jww_document is not None:
            validation = self._jww_document.get("validation", {})
            unresolved = list(validation.get("unresolved_def_numbers", []))
            total_refs = int(validation.get("total_references", 0))
            resolved_refs = int(validation.get("resolved_references", 0))

        warnings: list[str] = []
        issue_codes: list[str] = []
        if unresolved:
            warnings.append("unresolved block references detected")
            issue_codes.append("UNRESOLVED_BLOCK_REFERENCES")
        if unsupported:
            warnings.append("unsupported entities exist for DXF conversion")
            issue_codes.append("UNSUPPORTED_DXF_ENTITIES")

        return {
            "source_path": self._source_path,
            "total_references": total_refs,
            "resolved_references": resolved_refs,
            "unresolved_def_numbers": unresolved,
            "unresolved_count": len(unresolved),
            "unsupported_entities": unsupported,
            "unsupported_count": len(unsupported),
            "issue_codes": issue_codes,
            "has_issues": bool(unresolved or unsupported),
            "warnings": warnings,
        }

    def bbox(
        self,
        *,
        explode_inserts: bool = True,
        max_block_nesting: int = 32,
    ) -> dict[str, float | int] | None:
        dxf = self.to_dxf(
            explode_inserts=explode_inserts,
            max_block_nesting=max_block_nesting,
        )
        return _dxf_bbox(dxf)

    def stats(
        self,
        *,
        explode_inserts: bool = False,
        max_block_nesting: int = 32,
    ) -> dict[str, Any]:
        dxf = self.to_dxf(
            explode_inserts=explode_inserts,
            max_block_nesting=max_block_nesting,
        )
        return _dxf_stats(dxf)

    def report(
        self,
        *,
        explode_inserts: bool = False,
        max_block_nesting: int = 32,
    ) -> dict[str, Any]:
        nesting = _normalize_max_block_nesting(max_block_nesting)
        dxf = self.to_dxf(
            explode_inserts=explode_inserts,
            max_block_nesting=nesting,
        )
        return {
            "source_path": self._source_path,
            "explode_inserts": bool(explode_inserts),
            "max_block_nesting": int(nesting),
            "audit": self.audit(
                explode_inserts=explode_inserts,
                max_block_nesting=nesting,
            ),
            "bbox": _dxf_bbox(dxf),
            "stats": _dxf_stats(dxf),
        }

    def to_dxf_string(
        self,
        *,
        explode_inserts: bool = False,
        max_block_nesting: int = 32,
    ) -> str:
        if self._source_path is None:
            raise ValueError(
                "to_dxf_string() requires a source-backed drawing. use readfile(path)."
            )
        nesting = _normalize_max_block_nesting(max_block_nesting)
        return read_dxf_string(
            self._source_path,
            explode_inserts,
            nesting,
        )

    def saveas(
        self,
        output_path: str | Path,
        *,
        explode_inserts: bool = False,
        max_block_nesting: int = 32,
    ) -> None:
        if self._source_path is None:
            raise ValueError("saveas() requires a source-backed drawing. use readfile(path).")
        nesting = _normalize_max_block_nesting(max_block_nesting)
        write_dxf(
            self._source_path,
            str(output_path),
            explode_inserts,
            nesting,
        )

    def plot(
        self,
        *,
        explode_inserts: bool = True,
        max_block_nesting: int = 32,
        **kwargs: Any,
    ):
        nesting = _normalize_max_block_nesting(max_block_nesting)
        return plot_dxf_document(
            self.to_dxf(
                explode_inserts=explode_inserts,
                max_block_nesting=nesting,
            ),
            **kwargs,
        )


def readfile(path: str | Path) -> Drawing:
    return Drawing.from_file(path)


def new() -> Drawing:
    return Drawing.new()


def audit(
    path: str | Path,
    *,
    explode_inserts: bool = False,
    max_block_nesting: int = 32,
) -> dict[str, Any]:
    return readfile(path).audit(
        explode_inserts=explode_inserts,
        max_block_nesting=max_block_nesting,
    )


def bbox(
    path: str | Path,
    *,
    explode_inserts: bool = True,
    max_block_nesting: int = 32,
) -> dict[str, float | int] | None:
    return readfile(path).bbox(
        explode_inserts=explode_inserts,
        max_block_nesting=max_block_nesting,
    )


def to_dxf_string(
    path: str | Path,
    *,
    explode_inserts: bool = False,
    max_block_nesting: int = 32,
) -> str:
    nesting = _normalize_max_block_nesting(max_block_nesting)
    return read_dxf_string(
        str(path),
        explode_inserts,
        nesting,
    )


def stats(
    path: str | Path,
    *,
    explode_inserts: bool = False,
    max_block_nesting: int = 32,
) -> dict[str, Any]:
    return readfile(path).stats(
        explode_inserts=explode_inserts,
        max_block_nesting=max_block_nesting,
    )


def report(
    path: str | Path,
    *,
    explode_inserts: bool = False,
    max_block_nesting: int = 32,
) -> dict[str, Any]:
    return readfile(path).report(
        explode_inserts=explode_inserts,
        max_block_nesting=max_block_nesting,
    )


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="ezjww")
    subparsers = parser.add_subparsers(dest="command", required=True)

    audit_cmd = subparsers.add_parser("audit", help="run conversion-oriented health checks")
    audit_cmd.add_argument("path", help="input .jww file")
    audit_cmd.add_argument("--json", action="store_true", help="print audit result as JSON")
    audit_cmd.add_argument(
        "--fail-on-issues",
        action="store_true",
        help="exit with non-zero status when issues are detected",
    )
    audit_cmd.add_argument(
        "--explode-inserts",
        action="store_true",
        help="expand INSERT references before DXF-side checks",
    )
    audit_cmd.add_argument(
        "--max-block-nesting",
        type=int,
        default=32,
        help="maximum block nesting depth for INSERT expansion",
    )

    bbox_cmd = subparsers.add_parser("bbox", help="calculate drawing extents")
    bbox_cmd.add_argument("path", help="input .jww file")
    bbox_cmd.add_argument("--json", action="store_true", help="print extents as JSON")
    bbox_cmd.add_argument(
        "--explode-inserts",
        action="store_true",
        help="expand INSERT references before extents calculation",
    )
    bbox_cmd.add_argument(
        "--max-block-nesting",
        type=int,
        default=32,
        help="maximum block nesting depth for INSERT expansion",
    )

    stats_cmd = subparsers.add_parser("stats", help="show entity distribution statistics")
    stats_cmd.add_argument("path", help="input .jww file")
    stats_cmd.add_argument("--json", action="store_true", help="print statistics as JSON")
    stats_cmd.add_argument(
        "--explode-inserts",
        action="store_true",
        help="expand INSERT references before statistics calculation",
    )
    stats_cmd.add_argument(
        "--max-block-nesting",
        type=int,
        default=32,
        help="maximum block nesting depth for INSERT expansion",
    )

    report_cmd = subparsers.add_parser("report", help="emit combined audit+bbox+stats report")
    report_cmd.add_argument("path", help="input .jww file")
    report_cmd.add_argument("--json", action="store_true", help="print report as JSON")
    report_cmd.add_argument(
        "--fail-on-issues",
        action="store_true",
        help="exit with non-zero status when issues are detected",
    )
    report_cmd.add_argument(
        "--explode-inserts",
        action="store_true",
        help="expand INSERT references before report generation",
    )
    report_cmd.add_argument(
        "--max-block-nesting",
        type=int,
        default=32,
        help="maximum block nesting depth for INSERT expansion",
    )

    info = subparsers.add_parser("info", help="show JWW summary")
    info.add_argument("path", help="input .jww file")
    info.add_argument("--json", action="store_true", help="print as JSON")

    to_dxf = subparsers.add_parser("to-dxf", help="convert single JWW to DXF")
    to_dxf.add_argument("path", help="input .jww file")
    to_dxf.add_argument("-o", "--output", help="output .dxf path (default: input stem + .dxf)")
    to_dxf.add_argument(
        "--report",
        choices=["json"],
        help="emit conversion report in the selected format",
    )
    to_dxf.add_argument(
        "--report-path",
        help="path to write conversion report (default: stdout)",
    )
    to_dxf.add_argument(
        "--explode-inserts",
        action="store_true",
        help="expand INSERT references into transformed primitive entities",
    )
    to_dxf.add_argument(
        "--max-block-nesting",
        type=int,
        default=32,
        help="maximum block nesting depth for INSERT expansion",
    )

    to_dxf_dir = subparsers.add_parser(
        "to-dxf-dir", help="convert all .jww files in a directory"
    )
    to_dxf_dir.add_argument("input_dir", help="directory containing .jww files")
    to_dxf_dir.add_argument("-o", "--output-dir", help="output directory")
    to_dxf_dir.add_argument(
        "-r", "--recursive", action="store_true", help="scan subdirectories recursively"
    )
    to_dxf_dir.add_argument(
        "--fail-fast", action="store_true", help="stop at first conversion error"
    )
    to_dxf_dir.add_argument(
        "--report",
        choices=["json"],
        help="emit conversion report in the selected format",
    )
    to_dxf_dir.add_argument(
        "--report-path",
        help="path to write conversion report (default: stdout)",
    )
    to_dxf_dir.add_argument(
        "--explode-inserts",
        action="store_true",
        help="expand INSERT references into transformed primitive entities",
    )
    to_dxf_dir.add_argument(
        "--max-block-nesting",
        type=int,
        default=32,
        help="maximum block nesting depth for INSERT expansion",
    )

    plot = subparsers.add_parser("plot", help="render JWW to image with matplotlib")
    plot.add_argument("path", help="input .jww file")
    plot.add_argument(
        "-o",
        "--output",
        help="output image path (default: input stem + .png, unless --show only)",
    )
    plot.add_argument("--show", action="store_true", help="show interactive plot window")
    plot.add_argument(
        "--layers",
        help="comma-separated layer names to draw (e.g. '0,1,2' or '#lv4,#lv5')",
    )
    plot.add_argument("--dpi", type=int, default=150, help="output image DPI")
    plot.add_argument("--invert-y", action="store_true", help="invert Y axis")
    plot.add_argument("--no-text", action="store_true", help="hide TEXT entities")
    plot.add_argument("--no-points", action="store_true", help="hide POINT entities")
    plot.add_argument("--no-inserts", action="store_true", help="hide INSERT markers")
    plot.add_argument(
        "--explode-inserts",
        action="store_true",
        help="expand INSERT references into transformed primitive entities before plotting",
    )
    plot.add_argument(
        "--max-block-nesting",
        type=int,
        default=32,
        help="maximum block nesting depth for INSERT expansion",
    )

    return parser


def _collect_jww_files(input_dir: Path, recursive: bool) -> list[Path]:
    if recursive:
        iterator = input_dir.rglob("*")
    else:
        iterator = input_dir.iterdir()
    return sorted(
        path
        for path in iterator
        if path.is_file() and path.suffix.lower() == ".jww"
    )


def _default_output_path(input_path: Path) -> Path:
    return input_path.with_suffix(".dxf")


_SELECTOR_TOKEN_RE = re.compile(r"^[A-Za-z_][A-Za-z0-9_]*$")
_LAYER_FILTER_RE = re.compile(r"""^layer\s*==\s*(?P<quote>['"])(?P<value>.*?)(?P=quote)$""", re.IGNORECASE)
_COLOR_FILTER_RE = re.compile(r"^color\s*==\s*(?P<value>-?\d+)$", re.IGNORECASE)
_FILTER_SPLIT_RE = re.compile(r"\s*(?:,|&&|\band\b)\s*", re.IGNORECASE)


def _parse_query_selector(selector: str | None) -> tuple[set[str] | None, str | None, int | None]:
    if selector is None:
        return None, None, None
    if not isinstance(selector, str):
        raise TypeError("query selector must be a string or None")

    text = selector.strip()
    if not text:
        return None, None, None

    if "[" in text:
        if text.count("[") != 1 or not text.endswith("]"):
            raise ValueError(f"invalid query selector: {selector!r}")
        entity_part, filter_part = text.split("[", 1)
        filter_expr = filter_part[:-1].strip()
    else:
        entity_part = text
        filter_expr = ""

    query_types = _parse_selector_types(entity_part.strip(), selector)
    query_layer, query_color = _parse_selector_filters(filter_expr, selector)
    return query_types, query_layer, query_color


def _parse_selector_types(entity_part: str, original_selector: str) -> set[str] | None:
    if not entity_part or entity_part == "*":
        return None

    tokens = entity_part.replace(",", " ").split()
    if not tokens:
        return None

    types: set[str] = set()
    for token in tokens:
        upper = token.upper()
        if upper == "*":
            if len(tokens) > 1:
                raise ValueError(f"invalid query selector: {original_selector!r}")
            return None
        if not _SELECTOR_TOKEN_RE.fullmatch(token):
            raise ValueError(f"invalid query selector: {original_selector!r}")
        types.add(upper)
    return types


def _parse_selector_filters(filter_expr: str, original_selector: str) -> tuple[str | None, int | None]:
    if not filter_expr:
        return None, None

    layer: str | None = None
    color: int | None = None
    parts = [part.strip() for part in _FILTER_SPLIT_RE.split(filter_expr) if part.strip()]
    if not parts:
        raise ValueError(f"invalid query selector: {original_selector!r}")

    for part in parts:
        layer_match = _LAYER_FILTER_RE.fullmatch(part)
        if layer_match is not None:
            if layer is not None:
                raise ValueError(f"duplicate layer filter in selector: {original_selector!r}")
            layer = layer_match.group("value")
            continue

        color_match = _COLOR_FILTER_RE.fullmatch(part)
        if color_match is not None:
            if color is not None:
                raise ValueError(f"duplicate color filter in selector: {original_selector!r}")
            color = int(color_match.group("value"))
            continue

        raise ValueError(f"unsupported query filter in selector: {original_selector!r}")

    return layer, color


def _dxf_bbox(dxf_document: dict[str, Any]) -> dict[str, float | int] | None:
    points: list[tuple[float, float]] = []
    counted_entities = 0
    for entity in dxf_document.get("entities", []):
        entity_points = _dxf_entity_points(entity)
        if not entity_points:
            continue
        counted_entities += 1
        points.extend(entity_points)

    if not points:
        return None

    xs = [p[0] for p in points]
    ys = [p[1] for p in points]
    min_x = min(xs)
    min_y = min(ys)
    max_x = max(xs)
    max_y = max(ys)
    return {
        "min_x": min_x,
        "min_y": min_y,
        "max_x": max_x,
        "max_y": max_y,
        "width": max_x - min_x,
        "height": max_y - min_y,
        "entity_count": counted_entities,
    }


def _dxf_stats(dxf_document: dict[str, Any]) -> dict[str, Any]:
    entities = list(dxf_document.get("entities", []))
    by_type: Counter[str] = Counter()
    by_layer: Counter[str] = Counter()
    by_color: Counter[int] = Counter()

    for entity in entities:
        entity_type = str(entity.get("type", "")).upper() or "UNKNOWN"
        by_type[entity_type] += 1

        layer = str(entity.get("layer", "0")) or "0"
        by_layer[layer] += 1

        try:
            color = int(entity.get("color", 256))
        except (TypeError, ValueError):
            color = 256
        by_color[color] += 1

    return {
        "entity_count": len(entities),
        "type_count": len(by_type),
        "layer_count": len(by_layer),
        "color_count": len(by_color),
        "by_type": dict(sorted(by_type.items())),
        "by_layer": dict(sorted(by_layer.items())),
        "by_color": dict(sorted(by_color.items())),
    }


def _dxf_entity_points(entity: dict[str, Any]) -> list[tuple[float, float]]:
    try:
        entity_type = str(entity.get("type", "")).upper()
        if entity_type == "LINE":
            return [
                (_as_float(entity["x1"]), _as_float(entity["y1"])),
                (_as_float(entity["x2"]), _as_float(entity["y2"])),
            ]
        if entity_type == "POINT":
            return [(_as_float(entity["x"]), _as_float(entity["y"]))]
        if entity_type == "TEXT":
            return [(_as_float(entity["x"]), _as_float(entity["y"]))]
        if entity_type == "INSERT":
            return [(_as_float(entity["x"]), _as_float(entity["y"]))]
        if entity_type == "SOLID":
            return [
                (_as_float(entity["x1"]), _as_float(entity["y1"])),
                (_as_float(entity["x2"]), _as_float(entity["y2"])),
                (_as_float(entity["x3"]), _as_float(entity["y3"])),
                (_as_float(entity["x4"]), _as_float(entity["y4"])),
            ]
        if entity_type == "CIRCLE":
            cx = _as_float(entity["center_x"])
            cy = _as_float(entity["center_y"])
            radius = abs(_as_float(entity["radius"]))
            return [
                (cx - radius, cy),
                (cx + radius, cy),
                (cx, cy - radius),
                (cx, cy + radius),
            ]
        if entity_type == "ARC":
            return _arc_bbox_points(
                _as_float(entity["center_x"]),
                _as_float(entity["center_y"]),
                abs(_as_float(entity["radius"])),
                _as_float(entity["start_angle"]),
                _as_float(entity["end_angle"]),
            )
        if entity_type == "ELLIPSE":
            return _ellipse_bbox_points(
                _as_float(entity["center_x"]),
                _as_float(entity["center_y"]),
                _as_float(entity["major_axis_x"]),
                _as_float(entity["major_axis_y"]),
                _as_float(entity["minor_ratio"]),
                _as_float(entity["start_param"]),
                _as_float(entity["end_param"]),
            )
    except (KeyError, TypeError, ValueError):
        return []
    return []


def _as_float(value: Any) -> float:
    return float(value)


def _arc_bbox_points(
    center_x: float,
    center_y: float,
    radius: float,
    start_angle_deg: float,
    end_angle_deg: float,
) -> list[tuple[float, float]]:
    if radius <= 0.0:
        return [(center_x, center_y)]

    start = start_angle_deg % 360.0
    end = end_angle_deg % 360.0
    if end < start:
        end += 360.0

    points = [_arc_point(center_x, center_y, radius, start), _arc_point(center_x, center_y, radius, end)]
    for angle in (0.0, 90.0, 180.0, 270.0):
        candidate = angle
        while candidate < start:
            candidate += 360.0
        if start <= candidate <= end:
            points.append(_arc_point(center_x, center_y, radius, candidate))
    return points


def _arc_point(center_x: float, center_y: float, radius: float, angle_deg: float) -> tuple[float, float]:
    rad = math.radians(angle_deg)
    return (center_x + radius * math.cos(rad), center_y + radius * math.sin(rad))


def _ellipse_bbox_points(
    center_x: float,
    center_y: float,
    major_axis_x: float,
    major_axis_y: float,
    minor_ratio: float,
    start_param: float,
    end_param: float,
) -> list[tuple[float, float]]:
    start = start_param
    end = end_param
    if end <= start:
        end += 2.0 * math.pi
    span = max(0.0, end - start)

    sample_count = max(48, int(math.ceil(128.0 * (span / (2.0 * math.pi)))))
    minor_x = -major_axis_y * minor_ratio
    minor_y = major_axis_x * minor_ratio

    points = []
    for i in range(sample_count + 1):
        t = start + span * (i / sample_count)
        cos_t = math.cos(t)
        sin_t = math.sin(t)
        x = center_x + major_axis_x * cos_t + minor_x * sin_t
        y = center_y + major_axis_y * cos_t + minor_y * sin_t
        points.append((x, y))
    return points


def _normalize_max_block_nesting(max_block_nesting: int) -> int:
    value = int(max_block_nesting)
    if value < 1:
        raise ValueError("max_block_nesting must be >= 1")
    return value


def _emit_report(report: dict[str, Any], report_format: str | None, report_path: str | None) -> None:
    if report_format != "json":
        return
    if report_path:
        body = json.dumps(report, ensure_ascii=False, indent=2)
        out = Path(report_path)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(body + "\n", encoding="utf-8")
    else:
        _print_json(report)


def _print_json(value: Any) -> None:
    body = json.dumps(value, ensure_ascii=False, indent=2)
    try:
        sys.stdout.write(body + "\n")
    except UnicodeEncodeError:
        fallback = json.dumps(value, ensure_ascii=True, indent=2)
        sys.stdout.write(fallback + "\n")


def _run(argv: list[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)

    if args.command == "audit":
        try:
            max_block_nesting = _normalize_max_block_nesting(args.max_block_nesting)
        except ValueError as exc:
            print(str(exc), file=sys.stderr)
            return 2
        try:
            result = audit(
                args.path,
                explode_inserts=args.explode_inserts,
                max_block_nesting=max_block_nesting,
            )
        except Exception as exc:
            print(f"failed audit: {args.path}: {exc}", file=sys.stderr)
            return 2

        if args.json:
            _print_json(result)
        else:
            print(f"file: {args.path}")
            print(f"has_issues: {result['has_issues']}")
            print(f"issue_codes: {result['issue_codes']}")
            print(f"unresolved_count: {result['unresolved_count']}")
            print(f"unsupported_count: {result['unsupported_count']}")

        if args.fail_on_issues and result["has_issues"]:
            return 3
        return 0

    if args.command == "bbox":
        try:
            max_block_nesting = _normalize_max_block_nesting(args.max_block_nesting)
        except ValueError as exc:
            print(str(exc), file=sys.stderr)
            return 2
        try:
            result = bbox(
                args.path,
                explode_inserts=args.explode_inserts,
                max_block_nesting=max_block_nesting,
            )
        except Exception as exc:
            print(f"failed bbox: {args.path}: {exc}", file=sys.stderr)
            return 2

        if args.json:
            _print_json(result)
        else:
            print(f"file: {args.path}")
            if result is None:
                print("bbox: none")
            else:
                print(
                    "bbox: "
                    f"min=({result['min_x']:.6f}, {result['min_y']:.6f}) "
                    f"max=({result['max_x']:.6f}, {result['max_y']:.6f}) "
                    f"size=({result['width']:.6f}, {result['height']:.6f}) "
                    f"entities={result['entity_count']}"
                )
        return 0

    if args.command == "stats":
        try:
            max_block_nesting = _normalize_max_block_nesting(args.max_block_nesting)
        except ValueError as exc:
            print(str(exc), file=sys.stderr)
            return 2
        try:
            result = stats(
                args.path,
                explode_inserts=args.explode_inserts,
                max_block_nesting=max_block_nesting,
            )
        except Exception as exc:
            print(f"failed stats: {args.path}: {exc}", file=sys.stderr)
            return 2

        if args.json:
            _print_json(result)
        else:
            print(f"file: {args.path}")
            print(f"entity_count: {result['entity_count']}")
            print(f"type_count: {result['type_count']}")
            print(f"layer_count: {result['layer_count']}")
            print(f"color_count: {result['color_count']}")
            print(f"by_type: {result['by_type']}")
        return 0

    if args.command == "report":
        try:
            max_block_nesting = _normalize_max_block_nesting(args.max_block_nesting)
        except ValueError as exc:
            print(str(exc), file=sys.stderr)
            return 2
        try:
            result = report(
                args.path,
                explode_inserts=args.explode_inserts,
                max_block_nesting=max_block_nesting,
            )
        except Exception as exc:
            print(f"failed report: {args.path}: {exc}", file=sys.stderr)
            return 2

        if args.json:
            _print_json(result)
        else:
            audit_result = result["audit"]
            stats_result = result["stats"]
            bbox_result = result["bbox"]
            print(f"file: {args.path}")
            print(f"has_issues: {audit_result['has_issues']}")
            print(f"entity_count: {stats_result['entity_count']}")
            if bbox_result is None:
                print("bbox: none")
            else:
                print(
                    "bbox: "
                    f"min=({bbox_result['min_x']:.6f}, {bbox_result['min_y']:.6f}) "
                    f"max=({bbox_result['max_x']:.6f}, {bbox_result['max_y']:.6f})"
                )

        if args.fail_on_issues and result["audit"]["has_issues"]:
            return 3
        return 0

    if args.command == "info":
        doc = read_document(args.path)
        if args.json:
            _print_json(doc)
            return 0

        entity_total = sum(doc["entity_counts"].values())
        print(f"file: {args.path}")
        print(f"version: {doc['header']['version']}")
        print(f"entities: {entity_total}")
        print(f"block_defs: {len(doc['block_defs'])}")
        print(f"unresolved_block_refs: {doc['validation']['unresolved_def_numbers']}")
        print(f"unsupported_for_dxf: {len(read_dxf_document(args.path)['unsupported_entities'])}")
        return 0

    if args.command == "to-dxf":
        input_path = Path(args.path)
        try:
            max_block_nesting = _normalize_max_block_nesting(args.max_block_nesting)
        except ValueError as exc:
            print(str(exc), file=sys.stderr)
            return 2
        output = Path(args.output) if args.output else _default_output_path(input_path)
        output.parent.mkdir(parents=True, exist_ok=True)
        drawing: Drawing | None = None
        error: str | None = None
        try:
            drawing = readfile(str(input_path))
            drawing.saveas(
                output,
                explode_inserts=args.explode_inserts,
                max_block_nesting=max_block_nesting,
            )
            print(f"wrote: {output}")
            exit_code = 0
        except Exception as exc:
            error = str(exc)
            print(f"failed: {input_path} -> {output}: {error}", file=sys.stderr)
            exit_code = 2

        report_payload = {
            "source": str(input_path),
            "output": str(output),
            "ok": error is None,
            "error": error,
            "explode_inserts": bool(args.explode_inserts),
            "max_block_nesting": int(max_block_nesting),
            "audit": (
                drawing.audit(
                    explode_inserts=args.explode_inserts,
                    max_block_nesting=max_block_nesting,
                )
                if drawing is not None
                else None
            ),
        }
        _emit_report(report_payload, args.report, args.report_path)
        return exit_code

    if args.command == "plot":
        input_path = Path(args.path)
        try:
            max_block_nesting = _normalize_max_block_nesting(args.max_block_nesting)
        except ValueError as exc:
            print(str(exc), file=sys.stderr)
            return 2
        save_path: Path | None = None
        if args.output:
            save_path = Path(args.output)
        elif not args.show:
            save_path = input_path.with_suffix(".png")

        if save_path is not None:
            save_path.parent.mkdir(parents=True, exist_ok=True)

        layers = None
        if args.layers:
            layers = [layer.strip() for layer in args.layers.split(",") if layer.strip()]

        try:
            plot_jww(
                str(input_path),
                layers=layers,
                show=args.show,
                save_path=str(save_path) if save_path is not None else None,
                dpi=args.dpi,
                draw_text=not args.no_text,
                draw_points=not args.no_points,
                draw_inserts=not args.no_inserts,
                invert_y=args.invert_y,
                explode_inserts=args.explode_inserts,
                max_block_nesting=max_block_nesting,
            )
        except ImportError as exc:
            print(str(exc), file=sys.stderr)
            return 2

        if save_path is not None:
            print(f"wrote: {save_path}")
        elif args.show:
            print("plotted")
        return 0

    input_dir = Path(args.input_dir)
    if not input_dir.exists() or not input_dir.is_dir():
        print(f"input_dir is not a directory: {input_dir}", file=sys.stderr)
        return 2

    files = _collect_jww_files(input_dir, args.recursive)
    if not files:
        print(f"no .jww files found in {input_dir}", file=sys.stderr)
        return 1

    output_dir = Path(args.output_dir) if args.output_dir else input_dir
    try:
        max_block_nesting = _normalize_max_block_nesting(args.max_block_nesting)
    except ValueError as exc:
        print(str(exc), file=sys.stderr)
        return 2
    success = 0
    failed = 0
    report_items: list[dict[str, Any]] = []

    for src in files:
        if args.output_dir:
            rel = src.relative_to(input_dir)
            dst = (output_dir / rel).with_suffix(".dxf")
        else:
            dst = _default_output_path(src)
        dst.parent.mkdir(parents=True, exist_ok=True)

        try:
            drawing = readfile(str(src))
            drawing.saveas(
                dst,
                explode_inserts=args.explode_inserts,
                max_block_nesting=max_block_nesting,
            )
            success += 1
            report_items.append(
                {
                    "source": str(src),
                    "output": str(dst),
                    "ok": True,
                    "error": None,
                    "explode_inserts": bool(args.explode_inserts),
                    "max_block_nesting": int(max_block_nesting),
                    "audit": drawing.audit(
                        explode_inserts=args.explode_inserts,
                        max_block_nesting=max_block_nesting,
                    ),
                }
            )
        except Exception as exc:
            failed += 1
            err_text = str(exc)
            print(f"failed: {src} -> {dst}: {err_text}", file=sys.stderr)
            report_items.append(
                {
                    "source": str(src),
                    "output": str(dst),
                    "ok": False,
                    "error": err_text,
                    "explode_inserts": bool(args.explode_inserts),
                    "max_block_nesting": int(max_block_nesting),
                    "audit": None,
                }
            )
            if args.fail_fast:
                report_payload = {
                    "input_dir": str(input_dir),
                    "output_dir": str(output_dir),
                    "recursive": bool(args.recursive),
                    "explode_inserts": bool(args.explode_inserts),
                    "max_block_nesting": int(max_block_nesting),
                    "converted": success,
                    "failed": failed,
                    "items": report_items,
                }
                _emit_report(report_payload, args.report, args.report_path)
                return 2

    print(f"converted={success} failed={failed} output_dir={output_dir}")
    report_payload = {
        "input_dir": str(input_dir),
        "output_dir": str(output_dir),
        "recursive": bool(args.recursive),
        "explode_inserts": bool(args.explode_inserts),
        "max_block_nesting": int(max_block_nesting),
        "converted": success,
        "failed": failed,
        "items": report_items,
    }
    _emit_report(report_payload, args.report, args.report_path)
    return 0 if failed == 0 else 2


def main() -> None:
    raise SystemExit(_run())
