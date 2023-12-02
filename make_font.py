import sys
from math import ceil
from PIL import Image, ImageDraw, ImageFont

def main():

    font_path = sys.argv[1]
    font_size = int(sys.argv[2])

    chars = [chr(i) for i in range(32, 127)]

    font = ImageFont.truetype(font_path, font_size)
    char_h = sum(font.getmetrics())

    char_w_set = set(font.getlength(c) for c in chars)
    assert len(char_w_set) == 1
    char_w = int(ceil(char_w_set.pop()))

    print(char_w)

    image = Image.new("L", (char_w * len(chars), char_h))

    draw = ImageDraw.Draw(image)

    for i, c in enumerate(chars):
        draw.text((i * char_w, 0), c, font=font, fill=255)

    image.save("font.png")

    print(f"nb_chars: {len(chars)}")
    print(f"char_h: {char_h}")
    print(f"char_w: {char_w}")

if __name__ == "__main__":
    main()
