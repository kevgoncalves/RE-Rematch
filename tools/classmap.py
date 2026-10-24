"""Lists the game's own reflected classes (those registered from the game modules' code), with
their parent, properties and UFUNCTIONs, as a local map for finding where each mechanic lives.

A class belongs to the game when its registration function sits inside one of the game modules'
code ranges (SCCore, SCCombatSystem, Runtime). Only reads the file.
Usage: python -I classmap.py <exe> <out.md>
"""
import sys

from natives import class_functions
from usmapgen import Exe, Gen

GAME_RANGES = [(0x1446F0000, 0x144A00000), (0x1451C0000, 0x1451F0000), (0x145250000, 0x145460000)]


def main():
    x = Exe(sys.argv[1])
    g = Gen(x)
    g.pending, g.class_params = [], {}
    g.run()
    funcs = class_functions(g)
    game = []
    for p, name in g.class_params.items():
        reg = x.q(p)  # ClassNoRegisterFunc
        if any(lo <= reg < hi for lo, hi in GAME_RANGES):
            game.append(name)
    with open(sys.argv[2], 'w', encoding='utf-8') as f:
        for name in sorted(game):
            sup, props = g.structs.get(name) or (None, [])
            f.write(f'## {name} : {sup}\n')
            if props:
                f.write('props: ' + ', '.join(p['name'] for p in props) + '\n')
            if funcs.get(name):
                f.write('funcs: ' + ', '.join(sorted(funcs[name])) + '\n')
            f.write('\n')
    print(f'game classes: {len(game)} of {len(g.class_params)}')


if __name__ == '__main__':
    main()
