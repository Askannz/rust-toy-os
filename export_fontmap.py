import sys
import cv2
import numpy as np

img = cv2.imread(sys.argv[1])
img = cv2.cvtColor(img, cv2.COLOR_BGR2GRAY)
img = (img > 0).astype(np.uint8) * 255
print(img.shape)

with open("font_map.bin", "wb") as f:
    f.write(img.tobytes())
