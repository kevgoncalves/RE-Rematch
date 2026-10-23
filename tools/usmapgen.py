"""Builds a .usmap from the static reflection tables (UECodeGen_Private) of a monolithic UE5 executable.

Only reads the file: never runs or injects into the game.
Usage: python -I usmapgen.py <exe> <out.usmap>
"""
import re
import struct
import sys

BASE = 0x140000000
IDENT = re.compile(rb'[A-Za-z_][A-Za-z0-9_]*\Z')

# EPropertyGenFlags (& 0x3F) -> usmap EPropertyType
GEN2USMAP = {
    0: 0, 1: 23, 2: 22, 3: 2, 4: 21, 5: 20, 6: 19, 7: 18, 8: 2, 9: 19, 10: 3, 11: 7, 12: 1,
    13: 17, 14: 14, 15: 15, 16: 17, 17: 4, 18: 4, 19: 12, 20: 5, 21: 10, 22: 8, 23: 24, 24: 25,
    25: 9, 26: 6, 27: 13, 28: 13, 29: 11, 30: 26, 31: 27, 32: 7, 33: 28,
}
READ_MORE = {22: 1, 23: 2, 24: 1, 30: 1, 33: 1}  # Array, Map, Set, Enum, Optional


class Exe:
    def __init__(self, path):
        self.d = d = open(path, 'rb').read()
        pe = struct.unpack_from('<I', d, 0x3c)[0]
        n, opt = struct.unpack_from('<H', d, pe + 6)[0], struct.unpack_from('<H', d, pe + 20)[0]
        self.secs = {}
        for i in range(n):
            name, vsz, va, rsz, rptr = struct.unpack_from('<8sIIII', d, pe + 24 + opt + i * 40)
            self.secs[name.rstrip(b'\0').decode()] = (BASE + va, vsz, rptr, rsz)

    def off(self, va):
        for sva, vsz, rp, rs in self.secs.values():
            if sva <= va < sva + rs:
                return rp + va - sva
        return None

    def isin(self, va, *names):
        for n in names:
            sva, vsz, _, _ = self.secs[n]
            if sva <= va < sva + vsz:
                return True
        return False

    def q(self, va): return struct.unpack_from('<Q', self.d, self.off(va))[0]
    def u32(self, va): return struct.unpack_from('<I', self.d, self.off(va))[0]
    def u16(self, va): return struct.unpack_from('<H', self.d, self.off(va))[0]
    def u8(self, va): return self.d[self.off(va)]

    def cstr(self, va):
        o = self.off(va) if va else None
        if o is None:
            return None
        e = self.d.find(b'\0', o, o + 1024)
        s = self.d[o:e]
        return s.decode() if e > o and all(32 <= c < 127 for c in s) else None

    def wstr(self, va):
        o = self.off(va) if va else None
        if o is None or o % 2:
            return None
        e = o
        while e < o + 512 and self.d[e:e + 2] != b'\0\0':
            e += 2
        s = self.d[o:e]
        try:
            t = s.decode('utf-16-le')
        except UnicodeDecodeError:
            return None
        return t if t and IDENT.match(t.encode()) else None

    def leas(self, fn):
        """(reg, target) of every lea [rip+x] up to the first ret."""
        o = self.off(fn)
        if o is None:
            return
        if self.d[o] == 0xE9:  # jmp thunk
            fn = fn + 5 + struct.unpack_from('<i', self.d, o + 1)[0]
            o = self.off(fn)
        for i in range(0x80):
            b = self.d[o + i]
            if b == 0xC3:
                return
            if b in (0x48, 0x4C) and self.d[o + i + 1] == 0x8D and (self.d[o + i + 2] & 0xC7) == 0x05:
                reg = ((self.d[o + i + 2] >> 3) & 7) + (8 if b == 0x4C else 0)
                yield reg, fn + i + 7 + struct.unpack_from('<i', self.d, o + i + 3)[0]


class Gen:
    def __init__(self, exe):
        self.x = exe
        self.cache = {}
        self.structs = {}   # name -> (super, props)
        self.enums = {}     # name -> [enumerator names]
        self.fail = 0
        self.bad = 0

    def params(self, fn):
        """Z_Construct_* function (or StaticStruct) -> address of its params."""
        if fn in self.cache:
            return self.cache[fn]
        r = None
        for reg, t in self.x.leas(fn):
            if reg == 2 and self.x.isin(t, '.rdata', '.data'):
                r = t
                break
            if reg == 1 and self.x.isin(t, '.text'):  # StaticStruct -> GetStaticStruct(Z_Construct_..., ...)
                r = self.params(t)
                if r:
                    break
        self.cache[fn] = r
        return r

    def struct_name(self, fn):
        p = self.params(fn) if fn else None
        if not p:
            return None
        name = self.x.cstr(self.x.q(p + 0x18))
        if not name or not IDENT.match(name.encode()):
            return None
        if name not in self.structs:
            self.structs[name] = None  # guards against recursion
            sup = self.struct_name(self.x.q(p + 0x08))
            self.structs[name] = (sup, self.props(self.x.q(p + 0x20), self.x.u16(p + 0x28)))
        return name

    def enum_name(self, fn):
        p = self.params(fn) if fn else None
        if not p:
            return None
        x = self.x
        name = x.cstr(x.q(p + 0x10))
        if name and name not in self.enums:
            arr, n = x.q(p + 0x20), struct.unpack_from('<h', x.d, x.off(p + 0x2c))[0]
            vals = []
            for i in range(max(n, 0)):
                s = x.cstr(x.q(arr + 16 * i)) or ''
                vals.append((x.q(arr + 16 * i + 8), s.split('::')[-1]))
            self.enums[name] = [s for _, s in sorted(vals)]
        return name

    def props(self, arr, n):
        if not arr or not n or self.x.off(arr) is None:
            return []
        ptrs = [self.x.q(arr + 8 * i) for i in range(n)]
        if not all(self.x.isin(p, '.rdata', '.data') for p in ptrs):
            self.bad += 1
            return []
        out, i = [], n
        while i > 0:
            prop, i = self.prop(ptrs, i)
            out.append(prop)
        return out[::-1]

    def prop(self, ptrs, i):
        x = self.x
        i -= 1
        pp = ptrs[i]
        gen = x.u8(pp + 0x18) & 0x3F
        p = {'name': x.cstr(x.q(pp)) or f'Unknown_{pp:x}', 'dim': x.u16(pp + 0x30),
             'type': GEN2USMAP.get(gen, 0xFF), 'inner': []}
        if gen == 25:
            p['struct'] = self.struct_name(x.q(pp + 0x38))
            if not p['struct']:
                self.fail += 1
        elif gen == 30:
            p['enum'] = self.enum_name(x.q(pp + 0x38))
        for _ in range(READ_MORE.get(gen, 0)):
            inner, i = self.prop(ptrs, i)
            p['inner'].append(inner)
        return p, i

    def run(self):
        x = self.x
        sva, _, rp, rs = x.secs['.rdata']
        d = x.d
        classes = 0
        for o in range(rp, rp + rs - 32, 8):
            q0, q1, q2, q3 = struct.unpack_from('<4Q', d, o)
            if not (x.isin(q0, '.text') and x.isin(q1, '.text') and x.isin(q3, '.data') and x.isin(q2, '.rdata')):
                continue
            name = x.wstr(q2)
            if not name:
                continue
            p = self.params(q0)
            if not p:
                continue
            if x.cstr(x.q(p + 0x18)):  # FStructParams: NameUTF8 at +0x18
                self.struct_name(q0)
            elif x.isin(x.q(p), '.text'):  # FClassParams
                bits = x.u32(p + 0x38)
                deps, ndeps, nprops = x.q(p + 0x18), bits & 0xF, (bits >> 15) & 0x7FF
                sup = None
                if ndeps:
                    sp = self.params(x.q(deps))
                    if sp and x.isin(x.q(sp), '.text') and sp in self.class_params:
                        sup = self.class_params[sp]
                self.pending.append((name[1:], p, deps, ndeps, nprops))
                self.class_params[p] = name[1:]
                classes += 1
        for name, p, deps, ndeps, nprops in self.pending:
            sup = None
            if ndeps:
                sup = self.class_params.get(self.params(x.q(deps)))
            self.structs[name] = (sup, self.props(x.q(p + 0x28), nprops))
        return classes

    def write(self, path):
        names, idx = [], {}

        def ni(s):
            if s not in idx:
                idx[s] = len(names)
                names.append(s)
            return idx[s]

        def typ(p, out):
            out += bytes([p['type']])
            if p['type'] == 26:
                typ(p['inner'][0], out)
                out += struct.pack('<I', ni(p.get('enum') or 'None'))
            elif p['type'] == 9:
                out += struct.pack('<I', ni(p.get('struct') or 'None'))
            elif p['type'] in (8, 25, 28):
                typ(p['inner'][0], out)
            elif p['type'] == 24:
                typ(p['inner'][0], out)
                typ(p['inner'][1], out)

        body = bytearray()
        body += struct.pack('<I', len(self.enums))
        for name, vals in self.enums.items():
            vals = vals[:255]
            body += struct.pack('<IB', ni(name), len(vals))
            for v in vals:
                body += struct.pack('<I', ni(v))
        real = {k: v for k, v in self.structs.items() if v is not None}
        body += struct.pack('<I', len(real))
        for name, (sup, props) in real.items():
            body += struct.pack('<II', ni(name), 0xFFFFFFFF if sup is None else ni(sup))
            body += struct.pack('<HH', sum(max(p['dim'], 1) for p in props), len(props))
            k = 0
            for p in props:
                body += struct.pack('<HBI', k, max(p['dim'], 1), ni(p['name']))
                typ(p, body)
                k += max(p['dim'], 1)
        head = bytearray(struct.pack('<I', len(names)))
        for s in names:
            b = s.encode()[:255]
            head += bytes([len(b)]) + b
        payload = bytes(head + body)
        with open(path, 'wb') as f:
            f.write(struct.pack('<HBBII', 0x30C4, 0, 0, len(payload), len(payload)))  # version Initial, uncompressed
            f.write(payload)


def main():
    g = Gen(Exe(sys.argv[1]))
    g.pending, g.class_params = [], {}
    n = g.run()
    g.write(sys.argv[2])
    real = sum(1 for v in g.structs.values() if v is not None)
    print(f'classes={n} structs+classes={real} enums={len(g.enums)} unresolved_structs={g.fail} invalid_arrays={g.bad}')
    for k in ('CurveFloat', 'RichCurve', 'RichCurveKey', 'RealCurve', 'Vector', 'DataTable'):
        v = g.structs.get(k)
        print(k, '->', v and (v[0], [(p['name'], p['type']) for p in v[1]][:8]))


if __name__ == '__main__':
    main()
