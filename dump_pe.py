import sys
import pefile

#
# Warning: this is VERY DUMB
# PE code is NOT position-independent, and we're not relocating the sections.
# So it probably only works for very simple programs.

input_path = sys.argv[1]
output_path = sys.argv[2]

pe = pefile.PE(input_path)

max_addr = 0
for section in pe.sections:
    start_addr = section.VirtualAddress
    size = section.Misc_VirtualSize
    print(f"{section.Name} 0x{start_addr:x} 0x{size:x}")
    max_addr = max(max_addr, start_addr + size)

entrypoint = pe.OPTIONAL_HEADER.AddressOfEntryPoint
buffer = bytearray(max_addr)

for section in pe.sections:
    start_addr = section.VirtualAddress
    size = section.Misc_VirtualSize
    buffer[start_addr:start_addr+size] = section.get_data()

with open(output_path, "wb") as f:
    f.write(buffer)

print(f"{entrypoint=:0x}")
