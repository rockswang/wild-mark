"""BMP 像素统计：程序化验证渲染确实发生了（非全白）、样式色出现（标题蓝/代码底/表格边框）。

用法: python scripts/bmp_stats.py <file.bmp> [...]
"""
import struct, sys, collections


def stats(path):
    b = open(path, "rb").read()
    assert b[:2] == b"BM", f"{path} 不是 BMP"
    off = struct.unpack("<I", b[10:14])[0]
    w, h = struct.unpack("<ii", b[18:26])
    bpp = struct.unpack("<H", b[28:30])[0]
    row = ((w * bpp // 8) + 3) // 4 * 4
    px = b[off:]
    total = w * h
    cnt = collections.Counter()
    nonwhite = 0
    for y in range(h):
        base = y * row
        for x in range(w):
            i = base + x * 3
            c = (px[i], px[i + 1], px[i + 2])  # BGR
            cnt[c] += 1
            if c != (255, 255, 255):
                nonwhite += 1
    top = cnt.most_common(6)
    # 关键样式色（BGR）：标题蓝 0x0b5ca8 -> (a8,5c,0b)；链接蓝 0x0060c0；代码底 0xf2f4f7
    def has(c, tol=6):
        return any(all(abs(a - b) <= tol for a, b in zip(k, c)) for k in cnt)
    blue = has((0xa8, 0x5c, 0x0b)) or has((0xc0, 0x60, 0x00))
    codebg = has((0xf7, 0xf4, 0xf2)) or has((0xed, 0xee, 0xe8))
    gray = has((0xc8, 0xc8, 0xc8))
    tops = [(f"#{c[2]:02x}{c[1]:02x}{c[0]:02x}", n) for c, n in top]
    print(f"{path}: {w}x{h} bpp={bpp} 非白像素 {nonwhite*100//total}%  唯一色 {len(cnt)}")
    print(f"   标题/链接蓝={blue} 代码底={codebg} 边框灰={gray}")
    print(f"   top colors: {tops}")
    return nonwhite > 0 and (blue or len(cnt) > 8)


if __name__ == "__main__":
    ok = True
    for p in sys.argv[1:]:
        try:
            ok &= stats(p)
        except Exception as e:
            print(f"{p}: 分析失败 {e}")
            ok = False
    sys.exit(0 if ok else 1)
