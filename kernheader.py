import os
import sys
import struct
import subprocess
from ast import literal_eval

args = sys.argv

if len(args) != 3:
    exit(1)

kernname = args[1]
headername = args[2]
 
file_size = os.path.getsize(kernname)

sizebytes = struct.pack('<I', file_size)

# /usr/local/opt/binutils/bin/readelf -h target/x86_64-my_os/debug/my_kernel
out = subprocess.run(['readelf', '-h', kernname], stdout=subprocess.PIPE).stdout.decode('utf-8')
epointString = out.split("\n")[10].split(":")[1].strip()
entryPoint = literal_eval(epointString)
entryPointBytes = struct.pack('<I', entryPoint)

headersize = 512
with open(headername, "wb") as f:
    magic = b'\x99\x00\x11\x22' # header magic
    f.write(magic)
    headersize -= len(magic)

    f.write(sizebytes)
    headersize -= len(sizebytes)

    f.write(entryPointBytes)
    headersize -= len(entryPointBytes)

    f.write(b'\x00'*headersize)

