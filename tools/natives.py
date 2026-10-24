"""Names the game's native functions from the static reflection tables, for decompiling them.

For every reflected class it reads the UFUNCTION names, then finds the tables that register each
UFUNCTION's native entry point (name -> exec thunk) and matches them to their class. The output
maps addresses to Class::Function names; the decompiles made from it stay local.
Only reads the file: never runs or injects into the game.
Usage: python -I natives.py <exe> <out.tsv>
"""
import struct
import sys

from usmapgen import IDENT, Exe, Gen


def class_functions(g):
    """Class name -> set of its UFUNCTION names (from FClassParams' function link array)."""
    x = g.x
    out = {}
    for p, name in g.class_params.items():
        bits = x.u32(p + 0x38)
        arr, n = x.q(p + 0x20), (bits >> 4) & 0x7FF
        names = set()
        for i in range(n if arr and x.off(arr) is not None else 0):
            s = x.cstr(x.q(arr + 16 * i + 8))
            if s:
                names.add(s)
        out[name] = names
    return out


def native_tables(x):
    """Runs of (name, exec thunk) pairs in .rdata: one per class that has native UFUNCTIONs."""
    sva, _, rp, rs = x.secs['.rdata']
    d, runs, cur = x.d, [], []
    o = rp
    while o < rp + rs - 16:
        q0, q1 = struct.unpack_from('<2Q', d, o)
        s = x.cstr(q0) if x.isin(q0, '.rdata') and x.isin(q1, '.text') else None
        if s and IDENT.match(s.encode()):
            cur.append((s, q1))
            o += 16
            continue
        if len(cur) >= 1:
            runs.append(cur)
        cur = []
        o += 8
    return runs


def main():
    x = Exe(sys.argv[1])
    g = Gen(x)
    g.pending, g.class_params = [], {}
    g.run()
    funcs = class_functions(g)
    owner = {}
    for fname_set_class, names in funcs.items():
        for n in names:
            owner.setdefault(n, set()).add(fname_set_class)
    rows, matched, unmatched = [], 0, 0
    for run in native_tables(x):
        names = {n for n, _ in run}
        # The class whose UFUNCTIONs contain this whole table (smallest such class wins).
        cands = [c for c, fs in funcs.items() if names <= fs]
        if not cands:
            unmatched += 1
            continue
        cls = min(cands, key=lambda c: len(funcs[c]))
        matched += 1
        rows += [(addr, f'{cls}::exec{n}') for n, addr in run]
    with open(sys.argv[2], 'w', encoding='utf-8') as f:
        for addr, name in sorted(rows):
            f.write(f'{addr:x}\t{name}\n')
    print(f'classes={len(funcs)} with_functions={sum(1 for v in funcs.values() if v)} '
          f'native_tables={matched} unmatched_tables={unmatched} thunks={len(rows)}')


if __name__ == '__main__':
    main()
