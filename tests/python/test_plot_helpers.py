from __future__ import annotations

import importlib.util
import math
import unittest
from pathlib import Path


def load_plot_module():
    root = Path(__file__).resolve().parents[2]
    module_path = root / "src" / "ezjww" / "plot.py"
    spec = importlib.util.spec_from_file_location("ezjww_plot_module", module_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"failed to load module from {module_path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


PLOT = load_plot_module()


class PlotHelperTests(unittest.TestCase):
    def test_normalize_layer_filter(self):
        self.assertIsNone(PLOT._normalize_layer_filter(None))
        self.assertEqual(PLOT._normalize_layer_filter([" A ", "", "B"]), {"A", "B"})
        self.assertIsNone(PLOT._normalize_layer_filter([" ", ""]))

    def test_line_style_mapping(self):
        self.assertEqual(PLOT._line_style("CONTINUOUS"), "-")
        self.assertEqual(PLOT._line_style("dashed"), "--")
        self.assertEqual(PLOT._line_style("DASHDOT"), "-.")
        self.assertEqual(PLOT._line_style("DOT"), ":")
        self.assertEqual(PLOT._line_style("unknown"), "-")

    def test_ellipse_points_endpoints_for_full_loop(self):
        points = PLOT._ellipse_points(10.0, 5.0, 3.0, 0.0, 0.5, 0.0, 2.0 * math.pi)
        self.assertGreaterEqual(len(points), 24)
        self.assertAlmostEqual(points[0][0], points[-1][0], places=6)
        self.assertAlmostEqual(points[0][1], points[-1][1], places=6)

    def test_aci_to_color(self):
        self.assertEqual(PLOT._aci_to_color(1), "#ff0000")
        self.assertEqual(PLOT._aci_to_color(7), "#000000")
        fallback = PLOT._aci_to_color(200)
        self.assertIsInstance(fallback, tuple)
        self.assertEqual(len(fallback), 3)


if __name__ == "__main__":
    unittest.main()
