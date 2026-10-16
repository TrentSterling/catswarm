"""Generates the bundled kitty sprite and app icon.

Run from the kittyparty folder: python tools/make_art.py
Drawn at 4x and downsampled so the edges come out smooth.
"""
from PIL import Image, ImageDraw

S = 1024           # working canvas
OUT = 256          # sprite size baked into the exe
INK = (40, 30, 35, 255)
WHITE = (255, 255, 255, 255)
PINK = (255, 120, 170, 255)
PINK_DARK = (230, 70, 130, 255)
BLUSH = (255, 170, 190, 255)
YELLOW = (255, 205, 60, 255)
W = 22             # outline width at working scale


def ell(d, box, fill, grow=0):
    x0, y0, x1, y1 = box
    d.ellipse((x0 - grow, y0 - grow, x1 + grow, y1 + grow), fill=fill)


def poly(d, pts, fill, grow=0):
    if grow:
        cx = sum(p[0] for p in pts) / len(pts)
        cy = sum(p[1] for p in pts) / len(pts)
        out = []
        for x, y in pts:
            dx, dy = x - cx, y - cy
            n = max((dx * dx + dy * dy) ** 0.5, 1)
            out.append((x + dx / n * grow * 1.6, y + dy / n * grow * 1.6))
        pts = out
    d.polygon(pts, fill=fill)


def kitty():
    img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    tail = [(700, 760), (800, 700), (850, 600), (900, 560), (925, 600), (880, 680), (800, 790), (720, 820)]
    body = (330, 600, 700, 930)
    head = (150, 210, 870, 690)
    ear_l = [(195, 360), (230, 120), (420, 250)]
    ear_r = [(600, 250), (790, 120), (825, 360)]
    feet = [(330, 860, 490, 960), (530, 860, 690, 960)]
    arms = [(270, 680, 400, 800), (620, 680, 750, 800)]

    # Outline pass: every white shape grown by the stroke width, in ink.
    poly(d, tail, INK, W)
    for b in feet + arms:
        ell(d, b, INK, W)
    ell(d, body, INK, W)
    poly(d, ear_l, INK, W)
    poly(d, ear_r, INK, W)
    ell(d, head, INK, W)

    # Fill pass.
    poly(d, tail, WHITE)
    for b in feet + arms:
        ell(d, b, WHITE)
    ell(d, body, WHITE)
    poly(d, ear_l, WHITE)
    poly(d, ear_r, WHITE)
    ell(d, head, WHITE)
    poly(d, [(245, 330), (255, 190), (360, 265)], BLUSH)
    poly(d, [(660, 265), (765, 190), (775, 330)], BLUSH)

    # Pink overalls-style belly patch with a heart pocket.
    ell(d, (380, 700, 650, 900), PINK)
    heart = Image.new("RGBA", (S, S), (0, 0, 0, 0))
    hd = ImageDraw.Draw(heart)
    ell(hd, (470, 760, 520, 810), WHITE)
    ell(hd, (510, 760, 560, 810), WHITE)
    poly(hd, [(474, 795), (556, 795), (515, 850)], WHITE)
    img.alpha_composite(heart)

    # Face.
    ell(d, (330, 430, 385, 510), INK)          # eyes
    ell(d, (635, 430, 690, 510), INK)
    ell(d, (345, 440, 362, 462), WHITE)        # eye shine
    ell(d, (650, 440, 667, 462), WHITE)
    ell(d, (470, 505, 550, 555), YELLOW, 6)
    ell(d, (470, 505, 550, 555), YELLOW)
    d.arc((455, 540, 515, 600), 20, 160, fill=INK, width=12)   # little "w" smile
    d.arc((505, 540, 565, 600), 20, 160, fill=INK, width=12)
    ell(d, (240, 520, 330, 575), BLUSH)        # cheeks
    ell(d, (690, 520, 780, 575), BLUSH)
    for y, dy in ((470, -25), (520, 0), (570, 25)):
        d.line((120, y + dy, 290, y), fill=INK, width=14)
        d.line((730, y, 900, y + dy), fill=INK, width=14)

    # Bow on the right ear (viewer's right).
    cx, cy = 720, 250
    for flip in (-1, 1):
        wing = [(cx, cy), (cx + flip * 150, cy - 110), (cx + flip * 175, cy + 20), (cx + flip * 120, cy + 110)]
        poly(d, wing, INK, W)
    for flip in (-1, 1):
        wing = [(cx, cy), (cx + flip * 150, cy - 110), (cx + flip * 175, cy + 20), (cx + flip * 120, cy + 110)]
        poly(d, wing, PINK)
        ell(d, (cx + flip * 110 - 18, cy - 20, cx + flip * 110 + 18, cy + 16), PINK_DARK)
    ell(d, (cx - 55, cy - 55, cx + 55, cy + 55), INK)
    ell(d, (cx - 38, cy - 38, cx + 38, cy + 38), PINK_DARK)

    return img.resize((OUT, OUT), Image.LANCZOS)


if __name__ == "__main__":
    k = kitty()
    k.save("assets/kitty.png")
    k.save("assets/kittyparty.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    preview = Image.new("RGBA", (OUT * 2, OUT), (120, 170, 220, 255))
    preview.alpha_composite(k, (0, 0))
    preview.alpha_composite(k.transpose(Image.FLIP_LEFT_RIGHT), (OUT, 0))
    preview.save("/tmp/kitty_preview.png")
    print("ok")
