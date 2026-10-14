#!/usr/bin/env python3
"""Рисует контурные иконки разделов для меню слева (24×24, в два цвета).

    python3 scripts/gen-nav-icons.py   → crates/micontrol-gui/qml/img/nav_*.svg
                                         (nav_X.svg — обычный, nav_X_on.svg — выбранный)
"""
from pathlib import Path

OUT = Path(__file__).resolve().parent.parent / "crates/micontrol-gui/qml/img"

# Серый читается и на светлой, и на тёмной теме; выбранный — цвет выделения Fusion.
COLORS = {"": "#7b8189", "_on": "#2f7fc1"}

ICONS = {
    # шкала-спидометр со стрелкой
    "perf": '<path d="M4.6 17.5 A8.5 8.5 0 1 1 19.4 17.5"/>'
            '<path d="M12 13.5 L16 9"/><circle cx="12" cy="13.5" r="1.3" fill="{c}"/>',
    # батарея с молнией
    "battery": '<rect x="2.5" y="7" width="17" height="10" rx="2.5"/>'
               '<path d="M21.5 10.5 V13.5"/>'
               '<path d="M11.8 9 L9 12.3 H12.2 L10.2 15" stroke-width="1.6"/>',
    "mic": '<rect x="9" y="3" width="6" height="11" rx="3"/>'
           '<path d="M5.5 11 A6.5 6.5 0 0 0 18.5 11"/><path d="M12 17.5 V21 M8.5 21 H15.5"/>',
    # тачпад с двумя кнопками
    "touchpad": '<rect x="3" y="4" width="18" height="16" rx="3"/>'
                '<path d="M3 15 H21 M12 15 V20"/>',
    # клавиша с «⌘»
    "keys": '<rect x="3" y="3" width="18" height="18" rx="4"/>'
            '<path d="M10 10 V14 M14 10 V14 M10 10 H14 M10 14 H14'
            ' M10 10 A1.6 1.6 0 1 0 8.4 11.6 H10 M14 10 A1.6 1.6 0 1 1 15.6 11.6 H14'
            ' M10 14 A1.6 1.6 0 1 1 8.4 12.4 H10 M14 14 A1.6 1.6 0 1 0 15.6 12.4 H14"'
            ' stroke-width="1.4"/>',
    # микросхема процессора
    "system": '<rect x="6" y="6" width="12" height="12" rx="2"/><rect x="9.5" y="9.5" width="5" height="5" rx="0.8"/>'
              '<path d="M9.5 3 V6 M14.5 3 V6 M9.5 18 V21 M14.5 18 V21'
              ' M3 9.5 H6 M3 14.5 H6 M18 9.5 H21 M18 14.5 H21"/>',
    "about": '<circle cx="12" cy="12" r="9"/><path d="M12 11 V16.5"/>'
             '<circle cx="12" cy="7.8" r="1.1" fill="{c}" stroke="none"/>',
}


def svg(body, color):
    return ('<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" '
            f'fill="none" stroke="{color}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">'
            f'{body.format(c=color)}</svg>\n')


for name, body in ICONS.items():
    for suffix, color in COLORS.items():
        (OUT / f"nav_{name}{suffix}.svg").write_text(svg(body, color))
print("готово:", OUT)
