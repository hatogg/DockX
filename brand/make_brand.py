# Generates the DockX icon, installer bitmaps and GitHub artwork (PIL).
from PIL import Image, ImageDraw, ImageFilter, ImageFont
import os
OUT = os.path.dirname(os.path.abspath(__file__))
RED = (226, 38, 50)

def rr(d, box, r, **kw):
    d.rounded_rectangle(box, r, **kw)

def icon(size=1024):
    S = size
    img = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    # shadow
    sh = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    ImageDraw.Draw(sh).rounded_rectangle((70, 90, S - 70, S - 50), 210, fill=(0, 0, 0, 150))
    img.alpha_composite(sh.filter(ImageFilter.GaussianBlur(30)))
    body = Image.new('RGBA', (S, S), (0, 0, 0, 0))
    grad = Image.new('RGBA', (S, S))
    gd = ImageDraw.Draw(grad)
    for y in range(S):
        t = y / S
        c = tuple(int(a + (b - a) * t) for a, b in zip((40, 40, 45), (10, 10, 12)))
        gd.line([(0, y), (S, y)], fill=c + (255,))
    mask = Image.new('L', (S, S), 0)
    ImageDraw.Draw(mask).rounded_rectangle((64, 64, S - 64, S - 64), 200, fill=255)
    body.paste(grad, (0, 0), mask)
    img.alpha_composite(body)
    d = ImageDraw.Draw(img)
    rr(d, (64, 64, S - 64, S - 64), 200, outline=(255, 255, 255, 46), width=6)
    # island (pill) on top
    rr(d, (300, 150, 724, 250), 50, fill=(0, 0, 0, 255), outline=(70, 70, 78, 255), width=5)
    d.ellipse((330, 170, 390, 230), fill=RED + (255,))
    for i, h in enumerate((30, 52, 38, 58)):
        x = 600 + i * 26
        rr(d, (x, 200 - h // 2, x + 14, 200 + h // 2), 7, fill=(240, 240, 240, 255))
    # dock shelf
    rr(d, (150, 600, 874, 800), 70, fill=(52, 52, 58, 255), outline=(255, 255, 255, 70), width=6)
    # tiles: magnified red one
    rr(d, (205, 420, 425, 640), 52, fill=RED + (255,))
    rr(d, (455, 520, 615, 680), 40, fill=(236, 236, 236, 255))
    rr(d, (645, 520, 805, 680), 40, fill=(128, 128, 136, 255))
    d.ellipse((300, 735, 330, 765), fill=(240, 240, 240, 255))
    return img

def save_ico(img, path):
    sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
    img.resize((256, 256), Image.LANCZOS).save(path, sizes=sizes)

def font(name, size):
    for p in ['/usr/share/fonts/opentype/inter/Inter-%s.otf' % name]:
        if os.path.exists(p):
            return ImageFont.truetype(p, size)
    return ImageFont.load_default()

def installer_bitmaps(ic):
    # welcome/finish: 164x314 (MUI), drawn at 2x then downscaled
    W, H = 328, 628
    im = Image.new('RGB', (W, H), (10, 10, 12))
    glow = Image.new('RGBA', (W, H), (0, 0, 0, 0))
    ImageDraw.Draw(glow).ellipse((-120, 300, 360, 780), fill=RED + (120,))
    im.paste(glow.filter(ImageFilter.GaussianBlur(80)), (0, 0), glow.filter(ImageFilter.GaussianBlur(80)))
    im = im.convert('RGBA')
    im.alpha_composite(ic.resize((200, 200), Image.LANCZOS), (64, 120))
    d = ImageDraw.Draw(im)
    d.text((W // 2, 380), 'DockX', font=font('Bold', 58), fill=(245, 245, 245), anchor='mm')
    d.text((W // 2, 430), 'док + Dynamic Island', font=font('Regular', 22), fill=(190, 190, 196), anchor='mm')
    d.text((W // 2, 590), 'by X44TO', font=font('SemiBold', 20), fill=(226, 38, 50), anchor='mm')
    im.convert('RGB').resize((164, 314), Image.LANCZOS).save(os.path.join(OUT, 'installer-side.bmp'))
    # header: 150x57
    h = Image.new('RGB', (300, 114), (255, 255, 255)).convert('RGBA')
    h.alpha_composite(ic.resize((96, 96), Image.LANCZOS), (196, 9))
    h.convert('RGB').resize((150, 57), Image.LANCZOS).save(os.path.join(OUT, 'installer-header.bmp'))

if __name__ == '__main__':
    ic = icon()
    ic.save(os.path.join(OUT, 'icon-1024.png'))
    ic.resize((256, 256), Image.LANCZOS).save(os.path.join(OUT, 'icon-256.png'))
    save_ico(ic, os.path.join(OUT, '..', 'res', 'dockx.ico'))
    installer_bitmaps(ic)
    print('ok')
