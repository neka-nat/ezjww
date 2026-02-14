from typing import TypedDict


class LayerHeader(TypedDict):
    state: int
    protect: int
    name: str


class LayerGroupHeader(TypedDict):
    state: int
    write_layer: int
    scale: float
    protect: int
    name: str
    layers: list[LayerHeader]


class JwwHeader(TypedDict):
    version: int
    memo: str
    paper_size: int
    write_layer_group: int
    layer_groups: list[LayerGroupHeader]


class EntityBase(TypedDict):
    group: int
    pen_style: int
    pen_color: int
    pen_width: int
    layer: int
    layer_group: int
    flag: int


class LinePayload(TypedDict):
    start_x: float
    start_y: float
    end_x: float
    end_y: float


class PointPayload(TypedDict):
    x: float
    y: float
    is_temporary: bool
    code: int
    angle: float
    scale: float


class TextPayload(TypedDict):
    start_x: float
    start_y: float
    end_x: float
    end_y: float
    text_type: int
    size_x: float
    size_y: float
    spacing: float
    angle: float
    font_name: str
    content: str


class JwwEntity(TypedDict, total=False):
    type: str
    base: EntityBase
    start_x: float
    start_y: float
    end_x: float
    end_y: float
    center_x: float
    center_y: float
    radius: float
    start_angle: float
    arc_angle: float
    tilt_angle: float
    flatness: float
    is_full_circle: bool
    x: float
    y: float
    is_temporary: bool
    code: int
    angle: float
    scale: float
    text_type: int
    size_x: float
    size_y: float
    spacing: float
    font_name: str
    content: str
    point1_x: float
    point1_y: float
    point2_x: float
    point2_y: float
    point3_x: float
    point3_y: float
    point4_x: float
    point4_y: float
    color: int | None
    ref_x: float
    ref_y: float
    scale_x: float
    scale_y: float
    rotation: float
    def_number: int
    block_name: str | None
    line: LinePayload
    text: TextPayload
    sxf_mode: int | None
    aux_lines: list[LinePayload]
    aux_points: list[PointPayload]


class BlockDef(TypedDict):
    number: int
    is_referenced: bool
    name: str
    base: EntityBase
    entities: list[JwwEntity]


class BlockReferenceValidation(TypedDict):
    total_references: int
    resolved_references: int
    unresolved_def_numbers: list[int]
    has_unresolved: bool


class JwwDocument(TypedDict):
    header: JwwHeader
    entities: list[JwwEntity]
    block_defs: list[BlockDef]
    block_def_names: dict[int, str]
    entity_counts: dict[str, int]
    validation: BlockReferenceValidation


class DxfLayer(TypedDict):
    name: str
    color: int
    line_type: str
    frozen: bool
    locked: bool


class DxfEntity(TypedDict, total=False):
    type: str
    layer: str
    color: int
    line_type: str
    x1: float
    y1: float
    x2: float
    y2: float
    center_x: float
    center_y: float
    radius: float
    start_angle: float
    end_angle: float
    major_axis_x: float
    major_axis_y: float
    minor_ratio: float
    start_param: float
    end_param: float
    x: float
    y: float
    height: float
    rotation: float
    content: str
    style: str
    x3: float
    y3: float
    x4: float
    y4: float
    block_name: str
    scale_x: float
    scale_y: float


class DxfBlock(TypedDict):
    name: str
    base_x: float
    base_y: float
    entities: list[DxfEntity]


class DxfDocument(TypedDict):
    layers: list[DxfLayer]
    entities: list[DxfEntity]
    blocks: list[DxfBlock]
    unsupported_entities: list[str]


def hello_from_bin() -> str: ...
def is_jww_file(path: str) -> bool: ...
def read_header(path: str) -> JwwHeader: ...
def read_document(path: str) -> JwwDocument: ...
def read_dxf_document(
    path: str,
    explode_inserts: bool = False,
    max_block_nesting: int = 32,
) -> DxfDocument: ...
def read_dxf_string(
    path: str,
    explode_inserts: bool = False,
    max_block_nesting: int = 32,
) -> str: ...
def write_dxf(
    path: str,
    output_path: str,
    explode_inserts: bool = False,
    max_block_nesting: int = 32,
) -> None: ...
