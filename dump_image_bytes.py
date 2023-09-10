import sys
import numpy as np
from PIL import Image

img = Image.open(sys.argv[1])
img = np.array(img)

with open(sys.argv[2], "wb") as f:
    f.write(img.tobytes())
