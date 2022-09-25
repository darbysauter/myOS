import os
import sys
import struct
import subprocess
from ast import literal_eval
from os import listdir
from os.path import isfile, join

def round_to_multiple(number, multiple):
    return multiple * round((number + (multiple / 2.0)) / multiple)

args = sys.argv

fs_image_name = args[1]
dir = args[2]

files = [f for f in listdir(dir) if isfile(join(dir, f))]
num_files = len(files)
num_files_bytes = struct.pack('<I', num_files)

trailing_padding = []

sector_size = 512
padding_after_string = 0
with open(fs_image_name, "wb") as f:
    magic = b'\x77\x77\x12\x34' # header magic
    f.write(magic)
    sector_size -= len(magic)
    padding_after_string += len(magic)

    f.write(num_files_bytes)
    sector_size -= len(num_files_bytes)
    padding_after_string += len(num_files_bytes)

    for file in files:
        s = file.encode() + b'\x00'
        f.write(s)
        sector_size -= len(s)
        padding_after_string += len(s)

    pad = round_to_multiple(padding_after_string, 8) - padding_after_string

    f.write(b'\x00'*pad)
    sector_size -= pad

    offset = 512
    for file in files:
        file_size = os.path.getsize(join(dir, file))
        file_size_bytes = struct.pack('<Q', file_size)
        offset_bytes = struct.pack('<Q', offset)
        f.write(offset_bytes)
        sector_size -= len(offset_bytes)
        f.write(file_size_bytes)
        sector_size -= len(file_size_bytes)
        rounded = round_to_multiple(offset+file_size, 512)
        padding = rounded - (offset+file_size)
        offset = rounded
        trailing_padding.append(padding)

    f.write(b'\x00'*sector_size)

    i = 0
    for file in files:
        with open(join(dir, file), "rb") as prog:
            f.write(prog.read())
        f.write(b'\x00'*trailing_padding[i])
        i += 1

