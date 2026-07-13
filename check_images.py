from PIL import Image
import os

files = [
    'demo/压缩/photo_2026-07-13_16-33-15.jpg',
    'demo/压缩/photo_2026-07-13_16-33-22.jpg',
    'demo/压缩/photo_2026-07-13_16-33-26.jpg',
    'demo/差分/1.jpg',
    'demo/差分/1-1.jpg',
    'demo/差分/1-2.jpg'
]

for f in files:
    img = Image.open(f)
    size_kb = os.path.getsize(f) / 1024
    print(f"{os.path.basename(f)}: {img.size[0]}x{img.size[1]} ({size_kb:.2f}KB)")
