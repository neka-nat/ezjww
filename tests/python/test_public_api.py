from __future__ import annotations

import json
import sys
import tempfile
import unittest
from io import StringIO
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))

import ezjww


def sample_path() -> Path:
    return ROOT / "jww_samples" / "Test1.jww"


class PublicApiTests(unittest.TestCase):
    def test_readfile_modelspace_query(self):
        drawing = ezjww.readfile(sample_path())
        msp = drawing.modelspace()
        self.assertGreater(len(msp), 0)
        lines = msp.query("LINE")
        self.assertGreater(len(lines), 0)
        self.assertTrue(all(e.get("type") == "LINE" for e in lines))

    def test_modelspace_query_selector_multi_type(self):
        drawing = ezjww.readfile(sample_path())
        msp = drawing.modelspace()
        selected = msp.query("LINE POINT")
        self.assertGreater(len(selected), 0)
        self.assertTrue(all(e.get("type") in {"LINE", "POINT"} for e in selected))
        self.assertEqual(len(selected), len(msp.query("LINE")) + len(msp.query("POINT")))

    def test_modelspace_query_selector_filters(self):
        drawing = ezjww.readfile(sample_path())
        msp = drawing.modelspace()
        with_filters = msp.query('LINE[layer=="#lv4", color==5]')
        self.assertGreater(len(with_filters), 0)
        self.assertTrue(all(e.get("type") == "LINE" for e in with_filters))
        self.assertTrue(all(e.get("layer") == "#lv4" for e in with_filters))
        self.assertTrue(all(e.get("color") == 5 for e in with_filters))

        all_on_layer = msp.query('[layer=="#lv4"]')
        self.assertGreaterEqual(len(all_on_layer), len(with_filters))
        self.assertTrue(all(e.get("layer") == "#lv4" for e in all_on_layer))

    def test_modelspace_query_rejects_invalid_selector(self):
        drawing = ezjww.readfile(sample_path())
        msp = drawing.modelspace()
        with self.assertRaises(ValueError):
            msp.query("LINE[layer~=5]")

    def test_new_drawing_defaults(self):
        drawing = ezjww.new()
        self.assertEqual(len(drawing.modelspace()), 0)
        audit = drawing.audit()
        self.assertFalse(audit["has_issues"])
        self.assertEqual(audit["unsupported_entities"], [])
        self.assertEqual(audit["unresolved_def_numbers"], [])

    def test_audit_from_path(self):
        result = ezjww.audit(sample_path())
        self.assertIn("has_issues", result)
        self.assertIn("issue_codes", result)
        self.assertIn("unresolved_count", result)
        self.assertIn("unsupported_count", result)
        self.assertIn("unsupported_entities", result)
        self.assertIn("unresolved_def_numbers", result)
        self.assertEqual(result["unresolved_def_numbers"], [])
        self.assertEqual(result["issue_codes"], [])
        self.assertEqual(result["unresolved_count"], 0)
        self.assertEqual(result["unsupported_count"], 0)

    def test_bbox_from_path(self):
        result = ezjww.bbox(sample_path())
        self.assertIsNotNone(result)
        assert result is not None
        self.assertIn("min_x", result)
        self.assertIn("min_y", result)
        self.assertIn("max_x", result)
        self.assertIn("max_y", result)
        self.assertIn("width", result)
        self.assertIn("height", result)
        self.assertIn("entity_count", result)
        self.assertGreater(result["width"], 0.0)
        self.assertGreater(result["height"], 0.0)
        self.assertGreater(result["entity_count"], 0)

    def test_bbox_for_new_drawing_is_none(self):
        drawing = ezjww.new()
        self.assertIsNone(drawing.bbox())

    def test_stats_from_path(self):
        result = ezjww.stats(sample_path())
        self.assertIn("entity_count", result)
        self.assertIn("type_count", result)
        self.assertIn("layer_count", result)
        self.assertIn("color_count", result)
        self.assertIn("by_type", result)
        self.assertIn("by_layer", result)
        self.assertIn("by_color", result)
        self.assertGreater(result["entity_count"], 0)
        self.assertIn("LINE", result["by_type"])

    def test_modelspace_stats_for_subset(self):
        drawing = ezjww.readfile(sample_path())
        lines = drawing.modelspace().query("LINE")
        result = ezjww.Modelspace(lines).stats()
        self.assertEqual(set(result["by_type"].keys()), {"LINE"})
        self.assertEqual(result["entity_count"], len(lines))

    def test_report_from_path(self):
        result = ezjww.report(sample_path())
        self.assertIn("source_path", result)
        self.assertIn("explode_inserts", result)
        self.assertIn("max_block_nesting", result)
        self.assertIn("audit", result)
        self.assertIn("bbox", result)
        self.assertIn("stats", result)
        self.assertIn("has_issues", result["audit"])
        self.assertIn("entity_count", result["stats"])

    def test_to_dxf_string_from_path(self):
        text = ezjww.to_dxf_string(sample_path())
        self.assertIn("SECTION", text)
        self.assertTrue(text.endswith("  0\nEOF\n"))

    def test_drawing_to_dxf_string_with_options(self):
        drawing = ezjww.readfile(sample_path())
        text = drawing.to_dxf_string(explode_inserts=True, max_block_nesting=16)
        self.assertIn("ENTITIES", text)
        self.assertTrue(text.endswith("  0\nEOF\n"))

    def test_drawing_to_dxf_string_rejects_new_drawing(self):
        drawing = ezjww.new()
        with self.assertRaises(ValueError):
            drawing.to_dxf_string()

    def test_to_dxf_accepts_explode_options(self):
        drawing = ezjww.readfile(sample_path())
        regular = drawing.to_dxf()
        exploded = drawing.to_dxf(explode_inserts=True, max_block_nesting=16)
        self.assertIn("entities", regular)
        self.assertIn("entities", exploded)

    def test_to_dxf_rejects_invalid_max_block_nesting(self):
        drawing = ezjww.readfile(sample_path())
        with self.assertRaises(ValueError):
            drawing.to_dxf(explode_inserts=True, max_block_nesting=0)

    def test_cli_to_dxf_report_json(self):
        with tempfile.TemporaryDirectory(prefix="ezjww_test_") as tmp_dir:
            tmp = Path(tmp_dir)
            dxf_out = tmp / "out.dxf"
            report_out = tmp / "report.json"
            code = ezjww._run(
                [
                    "to-dxf",
                    str(sample_path()),
                    "-o",
                    str(dxf_out),
                    "--report",
                    "json",
                    "--report-path",
                    str(report_out),
                ]
            )
            self.assertEqual(code, 0)
            self.assertTrue(dxf_out.exists())
            self.assertTrue(report_out.exists())
            report = json.loads(report_out.read_text(encoding="utf-8"))
            self.assertTrue(report["ok"])
            self.assertIn("audit", report)
            self.assertIn("explode_inserts", report)
            self.assertIn("max_block_nesting", report)

    def test_print_json_fallback_for_non_utf8_stdout(self):
        class _AsciiStdout:
            def __init__(self):
                self.parts: list[str] = []

            def write(self, text: str) -> int:
                text.encode("ascii")
                self.parts.append(text)
                return len(text)

            def flush(self) -> None:
                return None

            def getvalue(self) -> str:
                return "".join(self.parts)

        fake_stdout = _AsciiStdout()
        with patch("sys.stdout", new=fake_stdout):
            ezjww._print_json({"text": "日本語"})

        parsed = json.loads(fake_stdout.getvalue())
        self.assertEqual(parsed["text"], "日本語")

    def test_cli_audit_json(self):
        buf = StringIO()
        with patch("sys.stdout", new=buf):
            code = ezjww._run(["audit", str(sample_path()), "--json"])
        self.assertEqual(code, 0)
        out = json.loads(buf.getvalue())
        self.assertIn("has_issues", out)
        self.assertIn("issue_codes", out)
        self.assertIn("unsupported_count", out)
        self.assertIn("unresolved_count", out)
        self.assertFalse(out["has_issues"])

    def test_cli_audit_fail_on_issues(self):
        fake = {
            "source_path": "dummy.jww",
            "total_references": 1,
            "resolved_references": 0,
            "unresolved_def_numbers": [10],
            "unresolved_count": 1,
            "unsupported_entities": [],
            "unsupported_count": 0,
            "issue_codes": ["UNRESOLVED_BLOCK_REFERENCES"],
            "has_issues": True,
            "warnings": ["unresolved block references detected"],
        }
        with patch.object(ezjww, "audit", return_value=fake):
            code = ezjww._run(["audit", str(sample_path()), "--fail-on-issues"])
        self.assertEqual(code, 3)

    def test_cli_audit_rejects_invalid_max_block_nesting(self):
        code = ezjww._run(["audit", str(sample_path()), "--max-block-nesting", "0"])
        self.assertEqual(code, 2)

    def test_cli_bbox_json(self):
        buf = StringIO()
        with patch("sys.stdout", new=buf):
            code = ezjww._run(["bbox", str(sample_path()), "--json", "--explode-inserts"])
        self.assertEqual(code, 0)
        out = json.loads(buf.getvalue())
        self.assertIn("min_x", out)
        self.assertIn("min_y", out)
        self.assertIn("max_x", out)
        self.assertIn("max_y", out)
        self.assertIn("width", out)
        self.assertIn("height", out)
        self.assertIn("entity_count", out)

    def test_cli_bbox_rejects_invalid_max_block_nesting(self):
        code = ezjww._run(["bbox", str(sample_path()), "--max-block-nesting", "0"])
        self.assertEqual(code, 2)

    def test_cli_stats_json(self):
        buf = StringIO()
        with patch("sys.stdout", new=buf):
            code = ezjww._run(["stats", str(sample_path()), "--json"])
        self.assertEqual(code, 0)
        out = json.loads(buf.getvalue())
        self.assertIn("entity_count", out)
        self.assertIn("by_type", out)
        self.assertIn("by_layer", out)
        self.assertIn("by_color", out)
        self.assertGreater(out["entity_count"], 0)

    def test_cli_stats_rejects_invalid_max_block_nesting(self):
        code = ezjww._run(["stats", str(sample_path()), "--max-block-nesting", "0"])
        self.assertEqual(code, 2)

    def test_cli_report_json(self):
        buf = StringIO()
        with patch("sys.stdout", new=buf):
            code = ezjww._run(["report", str(sample_path()), "--json"])
        self.assertEqual(code, 0)
        out = json.loads(buf.getvalue())
        self.assertIn("audit", out)
        self.assertIn("bbox", out)
        self.assertIn("stats", out)
        self.assertIn("has_issues", out["audit"])

    def test_cli_report_fail_on_issues(self):
        fake = {
            "source_path": "dummy.jww",
            "explode_inserts": False,
            "max_block_nesting": 32,
            "audit": {"has_issues": True},
            "bbox": None,
            "stats": {"entity_count": 0},
        }
        with patch.object(ezjww, "report", return_value=fake):
            code = ezjww._run(["report", str(sample_path()), "--fail-on-issues"])
        self.assertEqual(code, 3)

    def test_cli_report_rejects_invalid_max_block_nesting(self):
        code = ezjww._run(["report", str(sample_path()), "--max-block-nesting", "0"])
        self.assertEqual(code, 2)

    def test_cli_to_dxf_rejects_invalid_max_block_nesting(self):
        with tempfile.TemporaryDirectory(prefix="ezjww_test_") as tmp_dir:
            tmp = Path(tmp_dir)
            dxf_out = tmp / "out.dxf"
            code = ezjww._run(
                [
                    "to-dxf",
                    str(sample_path()),
                    "-o",
                    str(dxf_out),
                    "--max-block-nesting",
                    "0",
                ]
            )
            self.assertEqual(code, 2)
            self.assertFalse(dxf_out.exists())

    def test_cli_to_dxf_dir_report_json(self):
        with tempfile.TemporaryDirectory(prefix="ezjww_test_") as tmp_dir:
            tmp = Path(tmp_dir)
            out_dir = tmp / "dxf"
            report_out = tmp / "dir_report.json"
            code = ezjww._run(
                [
                    "to-dxf-dir",
                    str(ROOT / "jww_samples"),
                    "-o",
                    str(out_dir),
                    "--report",
                    "json",
                    "--report-path",
                    str(report_out),
                ]
            )
            self.assertEqual(code, 0)
            self.assertTrue(report_out.exists())
            report = json.loads(report_out.read_text(encoding="utf-8"))
            self.assertEqual(report["failed"], 0)
            self.assertGreater(report["converted"], 0)
            self.assertEqual(len(report["items"]), report["converted"])
            self.assertIn("explode_inserts", report)
            self.assertIn("max_block_nesting", report)


if __name__ == "__main__":
    unittest.main()
