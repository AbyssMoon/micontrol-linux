#!/usr/bin/env python3
"""Рисует SVG-иконки режимов: шкала-«спидометр» с цветом режима и символом.

    python3 scripts/gen-mode-icons.py   → crates/micontrol-gui/qml/img/mode_*.svg
                                          и tray_*.svg (то же на тёмном круге)
"""
import math
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "crates/micontrol-gui/qml/img"

# режим: (доля шкалы, цвет, символ)
MODES = {
    "silent": (0.25, "#38bdf8", "moon"),
    "balance": (0.5, "#34d399", "equal"),
    "auto": (0.5, "#a78bfa", "spark"),
    "turbo": (0.75, "#fbbf24", "bolt"),
    "full_speed": (1.0, "#f43f5e", "flame"),
}

GLYPHS = {
    # полумесяц: дуга внешней окружности (50,52 r15) и внутренней (57,46 r12)
    "moon": '<path d="M64.64 55.25 A15 15 0 1 1 49.02 37.03 A12 12 0 0 0 64.64 55.25 Z" fill="{c}"/>',
    "equal": '<g stroke="{c}" stroke-width="5" stroke-linecap="round">'
             '<line x1="39" y1="46" x2="61" y2="46"/><line x1="39" y1="58" x2="61" y2="58"/></g>',
    "spark": '<path d="M50 34 Q52 50 68 52 Q52 54 50 70 Q48 54 32 52 Q48 50 50 34 Z" fill="{c}"/>',
    "bolt": '<path d="M54 34 L39 55 L49 55 L45 71 L61 48 L51 48 Z" fill="{c}" '
            'stroke="{c}" stroke-width="2" stroke-linejoin="round"/>',
    "flame": '<path d="M50 34 C56 42 64 48 62 58 C61 66 56 70 50 70 C44 70 39 66 38 58 '
             'C37 52 41 47 44 44 C45 49 47 51 49 52 C48 45 49 39 50 34 Z" fill="{c}"/>',
}

CX, CY, R = 50, 52, 36
START, SWEEP = 135, 270


def point(deg):
    a = math.radians(deg)
    return CX + R * math.cos(a), CY + R * math.sin(a)


def arc(frac):
    x0, y0 = point(START)
    x1, y1 = point(START + SWEEP * frac)
    large = 1 if SWEEP * frac > 180 else 0
    return f"M{x0:.2f} {y0:.2f} A{R} {R} 0 {large} 1 {x1:.2f} {y1:.2f}"


def svg(frac, color, glyph, background=False):
    ticks = []
    for i in range(7):
        a = math.radians(START + SWEEP * i / 6)
        r1, r2 = R - 9, R - 5
        ticks.append(
            f'<line x1="{CX + r1 * math.cos(a):.2f}" y1="{CY + r1 * math.sin(a):.2f}" '
            f'x2="{CX + r2 * math.cos(a):.2f}" y2="{CY + r2 * math.sin(a):.2f}"/>'
        )
    bg = '<circle cx="50" cy="50" r="49" fill="#1f1f1f"/>' if background else ""
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">'
        f"{bg}"
        f'<path d="{arc(1)}" fill="none" stroke="#3a3a3a" stroke-width="7" stroke-linecap="round"/>'
        f'<path d="{arc(frac)}" fill="none" stroke="{color}" stroke-width="7" stroke-linecap="round"/>'
        f'<g stroke="#5a5a5a" stroke-width="2" stroke-linecap="round">{"".join(ticks)}</g>'
        f"{GLYPHS[glyph].format(c=color)}"
        "</svg>\n"
    )


for name, (frac, color, glyph) in MODES.items():
    (OUT / f"mode_{name}.svg").write_text(svg(frac, color, glyph))
    (OUT / f"tray_{name}.svg").write_text(svg(frac, color, glyph, background=True))
# Микрофон: корпус, дужка, ножка; у выключенного — перечёркнут.
MIC = (
    '<rect x="38" y="16" width="24" height="42" rx="12" fill="{c}"/>'
    '<path d="M28 46 A22 22 0 0 0 72 46" fill="none" stroke="{c}" stroke-width="6" stroke-linecap="round"/>'
    '<line x1="50" y1="68" x2="50" y2="80" stroke="{c}" stroke-width="6" stroke-linecap="round"/>'
    '<line x1="38" y1="82" x2="62" y2="82" stroke="{c}" stroke-width="6" stroke-linecap="round"/>'
)
SLASH = '<line x1="22" y1="16" x2="78" y2="84" stroke="#1f1f1f" stroke-width="12" stroke-linecap="round"/>' \
        '<line x1="22" y1="16" x2="78" y2="84" stroke="{c}" stroke-width="6" stroke-linecap="round"/>'


def mic(color, off):
    body = MIC.format(c=color) + (SLASH.format(c=color) if off else "")
    return f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">{body}</svg>\n'


(OUT / "mic_on.svg").write_text(mic("#34d399", False))
(OUT / "mic_off.svg").write_text(mic("#f43f5e", True))

# Режим неизвестен / нет связи с демоном.
(OUT / "tray_unknown.svg").write_text(svg(0, "#9ca3af", "equal", background=True))
print("готово:", OUT)
