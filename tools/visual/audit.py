#!/usr/bin/env python3
"""陰関数の成績表: 全部の陰関数を、式によらない同じ物差しで測る。

    python3 tools/visual/audit.py <出力先>            # 全部（数分かかる）
    AUDIT_ONLY=mod python3 tools/visual/audit.py <出力先>   # 名前か式にその文字を含むものだけ

tools/visual/cases.tsv の陰関数（設定の付いていない行）ごとに、次の版を作って比べる。
先に core で `cargo build --release --example dump_meshes` をしておく。
出力先に mesh/meta.tsv がもうあれば、メッシュは作り直さない（AUDIT_REDUMP=1 で作り直す）。

- 基準: 細かさ 160・ふつうの計算
- 80: 細かさ 80（アプリの既定）、p40・p24: 回転の再生中と同じ軽い計算（flags=1）で細かさ 40・24
- それぞれ、回さないものと、回したもの（xy 14°・xz 10°。基準も同じだけ回した式）

物差し（式ごとの正解は使わない）:

- 欠け: 基準の面の上の点のうち、近く（その版の格子の 1 マスほど以内）に面が無い点の割合（%）
- 余計: その版の面の上の点のうち、近くに基準の面が無い点の割合（%）
- 裏返り: 同じ向きで 2 つ以上の三角形に使われている辺の割合（%）。隣どうしで表裏が逆の所と、面が枝分かれする所で増える
- 破片: 頂点でつながった成分のうち、三角形が 12 個以下のものの数

基準も同じプログラムで作るので、細かくしても直らない間違い（どの細かさでも同じように欠ける面など）は見つからない。
それは、正解の分かっている式のテスト（core の mesh::pieces::tests など）で確かめる。
ここで見るのは「粗くしたとき・回したときに崩れないか」。
"""
import os
import struct
import subprocess
import sys

import numpy as np

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
DUMP = os.environ.get("AUDIT_DUMP") or os.path.join(ROOT, "core/target/release/examples/dump_meshes")
LO = np.array([-10.0, -10.0, -6.0])
HI = np.array([10.0, 10.0, 6.0])
ROT = "xy=14,xz=10"
# (名前, 設定, 細かさ)
VARIANTS = [("ref", "ires=160", 160), ("80", "ires=80", 80), ("p40", "flags=1,ires=40", 40), ("p24", "flags=1,ires=24", 24)]


def cases():
    only = os.environ.get("AUDIT_ONLY")
    out = []
    for line in open(os.path.join(ROOT, "tools/visual/cases.tsv"), encoding="utf8"):
        line = line.rstrip("\n")
        if not line or line.startswith("#"):
            continue
        name, opts, src = line.split("\t")
        # 陰関数（= のある式か、変数 z を含む式）で、設定の付いていないもの
        if opts != "0" or name[0] not in "suij" or name in ("s_stairs", "s_saddle", "s_gauss", "s_absgamma", "s_abszeta", "s_bessel"):
            continue
        if only and only not in name and only not in src:
            continue
        out.append((name, src))
    return out


def load(path):
    if not os.path.exists(path):
        return np.zeros((0, 3)), np.zeros((0, 3), dtype=np.int64)
    d = open(path, "rb").read()
    nv, ni = struct.unpack("<II", d[:8])
    p = np.frombuffer(d, "<f4", nv * 3, 8).reshape(-1, 3).astype(np.float64)
    i = np.frombuffer(d, "<u4", ni, 8 + nv * 12).reshape(-1, 3).astype(np.int64)
    return p, i


def samples(p, i):
    """面の上の点（頂点・辺の中点・重心）。表示範囲の中だけ"""
    if len(i) == 0:
        return np.zeros((0, 3))
    a, b, c = p[i[:, 0]], p[i[:, 1]], p[i[:, 2]]
    q = np.concatenate([a, b, c, (a + b) / 2, (b + c) / 2, (c + a) / 2, (a + b + c) / 3])
    q = q[np.isfinite(q).all(1)]
    q = q[((q > LO) & (q < HI)).all(1)]
    # 多すぎる点は間引く（毎回同じ点を選ぶ）
    if len(q) > 400_000:
        q = q[np.random.default_rng(0).choice(len(q), 400_000, replace=False)]
    return q


def keys(q, tol):
    c = np.floor((q - LO) / tol).astype(np.int64) + 4
    return c[:, 0] * 4096 * 4096 + c[:, 1] * 4096 + c[:, 2]


def far_fraction(src, dst, tol, margin):
    """src の点（表示範囲の端から margin より内側）のうち、近くに dst の点が無いものの割合（%）"""
    src = src[((src > LO + margin) & (src < HI - margin)).all(1)]
    if len(src) == 0:
        return 0.0
    if len(dst) == 0:
        return 100.0
    have = np.unique(keys(dst, tol))
    k = keys(src, tol)
    near = np.zeros(len(k), dtype=bool)
    for dx in (-1, 0, 1):
        for dy in (-1, 0, 1):
            for dz in (-1, 0, 1):
                near |= np.isin(k + dx * 4096 * 4096 + dy * 4096 + dz, have)
    return 100.0 * (1.0 - near.mean())


def main():
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    cs = cases()
    lines = ["# 陰関数の成績表用（audit.py が作る）"]
    for name, src in cs:
        for rot in ("", ROT):
            for v, opts, _ in VARIANTS:
                tag = f"{name}{'_r' if rot else ''}_{v}"
                lines.append(f"{tag}\t0,{opts}{',' + rot if rot else ''}\t{src}")
    tsv = os.path.join(out, "audit_cases.tsv")
    open(tsv, "w", encoding="utf8").write("\n".join(lines) + "\n")
    if os.environ.get("AUDIT_REDUMP") or not os.path.exists(os.path.join(out, "mesh", "meta.tsv")):
        subprocess.run([DUMP, tsv, os.path.join(out, "mesh")], check=True, stderr=subprocess.DEVNULL)
    meta = {}
    for line in open(os.path.join(out, "mesh", "meta.tsv"), encoding="utf8"):
        f = line.rstrip("\n").split("\t")
        meta[f[0]] = f
    rows = []
    for name, src in cs:
        for rot in ("", "_r"):
            ref = samples(*load(os.path.join(out, "mesh", f"{name}{rot}_ref.bin")))
            for v, _, n in VARIANTS[1:]:
                tag = f"{name}{rot}_{v}"
                p, i = load(os.path.join(out, "mesh", tag + ".bin"))
                tol = 0.75 * 20.0 / n  # 隣の箱まで見るので、0.75〜1.5 マス以内
                s = samples(p, i)
                m = meta.get(tag, [tag, "0", "0", "", "0", "0"])
                rows.append((name, "回" if rot else "-", v, far_fraction(ref, s, tol, tol), far_fraction(s, ref, tol, tol),
                             float(m[5]), int(m[4]), len(i), int(m[2]), src))
    head = "名前\t回転\t版\t欠け%\t余計%\t裏返り%\t破片\t三角形\tms\t式"
    body = ["\t".join([r[0], r[1], r[2], f"{r[3]:.1f}", f"{r[4]:.1f}", f"{r[5]:.1f}", str(r[6]), str(r[7]), str(r[8]), r[9]]) for r in rows]
    open(os.path.join(out, "audit.tsv"), "w", encoding="utf8").write("\n".join([head] + body) + "\n")
    # まとめ: 版ごとの平均と、悪い行
    print(f"{'版':6s} {'回転':4s} {'欠け%':>7s} {'余計%':>7s} {'裏返り%':>7s} {'破片':>7s}   （{len(cs)} 式の平均。破片は合計）")
    for rot in ("-", "回"):
        for v, _, _ in VARIANTS[1:]:
            sel = [r for r in rows if r[1] == rot and r[2] == v]
            if sel:
                print(f"{v:6s} {rot:4s} {np.mean([r[3] for r in sel]):7.2f} {np.mean([r[4] for r in sel]):7.2f} {np.mean([r[5] for r in sel]):7.2f} {sum(r[6] for r in sel):7d}")
    print("\n悪い行（欠けか余計が 3% 以上、裏返りが 2% 以上、破片が 20 個以上）:")
    for r in sorted(rows, key=lambda r: -(r[3] + r[4] + r[5])):
        if r[3] >= 3 or r[4] >= 3 or r[5] >= 2 or r[6] >= 20:
            print(f"  {r[0]:10s} {r[1]:2s} {r[2]:4s} 欠け {r[3]:5.1f} 余計 {r[4]:5.1f} 裏返り {r[5]:5.1f} 破片 {r[6]:5d}  {r[9]}")


if __name__ == "__main__":
    main()
