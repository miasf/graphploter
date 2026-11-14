// 座標系（計画 9 章）: 用意した座標系と、移行のスライダーで位置を混ぜる計算。
// 計算エンジンは、図形を座標の値の空間で作り、頂点ごとに座標系で移した先 T(q) を別の配列で返す。
// 画面側は s（0〜1）で (1 − s) q + s T(q) と混ぜる。s を動かしても式は計算し直さない。

import type { CoordPreset, CoordSystem } from './types'

/** 用意した座標系の、式（LaTeX。変数は x, y, z）と作る範囲 */
export const COORD_PRESETS: Record<
  CoordPreset,
  { name: string; map: CoordSystem['map']; ranges: CoordSystem['ranges']; names: [string, string, string]; note: string }
> = {
  cylinder: {
    name: '円柱座標',
    map: ['r\\cos\\theta', 'r\\sin\\theta', 'z'],
    ranges: [['0', '5'], ['0', '2\\pi'], ['-5', '5']],
    names: ['r', 'theta', 'z'],
    note: '半径 r（x と書いてもよい）、角度 θ（y）、高さ z。2D 表示では極座標',
  },
  sphere: {
    name: '球座標',
    map: ['r\\sin\\theta\\cos\\varphi', 'r\\sin\\theta\\sin\\varphi', 'r\\cos\\theta'],
    ranges: [['0', '5'], ['0', '\\pi'], ['0', '2\\pi']],
    names: ['r', 'theta', 'phi'],
    note: '半径 r（x と書いてもよい）、z 軸からの角 θ（y）、xy 平面での角 φ（z）',
  },
  ellipticCylinder: {
    name: '楕円柱座標',
    map: ['\\cosh\\mu\\cos\\nu', '\\sinh\\mu\\sin\\nu', 'z'],
    ranges: [['0', '2'], ['0', '2\\pi'], ['-5', '5']],
    names: ['mu', 'nu', 'z'],
    note: 'μ = 一定は楕円、ν = 一定は双曲線（焦点はどちらも (±1, 0)）。2D 表示では楕円座標',
  },
  parabolicCylinder: {
    name: '放物柱座標',
    map: ['\\frac{\\sigma^2-\\tau^2}{2}', '\\sigma\\tau', 'z'],
    ranges: [['-3', '3'], ['0', '3'], ['-5', '5']],
    names: ['sigma', 'tau', 'z'],
    note: 'σ = 一定・τ = 一定は、向かい合う放物線（焦点は原点）。2D 表示では放物線座標',
  },
  bipolar: {
    name: '双極座標',
    map: ['\\frac{\\sinh\\tau}{\\cosh\\tau-\\cos\\sigma}', '\\frac{\\sin\\sigma}{\\cosh\\tau-\\cos\\sigma}', 'z'],
    ranges: [['0', '2\\pi'], ['-3', '3'], ['-5', '5']],
    names: ['sigma', 'tau', 'z'],
    note: 'τ = 一定は 2 つの焦点 (±1, 0) のどちらかを囲む円（アポロニウスの円）、σ = 一定は 2 つの焦点を通る円。σ = τ = 0 は無限遠に飛ぶ',
  },
  logPolar: {
    name: '対数極座標',
    map: ['e^{\\rho}\\cos\\theta', 'e^{\\rho}\\sin\\theta', 'z'],
    ranges: [['-2', '1.5'], ['0', '2\\pi'], ['-5', '5']],
    names: ['rho', 'theta', 'z'],
    note: 'r = e^ρ。等間隔の ρ が、外へ行くほど広がる同心円になる（角度を保つ写像。らせん ρ = θ/5 は等角らせん）',
  },
  parabolic: {
    name: '放物線座標',
    map: ['\\sigma\\tau\\cos\\varphi', '\\sigma\\tau\\sin\\varphi', '\\frac{\\tau^2-\\sigma^2}{2}'],
    ranges: [['0', '2.5'], ['0', '2.5'], ['0', '2\\pi']],
    names: ['sigma', 'tau', 'phi'],
    note: 'σ = 一定は下向き、τ = 一定は上向きの放物面（焦点は原点）。φ は z 軸の周りの角',
  },
  prolate: {
    name: '長球座標',
    map: ['\\sinh\\mu\\sin\\nu\\cos\\varphi', '\\sinh\\mu\\sin\\nu\\sin\\varphi', '\\cosh\\mu\\cos\\nu'],
    ranges: [['0', '1.5'], ['0', '\\pi'], ['0', '2\\pi']],
    names: ['mu', 'nu', 'phi'],
    note: 'μ = 一定はラグビーボール形の回転楕円体、ν = 一定は 2 枚の双曲面（焦点は (0, 0, ±1)）',
  },
  oblate: {
    name: '扁球座標',
    map: ['\\cosh\\mu\\cos\\nu\\cos\\varphi', '\\cosh\\mu\\cos\\nu\\sin\\varphi', '\\sinh\\mu\\sin\\nu'],
    ranges: [['0', '1.5'], ['-\\frac{\\pi}{2}', '\\frac{\\pi}{2}'], ['0', '2\\pi']],
    names: ['mu', 'nu', 'phi'],
    note: 'μ = 一定はみかん形につぶれた回転楕円体、ν = 一定は 1 枚の双曲面（焦点は半径 1 の円）',
  },
  toroidal: {
    name: 'トーラス座標',
    map: ['\\frac{\\sinh\\tau\\cos\\varphi}{\\cosh\\tau-\\cos\\sigma}', '\\frac{\\sinh\\tau\\sin\\varphi}{\\cosh\\tau-\\cos\\sigma}', '\\frac{\\sin\\sigma}{\\cosh\\tau-\\cos\\sigma}'],
    ranges: [['-\\pi', '\\pi'], ['0.3', '3'], ['0', '2\\pi']],
    names: ['sigma', 'tau', 'phi'],
    note: 'τ = 一定は入れ子になったトーラス（τ が大きいほど細い。中心の円は半径 1）、σ = 一定は球の一部',
  },
  twist: {
    name: 'ねじれ座標',
    map: ['x\\cos\\frac{z}{2}-y\\sin\\frac{z}{2}', 'x\\sin\\frac{z}{2}+y\\cos\\frac{z}{2}', 'z'],
    ranges: [['-3', '3'], ['-3', '3'], ['-5', '5']],
    names: ['x', 'y', 'z'],
    note: '高さ z に比例して xy 平面が回る（z が 1 増えると 1/2 ラジアン）。平面 x = 1 がねじれた面に、まっすぐな線がらせんになる',
  },
  custom: {
    name: '自作の座標系',
    map: ['x', 'y', 'z'],
    ranges: [['-5', '5'], ['-5', '5'], ['-5', '5']],
    names: ['x', 'y', 'z'],
    note: 'x, y, z の式を 3 つ書きます。パラメータも使えます',
  },
}

/**
 * 座標の名前を、計算エンジンの名前にする（θ・\theta → theta、φ・\varphi → phi、ρ → rho、σ・τ・μ・ν も同じ。前後の空白は取る）。
 * 空なら null（その座標は x, y, z のまま）
 */
export function engineName(s: string): string | null {
  const t = s.trim().replace(/^\\/, '')
  if (t === '') return null
  if (['θ', 'ϑ', 'theta', 'vartheta'].includes(t)) return 'theta'
  if (['φ', 'ϕ', 'phi', 'varphi'].includes(t)) return 'phi'
  if (['ρ', 'rho', 'varrho'].includes(t)) return 'rho'
  if (['σ', 'sigma', 'varsigma'].includes(t)) return 'sigma'
  if (['τ', 'tau'].includes(t)) return 'tau'
  if (['μ', 'mu'].includes(t)) return 'mu'
  if (['ν', 'nu'].includes(t)) return 'nu'
  return t
}

/** 1 つの名前として読むギリシャ文字（計算エンジンの名前 → 画面に出す文字） */
export const GREEK_NAMES: Record<string, string> = { theta: 'θ', phi: 'φ', rho: 'ρ', sigma: 'σ', tau: 'τ', mu: 'μ', nu: 'ν' }

/** 計算エンジンの名前を、画面に出す名前にする（theta → θ） */
export function prettyCoordName(s: string): string {
  return GREEK_NAMES[s] ?? s
}

/**
 * 位置を混ぜる: (1 − s) q + s T(q)。s = 0 なら q そのもの（新しい配列にはしない）、s = 1 なら target そのもの。
 * NaN（線の切れ目）はそのまま残る（どちらかが NaN なら NaN）
 */
export function mixPositions(q: Float32Array, target: Float32Array, s: number): Float32Array {
  if (s <= 0 || target.length !== q.length) return q
  if (s >= 1) return target
  const out = new Float32Array(q.length)
  for (let k = 0; k < q.length; k++) out[k] = q[k] + s * (target[k] - q[k])
  return out
}

/**
 * 三角形から頂点の法線を作る（三角形の面積の重みで足して、長さ 1 にする）。
 * 2 つの法線を混ぜると途中の s で正しくないので、混ぜた後の位置から計算し直す。面積 0 の三角形は自然に効かない
 */
export function vertexNormals(pos: Float32Array, idx: Uint32Array): Float32Array {
  const n = new Float32Array(pos.length)
  for (let t = 0; t + 2 < idx.length; t += 3) {
    const a = 3 * idx[t]
    const b = 3 * idx[t + 1]
    const c = 3 * idx[t + 2]
    const ux = pos[b] - pos[a]
    const uy = pos[b + 1] - pos[a + 1]
    const uz = pos[b + 2] - pos[a + 2]
    const vx = pos[c] - pos[a]
    const vy = pos[c + 1] - pos[a + 1]
    const vz = pos[c + 2] - pos[a + 2]
    // 外積（長さが三角形の面積の 2 倍なので、そのまま足すと面積の重みになる）
    const nx = uy * vz - uz * vy
    const ny = uz * vx - ux * vz
    const nz = ux * vy - uy * vx
    if (!Number.isFinite(nx + ny + nz)) continue
    for (const v of [a, b, c]) {
      n[v] += nx
      n[v + 1] += ny
      n[v + 2] += nz
    }
  }
  for (let v = 0; v < n.length; v += 3) {
    const len = Math.hypot(n[v], n[v + 1], n[v + 2])
    if (len > 0) {
      n[v] /= len
      n[v + 1] /= len
      n[v + 2] /= len
    }
  }
  return n
}

/**
 * 移した先の位置が同じ（範囲の大きさの 10⁻⁵ 以内）頂点の組。周期のある座標の継ぎ目（φ = 0 と 2π）では、
 * 同じ点の頂点が別々にあり、法線が合わずに筋が見える。s = 1 のときは、この組ごとに法線を足し合わせてそろえる。
 * 返すのは、組ごとに頂点の番号を並べ、組の終わりに −1 を置いた列
 */
export function weldGroups(target: Float32Array, idx: Uint32Array): Int32Array {
  const used = new Uint8Array(target.length / 3)
  for (const v of idx) used[v] = 1
  let lo = Infinity
  let hi = -Infinity
  for (let k = 0; k < target.length; k++) {
    if (!used[Math.floor(k / 3)] || !Number.isFinite(target[k])) continue
    lo = Math.min(lo, target[k])
    hi = Math.max(hi, target[k])
  }
  if (!(hi > lo)) return new Int32Array(0)
  const cell = (hi - lo) * 1e-5
  const buckets = new Map<string, number[]>()
  for (let v = 0; v < used.length; v++) {
    if (!used[v]) continue
    const key = `${Math.round(target[3 * v] / cell)},${Math.round(target[3 * v + 1] / cell)},${Math.round(target[3 * v + 2] / cell)}`
    const b = buckets.get(key)
    if (b) b.push(v)
    else buckets.set(key, [v])
  }
  const out: number[] = []
  for (const b of buckets.values()) {
    if (b.length < 2) continue
    out.push(...b, -1)
  }
  return Int32Array.from(out)
}

/** [`weldGroups`] の組ごとに、法線を足し合わせて長さ 1 にする（normals を書き換える） */
export function weldNormals(normals: Float32Array, groups: Int32Array) {
  let start = 0
  for (let k = 0; k < groups.length; k++) {
    if (groups[k] >= 0) continue
    let x = 0
    let y = 0
    let z = 0
    for (let j = start; j < k; j++) {
      const v = 3 * groups[j]
      x += normals[v]
      y += normals[v + 1]
      z += normals[v + 2]
    }
    const len = Math.hypot(x, y, z)
    if (len > 0) {
      for (let j = start; j < k; j++) {
        const v = 3 * groups[j]
        normals[v] = x / len
        normals[v + 1] = y / len
        normals[v + 2] = z / len
      }
    }
    start = k + 1
  }
}
