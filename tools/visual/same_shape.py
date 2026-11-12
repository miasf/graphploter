#!/usr/bin/env python3
"""2 つの書き出し（dump_revs.sh の出力先の中の、コミットごとのフォルダ）を式ごとに比べて、形が同じかを調べる。

    python3 tools/visual/same_shape.py <前のフォルダ> <後のフォルダ>        # 違う式と、三角形の数が変わった式だけ
    python3 tools/visual/same_shape.py <前のフォルダ> <後のフォルダ> -v     # 全部の式

「同じ形」は、つぶれていない三角形の集合（頂点の位置を 1e-4 で丸めて比べる。順番と、面積 0 の三角形は見ない）が同じこと。
速くするだけ・整理するだけの変更で、形が変わっていないことを確かめるのに使う。違う式は、片方にしか無い三角形の数と面積を出す
（面積がほぼ同じなら、同じ面の切り方が変わっただけのことが多い。render.cjs で見比べる）。

陽関数で区間演算が使えない式は、計算時間で追加の探索を省くかどうかが変わるので、同じプログラムでも結果が変わることがある。
"""
import sys, os, struct, numpy as np
def load(path):
    d=open(path,'rb').read(); nv,ni=struct.unpack('<II',d[:8])
    p=np.frombuffer(d,'<f4',nv*3,8).reshape(-1,3).astype(float)
    i=np.frombuffer(d,'<u4',ni,8+nv*12).reshape(-1,3).astype(np.int64); return p,i
def tris(path):
    p,i=load(path)
    q=np.round(p*1e4).astype(np.int64)
    k=(q[:,0]+(1<<20))*(1<<42)+(q[:,1]+(1<<20))*(1<<21)+(q[:,2]+(1<<20))
    t=np.sort(k[i],axis=1)
    deg=(t[:,0]==t[:,1])|(t[:,1]==t[:,2])
    A,B,C=p[i[:,0]],p[i[:,1]],p[i[:,2]]
    area=np.linalg.norm(np.cross(B-A,C-A),axis=1)/2
    return t[~deg], len(i), deg.sum(), area, ~deg
a,b=sys.argv[1],sys.argv[2]
def meta(d):
    return {l.split('\t')[0]:l.rstrip('\n').split('\t') for l in open(os.path.join(d,'meta.tsv'),encoding='utf8')}
ma,mb=meta(a),meta(b)
same=diff=0; ta=tb=0; msa=msb=0
for name in ma:
    fa,fb=os.path.join(a,name+'.bin'),os.path.join(b,name+'.bin')
    if not (os.path.exists(fa) and os.path.exists(fb)): continue
    A,na,da,ara,ka=tris(fa); B,nb,db,arb,kb=tris(fb)
    ta+=na; tb+=nb; msa+=float(ma[name][2]); msb+=float(mb[name][2])
    va=A.view([('',A.dtype)]*3).ravel(); vb=B.view([('',B.dtype)]*3).ravel()
    onlya=~np.isin(va,vb); onlyb=~np.isin(vb,va)
    areaA=ara[ka][onlya].sum(); areaB=arb[kb][onlyb].sum()
    if onlya.sum()==0 and onlyb.sum()==0:
        same+=1
        if na!=nb or '-v' in sys.argv: print(f'  同じ形 {name:16s} 三角形 {na} → {nb}（つぶれ {da} → {db}） ms {ma[name][2]} → {mb[name][2]}')
    else:
        diff+=1
        print(f'! 違う   {name:16s} 三角形 {na} → {nb}（つぶれ {da} → {db}） 片方だけ {onlya.sum()} / {onlyb.sum()} 面積 {areaA:.4f} / {areaB:.4f}（全体 {ara.sum():.1f}） ms {ma[name][2]} → {mb[name][2]}  {ma[name][3]}')
print(f'同じ {same} 違う {diff}  三角形の合計 {ta} → {tb}  時間の合計 {msa:.0f} → {msb:.0f} ms')
