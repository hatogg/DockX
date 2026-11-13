# Profile card (same format as the OptimaX card) and README banner.
from PIL import Image, ImageDraw, ImageFilter, ImageFont
import os
HERE = os.path.dirname(os.path.abspath(__file__))
DOCS = os.path.join(HERE, '..', 'docs')
RED = (226, 38, 50)

def F(w, s):
    return ImageFont.truetype('/usr/share/fonts/opentype/inter/Inter-%s.otf' % w, s)

def background(W, H, rad):
    bg = Image.new('RGBA', (W, H))
    d = ImageDraw.Draw(bg)
    for y in range(H):
        t = y / H
        c = tuple(int(a + (b - a) * t) for a, b in zip((28, 12, 16), (9, 9, 11)))
        d.line([(0, y), (W, y)], fill=c + (255,))
    glow = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    gd = ImageDraw.Draw(glow)
    gd.ellipse((-W * 0.25, H * 0.35, W * 0.55, H * 1.4), fill=RED + (110,))
    gd.ellipse((W * 0.55, -H * 0.5, W * 1.3, H * 0.6), fill=(150, 20, 34, 90))
    bg.alpha_composite(glow.filter(ImageFilter.GaussianBlur(min(W, H) // 5)))
    mask = Image.new('L', (W, H), 0)
    ImageDraw.Draw(mask).rounded_rectangle((0, 0, W - 1, H - 1), rad, fill=255)
    out = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    out.paste(bg, (0, 0), mask)
    return out, mask

def chip(d, x, y, text, font, pad=26, h=64):
    w = d.textlength(text, font=font) + 2 * pad
    d.rounded_rectangle((x, y, x + w, y + h), h // 2, fill=(255, 255, 255, 34))
    d.text((x + w / 2, y + h / 2), text, font=font, fill=(255, 255, 255, 240), anchor='mm')
    return w

def screen(img, w, rad, border=2):
    s = img.resize((w, int(img.height * w / img.width)), Image.LANCZOS).convert('RGBA')
    m = Image.new('L', s.size, 0)
    ImageDraw.Draw(m).rounded_rectangle((0, 0, s.width - 1, s.height - 1), rad, fill=255)
    out = Image.new('RGBA', s.size, (0, 0, 0, 0))
    out.paste(s, (0, 0), m)
    ImageDraw.Draw(out).rounded_rectangle((0, 0, s.width - 1, s.height - 1), rad, outline=(255, 255, 255, 60), width=border)
    return out

def shadowed(base, im, x, y, blur=24, alpha=170):
    sh = Image.new('RGBA', base.size, (0, 0, 0, 0))
    m = im.split()[3].point(lambda v: alpha if v > 0 else 0)
    sh.paste((0, 0, 0, 255), (x + 8, y + 16), m)
    base.alpha_composite(sh.filter(ImageFilter.GaussianBlur(blur)))
    base.alpha_composite(im, (x, y))

def compact_desktop():
    full = Image.open(os.path.join(DOCS, 'full_music.png')).convert('RGB')
    x0, x1 = 300, 1620
    top = full.crop((x0, 0, x1, 300))
    mid = full.crop((x0, 420, x1, 470)).resize((x1 - x0, 60))
    bot = full.crop((x0, 960, x1, 1080))
    out = Image.new('RGB', (x1 - x0, top.height + mid.height + bot.height))
    out.paste(top, (0, 0)); out.paste(mid, (0, top.height)); out.paste(bot, (0, top.height + mid.height))
    return out

def profile_card():
    W, H = 1120, 660
    card, mask = background(W, H, 64)
    icon = Image.open(os.path.join(HERE, 'icon-1024.png')).resize((140, 140), Image.LANCZOS)
    card.alpha_composite(icon, (54, 50))
    sc = screen(compact_desktop(), 760, 22)
    layer = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    shadowed(layer, sc, 610, 170)
    clip = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    clip.paste(layer, (0, 0), mask)
    card.alpha_composite(clip)
    ov = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(ov)
    d.text((62, 210), 'DockX', font=F('Black', 112), fill=(255, 255, 255))
    sub = F('Medium', 36)
    d.text((64, 350), 'Док як у macOS', font=sub, fill=(255, 255, 255, 215))
    d.text((64, 396), 'і Dynamic Island', font=sub, fill=(255, 255, 255, 215))
    d.text((64, 442), 'для Windows', font=sub, fill=(255, 255, 255, 215))
    f = F('SemiBold', 30)
    x = 64
    x += chip(d, x, 540, 'Dynamic Island', f) + 18
    chip(d, x, 540, 'Rust · 1,5 МБ', f)
    card.alpha_composite(ov)
    card.save(os.path.join(DOCS, 'card.png'))

def banner():
    W, H = 1280, 640
    b, _ = background(W, H, 0)
    sc = screen(compact_desktop(), 800, 20)
    shadowed(b, sc, W - 800 - 50, (H - sc.height) // 2)
    icon = Image.open(os.path.join(HERE, 'icon-1024.png')).resize((120, 120), Image.LANCZOS)
    b.alpha_composite(icon, (60, 70))
    ov = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(ov)
    d.text((60, 210), 'DockX', font=F('Black', 96), fill=(255, 255, 255))
    d.text((62, 330), 'Док + Dynamic Island', font=F('Medium', 30), fill=(255, 255, 255, 220))
    d.text((62, 370), 'для Windows 10 / 11', font=F('Medium', 30), fill=(255, 255, 255, 220))
    d.text((62, 560), 'by X44TO', font=F('Bold', 26), fill=RED)
    b.alpha_composite(ov)
    b.convert('RGB').save(os.path.join(DOCS, 'banner.png'), optimize=True)

if __name__ == '__main__':
    profile_card()
    banner()
    print('ok')
