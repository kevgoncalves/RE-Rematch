#!/bin/sh
# Builds oo2core_9_win64.dll (decompression only) from ooz (GPL-3, github.com/powzix/ooz) with MinGW g++.
# Usage: sh build.sh <short-temp-dir>   (Windows git fails on long paths)
set -e
W=${1:-$TEMP/rre}
SHIM=$(cd "$(dirname "$0")" && pwd)/shim.cpp
mkdir -p "$W"
[ -d "$W/ooz" ] || git clone -q https://github.com/powzix/ooz.git "$W/ooz"
git -C "$W/ooz" checkout -q 05038060aa68f9187ae9923b2388ca8db40e58d1
cd "$W/ooz"
# the CLI main() is renamed so only the decoder is used from the DLL
g++ -O2 -msse4.1 -shared -static -fpermissive -w -Dmain=ooz_cli_main -include sys/stat.h \
  kraken.cpp lzna.cpp bitknit.cpp "$SHIM" -o "$W/oo2core_9_win64.dll"
echo "$W/oo2core_9_win64.dll"
