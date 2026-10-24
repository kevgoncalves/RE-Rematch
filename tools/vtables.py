"""Finds the virtual function table of every game class and names the slots each class overrides,
so the decompiled game modules show which class a virtual function belongs to.

Path, per class: its StaticClass function (from the reflection tables) passes the class's
constructor as the 10th argument of the class registration call; the constructor writes the
class's vtable into the object. Slots that differ from the parent class's vtable are the class's
overrides; slots past the parent's end are new virtuals. Output rows: address, Class__vfNN
(NN = slot index). Only reads the file.
Usage: python -I vtables.py <exe> <out.tsv> [Class ...]
"""
import re
import struct
import sys

from classmap import GAME_RANGES
from usmapgen import Exe, Gen

LEA_RIP = re.compile(rb'[\x48\x4c]\x8d[\x05\x0d\x15\x1d\x25\x2d\x35\x3d]')
# mov [rsp+0x48], r64: the 10th call argument on the stack.
TO_ARG10 = re.compile(rb'[\x48\x4c]\x89[\x44\x4c\x54\x5c\x64\x6c\x74\x7c]\x24\x48')
# mov [r64], r64 with no displacement: writing the vtable pointer into the object.
TO_OBJECT = re.compile(rb'[\x48\x49\x4c\x4d]\x89[\x00-\x3f]')
MOV_R11_RSP = b'\x4c\x8b\xdc'
SUB_RSP = re.compile(rb'\x48\x83\xec(.)|\x48\x81\xec(....)', re.S)


def body(x, fn, limit=0x1000):
    """The bytes of fn up to the int3 padding that follows it (MSVC pads functions with 0xCC)."""
    o = x.off(fn)
    if o is None:
        return b''
    end = x.d.find(b'\xcc\xcc', o, o + limit)
    return x.d[o:end if end > 0 else o + limit]


def table_len(x, va):
    """How many function pointers in a row start at va (0 if it isn't a vtable)."""
    n = 0
    while x.off(va + 8 * n) is not None and x.isin(x.q(va + 8 * n), '.text'):
        n += 1
        if n > 600:
            break
    return n


def lea_then(code, fn, follow):
    """Targets of `lea r64,[rip+d]` whose next instruction matches `follow`."""
    out = []
    for m in LEA_RIP.finditer(code):
        i = m.start()
        if i + 7 <= len(code) and follow.match(code, i + 7):
            out.append(fn + i + 7 + struct.unpack_from('<i', code, i + 3)[0])
    return out


def arg10_store(code):
    """Pattern for storing the 10th call argument: by rsp, or by r11 after `mov r11, rsp`."""
    m = SUB_RSP.search(code, 0, 16)
    if not code.startswith(MOV_R11_RSP) or not m:
        return TO_ARG10
    frame = m.group(1)[0] if m.group(1) else struct.unpack('<I', m.group(2))[0]
    disp = (0x48 - frame) & 0xFF
    return re.compile(rb'\x49\x89[\x43\x4b\x53\x5b\x63\x6b\x73\x7b]' + re.escape(bytes([disp])))


def stored_tables(x, code, fn):
    """vtables a function loads with `lea reg,[rip+d]` and then writes, with that same register,
    into an object (`mov [r64], reg`, no displacement) within the next few instructions."""
    out = []
    for m in LEA_RIP.finditer(code):
        i = m.start()
        if i + 7 > len(code):
            break
        reg =((code[i] >> 2) & 1) << 3 | (code[i + 2] >> 3) & 7
        t = fn + i + 7 + struct.unpack_from('<i', code, i + 3)[0]
        if not x.isin(t, '.rdata') or table_len(x, t) < 8:
            continue
        for j in range(i + 7, min(i + 0x200, len(code) - 2)):
            rex, op, modrm = code[j], code[j + 1], code[j + 2]
            if (rex & 0xF8) == 0x48 and op == 0x89 and modrm >> 6 == 0 and (modrm & 7) not in (4, 5) \
                    and ((rex >> 2) & 1) << 3 | (modrm >> 3) & 7 == reg:
                out.append(t)
                break
    return out


def branch_targets(code, fn):
    """Targets of jmp rel32 and conditional jumps rel32 in code (calls are helpers, not the
    constructor the wrapper hands over to)."""
    out = []
    for m in re.finditer(rb'\xe9|\x0f[\x80-\x8f]', code):
        i, n = m.start(), len(m.group())
        if i + n + 4 <= len(code):
            out.append(fn + i + n + 4 + struct.unpack_from('<i', code, i + n)[0])
    return out


def vtable_of(x, static_class):
    """The vtable the class's constructor writes (the last one, after any inlined parent). The
    constructor handed to the registration is often a small wrapper that jumps to the real one."""
    code = body(x, static_class)
    for ctor in lea_then(code, static_class, arg10_store(code)):
        wrapper = body(x, ctor)
        tables = stored_tables(x, wrapper, ctor)
        if not tables and len(wrapper) < 0x40:
            for real in branch_targets(wrapper, ctor):
                if x.isin(real, '.text'):
                    tables = stored_tables(x, body(x, real), real) or tables
        if tables:
            return tables[-1]
    return None


def main():
    x = Exe(sys.argv[1])
    g = Gen(x)
    g.pending, g.class_params = [], {}
    g.run()
    by_name = {name: p for p, name in g.class_params.items()}
    only = set(sys.argv[3:])
    # Game classes only: engine ancestors aren't needed (only game code gets named) and their
    # tables, detected less reliably, would spoil the bounds below.
    vt = {name: t for name, p in by_name.items()
          if any(lo <= x.q(p) < hi for lo, hi in GAME_RANGES) and (t := vtable_of(x, x.q(p)))}
    # Without RTTI nothing separates one vtable from the next: end each at the next known one.
    starts = sorted(set(vt.values()))
    nxt = {a: b for a, b in zip(starts, starts[1:])}
    length = {c: min(table_len(x, t), (nxt.get(t, t + 8 * 601) - t) // 8) for c, t in vt.items()}

    def parent_of(c):
        """Nearest ancestor with a known vtable."""
        sup = (g.structs.get(c) or (None, None))[0]
        while sup and sup not in vt:
            sup = (g.structs.get(sup) or (None, None))[0]
        return sup

    # A class's vtable can't be longer than any of its children's.
    for c in vt:
        p = parent_of(c)
        if p:
            length[p] = min(length[p], length[c])

    rows, found = [], 0
    for name, p in by_name.items():
        if (only and name not in only) or name not in vt or not any(lo <= x.q(p) < hi for lo, hi in GAME_RANGES):
            continue
        found += 1
        sup = parent_of(name)
        parent = vt.get(sup)
        pn = length[sup] if parent else 0
        # New virtuals past the parent's end are few; more usually means the next table.
        mine, n = vt[name], min(length[name], pn + 24) if parent else length[name]
        # Only the game's own code: inherited engine functions keep their engine identity.
        mine_rows = [(f, f'{name}__vf{i}') for i in range(n)
                     if (i >= pn or (f := x.q(mine + 8 * i)) != x.q(parent + 8 * i))
                     and any(lo <= (f := x.q(mine + 8 * i)) < hi for lo, hi in GAME_RANGES)]
        rows += mine_rows
        if only:
            print(name, 'vtable', hex(mine), 'slots', n, 'parent', sup, pn, 'overrides', [r[1].split('__')[1] for r in mine_rows])
    with open(sys.argv[2], 'w', encoding='utf-8') as out:
        for f, n in sorted(set(rows)):
            out.write(f'{f:x}\t{n}\n')
    print(f'classes with a vtable: {len(vt)}; game classes: {found}; named slots: {len(rows)}')


if __name__ == '__main__':
    main()
