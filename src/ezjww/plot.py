from __future__ import annotations

import math
from pathlib import Path
from typing import Any, Iterable


def _load_matplotlib() -> tuple[Any, Any]:
    try:
        import matplotlib.pyplot as plt
        from matplotlib import patches
    except Exception as exc:  # pragma: no cover - runtime dependency path
        raise ImportError(
            "matplotlib is required for plotting. Install with: pip install 'ezjww[plot]'"
        ) from exc
    return plt, patches


def _normalize_layer_filter(layers: Iterable[str] | None) -> set[str] | None:
    if layers is None:
        return None
    normalized = {layer.strip() for layer in layers if layer.strip()}
    return normalized or None


def _aci_to_color(aci: int) -> Any:
    mapping = {
        1: "#ff0000",
        2: "#ffff00",
        3: "#00ff00",
        4: "#00ffff",
        5: "#0000ff",
        6: "#ff00ff",
        7: "#000000",
        8: "#808080",
        9: "#c0c0c0",
    }
    if aci in mapping:
        return mapping[aci]
    if aci <= 0 or aci == 256:
        return "#000000"
    hue = (aci % 255) / 255.0
    # fallback color generation for extended ACI indexes
    return (hue, 0.7, 0.9)


def _line_style(line_type: str) -> str:
    name = line_type.upper()
    if name == "CONTINUOUS":
        return "-"
    if name in {"DASHED", "DASHED2"}:
        return "--"
    if name == "DASHDOT":
        return "-."
    if name == "DOT":
        return ":"
    return "-"


def _ellipse_points(
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
    span = end - start
    count = max(24, int(64 * (span / (2.0 * math.pi))))

    u_x = major_axis_x
    u_y = major_axis_y
    v_x = -major_axis_y * minor_ratio
    v_y = major_axis_x * minor_ratio

    points = []
    for i in range(count + 1):
        t = start + (span * i / count)
        cos_t = math.cos(t)
        sin_t = math.sin(t)
        x = center_x + u_x * cos_t + v_x * sin_t
        y = center_y + u_y * cos_t + v_y * sin_t
        points.append((x, y))
    return points


def plot_dxf_document(
    dxf_document: dict[str, Any],
    *,
    ax: Any | None = None,
    layers: Iterable[str] | None = None,
    linewidth: float = 0.8,
    point_size: float = 12.0,
    draw_text: bool = True,
    draw_points: bool = True,
    draw_inserts: bool = True,
    text_scale: float = 1.0,
    invert_y: bool = False,
    equal_aspect: bool = True,
    autoscale: bool = True,
    show: bool = False,
    save_path: str | Path | None = None,
    dpi: int = 150,
    figsize: tuple[float, float] = (10.0, 10.0),
) -> Any:
    plt, patches = _load_matplotlib()
    layer_filter = _normalize_layer_filter(layers)

    if ax is None:
        fig, ax = plt.subplots(figsize=figsize)
    else:
        fig = ax.figure

    for entity in dxf_document.get("entities", []):
        layer = str(entity.get("layer", "0"))
        if layer_filter is not None and layer not in layer_filter:
            continue

        entity_type = str(entity.get("type", ""))
        color = _aci_to_color(int(entity.get("color", 256)))
        line_style = _line_style(str(entity.get("line_type", "CONTINUOUS")))

        if entity_type == "LINE":
            ax.plot(
                [entity["x1"], entity["x2"]],
                [entity["y1"], entity["y2"]],
                color=color,
                linewidth=linewidth,
                linestyle=line_style,
            )
        elif entity_type == "CIRCLE":
            patch = patches.Circle(
                (entity["center_x"], entity["center_y"]),
                entity["radius"],
                fill=False,
                edgecolor=color,
                linewidth=linewidth,
                linestyle=line_style,
            )
            ax.add_patch(patch)
        elif entity_type == "ARC":
            start = float(entity["start_angle"])
            end = float(entity["end_angle"])
            if end < start:
                end += 360.0
            patch = patches.Arc(
                (entity["center_x"], entity["center_y"]),
                2.0 * entity["radius"],
                2.0 * entity["radius"],
                angle=0.0,
                theta1=start,
                theta2=end,
                edgecolor=color,
                linewidth=linewidth,
                linestyle=line_style,
            )
            ax.add_patch(patch)
        elif entity_type == "ELLIPSE":
            major_x = float(entity["major_axis_x"])
            major_y = float(entity["major_axis_y"])
            major_radius = math.hypot(major_x, major_y)
            minor_ratio = float(entity["minor_ratio"])
            start_param = float(entity["start_param"])
            end_param = float(entity["end_param"])

            if major_radius <= 0.0:
                continue

            span = end_param - start_param
            if span <= 0.0:
                span += 2.0 * math.pi

            is_full = abs(span - 2.0 * math.pi) < 1e-6
            if is_full:
                angle_deg = math.degrees(math.atan2(major_y, major_x))
                patch = patches.Ellipse(
                    (entity["center_x"], entity["center_y"]),
                    width=2.0 * major_radius,
                    height=2.0 * major_radius * minor_ratio,
                    angle=angle_deg,
                    fill=False,
                    edgecolor=color,
                    linewidth=linewidth,
                    linestyle=line_style,
                )
                ax.add_patch(patch)
            else:
                points = _ellipse_points(
                    float(entity["center_x"]),
                    float(entity["center_y"]),
                    major_x,
                    major_y,
                    minor_ratio,
                    start_param,
                    end_param,
                )
                xs, ys = zip(*points)
                ax.plot(xs, ys, color=color, linewidth=linewidth, linestyle=line_style)
        elif entity_type == "POINT":
            if draw_points:
                ax.scatter([entity["x"]], [entity["y"]], s=point_size, c=[color], marker="o")
        elif entity_type == "TEXT":
            if draw_text:
                content = str(entity.get("content", ""))
                height = max(6.0, float(entity.get("height", 2.5)) * text_scale)
                ax.text(
                    entity["x"],
                    entity["y"],
                    content,
                    color=color,
                    fontsize=height,
                    rotation=float(entity.get("rotation", 0.0)),
                    ha="left",
                    va="bottom",
                )
        elif entity_type == "SOLID":
            points = [
                (entity["x1"], entity["y1"]),
                (entity["x2"], entity["y2"]),
                (entity["x3"], entity["y3"]),
                (entity["x4"], entity["y4"]),
            ]
            patch = patches.Polygon(
                points,
                closed=True,
                facecolor=color,
                edgecolor=color,
                linewidth=max(0.3, linewidth * 0.8),
                alpha=0.3,
            )
            ax.add_patch(patch)
        elif entity_type == "INSERT":
            if draw_inserts:
                x = float(entity.get("x", 0.0))
                y = float(entity.get("y", 0.0))
                ax.scatter([x], [y], s=point_size * 1.5, c=[color], marker="x")
                name = str(entity.get("block_name", ""))
                if name:
                    ax.text(x, y, name, color=color, fontsize=7.0, ha="left", va="bottom")

    if equal_aspect:
        ax.set_aspect("equal", adjustable="datalim")
    if autoscale:
        ax.autoscale_view()
    if invert_y:
        ax.invert_yaxis()

    ax.set_xlabel("X")
    ax.set_ylabel("Y")
    ax.set_title("JWW Plot")
    ax.grid(False)

    if save_path is not None:
        output = Path(save_path)
        output.parent.mkdir(parents=True, exist_ok=True)
        fig.savefig(output, dpi=dpi, bbox_inches="tight")
    if show:
        plt.show()

    return ax


def plot_jww(
    path: str | Path,
    *,
    explode_inserts: bool = False,
    max_block_nesting: int = 32,
    **kwargs: Any,
) -> Any:
    from ezjww._core import read_dxf_document

    dxf_document = read_dxf_document(
        str(path),
        explode_inserts,
        max_block_nesting,
    )
    return plot_dxf_document(dxf_document, **kwargs)
