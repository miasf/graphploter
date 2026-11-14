// 曲面用の材質。Three.js の標準の材質に「格子線」を描く処理を足したもの。
//
// 格子線はメッシュの三角形とは関係なく、画素ごとに
// 「格子用の座標 gridCoord の各成分が、間隔のちょうど整数倍に近いか」を調べて線の色を混ぜている。
// そのため、どんな形の曲面にも同じように線が引ける。
// - 陽関数・陰関数: gridCoord = (x, y, z)。z 方向の線は z = f(x, y) では等高線になる
// - 媒介変数表示の曲面: gridCoord = (u − u0, v − v0, z)。曲面に沿った u, v の格子になる
// - 媒介変数表示の立体: gridCoord = (u − u0, v − v0, t − t0)。その面で固定した媒介変数の成分は −1e30（線を引かない）

import * as THREE from 'three'

/** シェーダーに渡す値。表面用・裏面用の2つの材質で共有する。 */
export interface GridUniforms {
  /** x, y, z 方向の線の間隔（0 なら描かない） */
  uGridSpacing: { value: THREE.Vector3 }
  uGridColor: { value: THREE.Color }
  /** 線の濃さ（0〜1） */
  uGridStrength: { value: number }
  /** 線の太さ（ふつうのときの画素密度での画素） */
  uGridWidth: { value: number }
  /** 今の画素密度 ÷ ふつうのときの画素密度。動かしている間に画素密度を下げても、線の太さが変わって見えないようにする
   *  （全部の曲面で同じものを共有する） */
  uPixelScale: { value: number }
  /** 面の不透明度（0 なら格子線だけが見える） */
  uFillOpacity: { value: number }
  /** 1 なら複素数の偏角で色を付ける（頂点の complexValue を使う） */
  uComplex: { value: number }
  /** 1 なら 2D の塗り分け: 値 vGridPos.z が uHeatRange の範囲のどこにあるかで、色の濃さを変える */
  uHeat: { value: number }
  uHeatRange: { value: THREE.Vector2 }
  /** 1 なら表示していない座標の値で色を付ける: 値 vComplex.x が uValueRange のどこにあるかを、色の対応表 uColormap で色にする */
  uValue: { value: number }
  uValueRange: { value: THREE.Vector2 }
  /** 色の対応表（0: viridis, 1: turbo, 2: 式の色の明暗） */
  uColormap: { value: number }
  /** 1 なら裏面を暗くしない（2D 表示では面の表裏に意味が無い） */
  uNoBackShade: { value: number }
  /**
   * 1 なら格子線を、格子用の座標に 3D オブジェクトの行列を掛けた位置（表示中の座標）で引く。
   * 回す前の形で作った陰関数は、オブジェクトを回して描くので、そのままでは格子線も一緒に回ってしまう
   */
  uGridWorld: { value: number }
}

export function createGridUniforms(pixelScale: { value: number } = { value: 1 }): GridUniforms {
  return {
    uPixelScale: pixelScale,
    uGridSpacing: { value: new THREE.Vector3() },
    uGridColor: { value: new THREE.Color() },
    uGridStrength: { value: 0.75 },
    uGridWidth: { value: 1 },
    uFillOpacity: { value: 1 },
    uComplex: { value: 0 },
    uHeat: { value: 0 },
    uHeatRange: { value: new THREE.Vector2(-1, 1) },
    uValue: { value: 0 },
    uValueRange: { value: new THREE.Vector2(0, 1) },
    uColormap: { value: 0 },
    uNoBackShade: { value: 0 },
    uGridWorld: { value: 0 },
  }
}

/**
 * 色の対応表（GLSL）。t は 0〜1、base は式の色。返す色は線形の色空間（three.js の diffuseColor と同じ）。
 * viridis・turbo は多項式で近似したもの（matplotlib の viridis の近似と、Google の Turbo の近似）
 */
export const COLORMAP_GLSL = /* glsl */ `
vec3 cmapViridis(float t) {
  const vec3 c0 = vec3(0.2777273272234177, 0.005407344544966578, 0.3340998053353061);
  const vec3 c1 = vec3(0.1050930431085774, 1.404613529898575, 1.384590162594685);
  const vec3 c2 = vec3(-0.3308618287255563, 0.214847559468213, 0.09509516302823659);
  const vec3 c3 = vec3(-4.634230498983486, -5.799100973351585, -19.33244095627987);
  const vec3 c4 = vec3(6.228269936347081, 14.17993336680509, 56.69055260068105);
  const vec3 c5 = vec3(4.776384997670288, -13.74514537774601, -65.35303263337234);
  const vec3 c6 = vec3(-5.435455855934631, 4.645852612178535, 26.3124352495832);
  return c0 + t * (c1 + t * (c2 + t * (c3 + t * (c4 + t * (c5 + t * c6)))));
}
vec3 cmapTurbo(float t) {
  const vec4 kr = vec4(0.13572138, 4.61539260, -42.66032258, 132.13108234);
  const vec4 kg = vec4(0.09140261, 2.19418839, 4.84296658, -14.18503333);
  const vec4 kb = vec4(0.10667330, 12.64194608, -60.58204836, 110.36276771);
  const vec2 kr2 = vec2(-152.94239396, 59.28637943);
  const vec2 kg2 = vec2(4.27729857, 2.82956604);
  const vec2 kb2 = vec2(-89.90310912, 27.34824973);
  vec4 v4 = vec4(1.0, t, t * t, t * t * t);
  vec2 v2 = v4.zw * v4.z;
  return vec3(dot(v4, kr) + dot(v2, kr2), dot(v4, kg) + dot(v2, kg2), dot(v4, kb) + dot(v2, kb2));
}
// 線形 sRGB ⇔ OKLab（Björn Ottosson）
vec3 linToOklab(vec3 c) {
  vec3 l = vec3(
    0.4122214708 * c.r + 0.5363325363 * c.g + 0.0514459929 * c.b,
    0.2119034982 * c.r + 0.6806995451 * c.g + 0.1073969566 * c.b,
    0.0883024619 * c.r + 0.2817188376 * c.g + 0.6299787005 * c.b);
  l = pow(max(l, 0.0), vec3(1.0 / 3.0));
  return vec3(
    0.2104542553 * l.x + 0.7936177850 * l.y - 0.0040720468 * l.z,
    1.9779984951 * l.x - 2.4285922050 * l.y + 0.4505937099 * l.z,
    0.0259040371 * l.x + 0.7827717662 * l.y - 0.8086757660 * l.z);
}
vec3 oklabToLin(vec3 c) {
  vec3 l = vec3(
    c.x + 0.3963377774 * c.y + 0.2158037573 * c.z,
    c.x - 0.1055613458 * c.y - 0.0638541728 * c.z,
    c.x - 0.0894841775 * c.y - 1.2914855480 * c.z);
  l = l * l * l;
  return vec3(
    4.0767416621 * l.x - 3.3077115913 * l.y + 0.2309699292 * l.z,
    -1.2684380046 * l.x + 2.6097574011 * l.y - 0.3413193965 * l.z,
    -0.0041960863 * l.x - 0.7034186147 * l.y + 1.7076147010 * l.z);
}
// 式の色の明暗: OKLab の明るさを 0.3〜0.94 で動かし、色相は式の色のまま。鮮やかさは式の色の明るさの所で最大（式の色そのもの）にし、
// 暗い側・明るい側へ向かって少し落とす（線形の色を混ぜると途中が灰色に寄ってくすむため）。表せない色は鮮やかさを下げて収める
vec3 cmapShade(float t, vec3 base) {
  vec3 lab = linToOklab(max(base, 0.0));
  float L0 = clamp(lab.x, 0.4, 0.85);
  float L = mix(0.3, 0.94, t);
  float k = L < L0 ? mix(0.7, 1.0, (L - 0.3) / (L0 - 0.3)) : mix(1.0, 0.35, (L - L0) / (0.94 - L0));
  vec3 c = oklabToLin(vec3(L, lab.yz * k));
  if (any(lessThan(c, vec3(-1e-4))) || any(greaterThan(c, vec3(1.0001)))) {
    float lo = 0.0, hi = k;
    for (int i = 0; i < 10; i++) {
      float m = 0.5 * (lo + hi);
      vec3 q = oklabToLin(vec3(L, lab.yz * m));
      if (any(lessThan(q, vec3(-1e-4))) || any(greaterThan(q, vec3(1.0001)))) hi = m; else lo = m;
    }
    c = oklabToLin(vec3(L, lab.yz * lo));
  }
  return clamp(c, 0.0, 1.0);
}
vec3 colormap(float t, float which, vec3 base) {
  t = clamp(t, 0.0, 1.0);
  if (which > 1.5) return cmapShade(t, base);
  vec3 srgb = clamp(which > 0.5 ? cmapTurbo(t) : cmapViridis(t), 0.0, 1.0);
  return pow(srgb, vec3(2.2));
}
`

/** COLORMAP_GLSL の colormap と同じ色（JS 版。曲線の頂点の色に使う）。返す色は線形の色空間 */
export function colormapColor(t: number, which: 'viridis' | 'turbo' | 'shade', base: THREE.Color, out: THREE.Color): THREE.Color {
  t = Math.min(1, Math.max(0, t))
  if (which === 'shade') return shadeColor(t, base, out)
  let r: number, g: number, b: number
  if (which === 'turbo') {
    const v4 = [1, t, t * t, t * t * t]
    const v2 = [v4[2] * v4[2], v4[3] * v4[2]]
    const dot4 = (k: number[]) => k[0] * v4[0] + k[1] * v4[1] + k[2] * v4[2] + k[3] * v4[3]
    r = dot4([0.13572138, 4.6153926, -42.66032258, 132.13108234]) + -152.94239396 * v2[0] + 59.28637943 * v2[1]
    g = dot4([0.09140261, 2.19418839, 4.84296658, -14.18503333]) + 4.27729857 * v2[0] + 2.82956604 * v2[1]
    b = dot4([0.1066733, 12.64194608, -60.58204836, 110.36276771]) + -89.90310912 * v2[0] + 27.34824973 * v2[1]
  } else {
    const c = [
      [0.2777273272234177, 0.005407344544966578, 0.3340998053353061],
      [0.1050930431085774, 1.404613529898575, 1.384590162594685],
      [-0.3308618287255563, 0.214847559468213, 0.09509516302823659],
      [-4.634230498983486, -5.799100973351585, -19.33244095627987],
      [6.228269936347081, 14.17993336680509, 56.69055260068105],
      [4.776384997670288, -13.74514537774601, -65.35303263337234],
      [-5.435455855934631, 4.645852612178535, 26.3124352495832],
    ]
    const ch = (k: number) => c.reduceRight((acc, row) => row[k] + t * acc, 0)
    ;[r, g, b] = [ch(0), ch(1), ch(2)]
  }
  const lin = (v: number) => Math.pow(Math.min(1, Math.max(0, v)), 2.2)
  return out.setRGB(lin(r), lin(g), lin(b))
}

const FRAGMENT_HEADER = /* glsl */ `
uniform float uValue;
uniform vec2 uValueRange;
uniform float uColormap;
${COLORMAP_GLSL}
varying vec3 vGridPos;
varying vec3 vComplex;
uniform float uComplex;
uniform float uHeat;
uniform vec2 uHeatRange;
uniform float uNoBackShade;
uniform vec3 uGridSpacing;
uniform vec3 uGridColor;
uniform float uGridStrength;
uniform float uGridWidth;
uniform float uPixelScale;
uniform float uFillOpacity;

// 座標 coord が spacing の整数倍の近くなら 1、離れていれば 0（太さ uGridWidth 画素）
float gridLine(float coord, float spacing) {
  if (spacing <= 0.0 || coord < -1e29) return 0.0;
  float c = coord / spacing;
  float w = fwidth(c);                      // 1画素で c がどれだけ変わるか
  // 座標が変わらない面（例: 切り口のふた y = 0 の上の y）には線を引かない（0 / 0 で面全体が線になるのを防ぐ）
  if (w <= 0.0) return 0.0;
  float d = abs(fract(c - 0.5) - 0.5) / w;  // 一番近い線までの距離（画素）
  // 今の画素密度での太さ（画素密度を下げている間は細くして、画面の上で同じ太さに見せる。1 画素より細い分は薄くする）
  float width = uGridWidth * uPixelScale;
  float hw = 0.5 * width;                    // 太さの半分
  float line = (1.0 - smoothstep(hw - 0.5, hw + 0.5, d)) * min(width, 1.0);
  // 線が細かすぎて1画素に何本も入るときは、1本ずつ描くとちらつく（モアレ）ので、
  // 「線が画素を覆う割合」（= 太さ × 1画素あたりの本数）の濃さで一様に塗る
  float coverage = min(width * w, 1.0);
  return mix(line, coverage, smoothstep(0.2, 0.5, w));
}
`

const FRAGMENT_APPLY = /* glsl */ `
{
  if (uValue > 0.5) {
    // 表示していない座標の値で色を付ける（範囲が 1 点なら真ん中の色）
    float span = uValueRange.y - uValueRange.x;
    float t = span > 0.0 ? (vComplex.x - uValueRange.x) / span : 0.5;
    diffuseColor.rgb = colormap(t, uColormap, diffuseColor.rgb);
  } else if (uComplex > 0.5) {
    // 複素数の色分け: 色相 = 偏角（0 で赤、π/2 で黄緑、π で水色、−π/2 で紫）、
    // |w| が 2 倍になるごとに明るさが少しずつ変わる縞（大きさの等高線の代わり）
    float hue = atan(vComplex.y, vComplex.x) / 6.28318530718;
    vec3 rgb = clamp(abs(mod(hue * 6.0 + vec3(0.0, 4.0, 2.0), 6.0) - 3.0) - 1.0, 0.0, 1.0);
    rgb = mix(vec3(1.0), rgb, 0.85);
    float band = fract(vComplex.z);
    diffuseColor.rgb = rgb * (0.72 + 0.28 * band);
  } else if (uHeat > 0.5) {
    // 2D の塗り分け: 値が小さいほど暗く、大きいほど明るく（範囲の外は端の色）
    float h = clamp((vGridPos.z - uHeatRange.x) / (uHeatRange.y - uHeatRange.x), 0.0, 1.0);
    diffuseColor.rgb = mix(diffuseColor.rgb * 0.25, mix(diffuseColor.rgb, vec3(1.0), 0.75), h);
  }
  // 裏面は暗くして、どちら側を見ているか分かるようにする（表と裏を1回で描けるので、メッシュが1つで済む）
  if (!gl_FrontFacing && uNoBackShade < 0.5) diffuseColor.rgb *= 0.55;
  float g = max(max(gridLine(vGridPos.x, uGridSpacing.x),
                    gridLine(vGridPos.y, uGridSpacing.y)),
                gridLine(vGridPos.z, uGridSpacing.z));
  diffuseColor.rgb = mix(diffuseColor.rgb, uGridColor, g * uGridStrength);
  // 面は uFillOpacity の濃さ、線の上は不透明にする。ほぼ透明な所は描かない
  diffuseColor.a = max(uFillOpacity, g);
  if (diffuseColor.a < 0.01) discard;
}
`

/** 曲面の材質（表と裏の両面を描く。裏面はシェーダーで暗くする） */
export function createSurfaceMaterial(grid: GridUniforms, clippingPlanes: THREE.Plane[]): THREE.MeshStandardMaterial {
  const m = new THREE.MeshStandardMaterial({
    side: THREE.DoubleSide,
    roughness: 0.55,
    metalness: 0.05,
    clippingPlanes,
  })
  // Three.js が作るシェーダーのコードに、格子線の処理を差し込む
  m.onBeforeCompile = (shader) => {
    Object.assign(shader.uniforms, grid)
    shader.vertexShader = shader.vertexShader
      .replace(
        '#include <common>',
        '#include <common>\nattribute vec3 gridCoord;\nattribute vec3 complexValue;\nvarying vec3 vGridPos;\nvarying vec3 vComplex;\nuniform float uGridWorld;',
      )
      .replace(
        '#include <begin_vertex>',
        '#include <begin_vertex>\nvGridPos = uGridWorld > 0.5 ? (modelMatrix * vec4(gridCoord, 1.0)).xyz : gridCoord;\nvComplex = complexValue;',
      )
    shader.fragmentShader = shader.fragmentShader
      .replace('#include <common>', '#include <common>\n' + FRAGMENT_HEADER)
      .replace('#include <color_fragment>', '#include <color_fragment>\n' + FRAGMENT_APPLY)
  }
  m.customProgramCacheKey = () => 'surface-with-grid'
  return m
}

function linToOklab(r: number, g: number, b: number): [number, number, number] {
  const l = Math.cbrt(Math.max(0, 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b))
  const m = Math.cbrt(Math.max(0, 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b))
  const s = Math.cbrt(Math.max(0, 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b))
  return [
    0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s,
    1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s,
    0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s,
  ]
}
function oklabToLin(L: number, a: number, b: number): [number, number, number] {
  const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3
  const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3
  const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s,
  ]
}
const outOfGamut = (c: number[]) => c.some((v) => v < -1e-4 || v > 1.0001)

/** COLORMAP_GLSL の cmapShade と同じ（式の色の明暗。OKLab で明るさだけを動かし、色相と鮮やかさを保つ） */
function shadeColor(t: number, base: THREE.Color, out: THREE.Color): THREE.Color {
  const [L0raw, a, b] = linToOklab(base.r, base.g, base.b)
  const L0 = Math.min(0.85, Math.max(0.4, L0raw))
  const L = 0.3 + (0.94 - 0.3) * t
  const k = L < L0 ? 0.7 + 0.3 * ((L - 0.3) / (L0 - 0.3)) : 1 + (0.35 - 1) * ((L - L0) / (0.94 - L0))
  let c = oklabToLin(L, a * k, b * k)
  if (outOfGamut(c)) {
    let lo = 0
    let hi = k
    for (let i = 0; i < 10; i++) {
      const m = (lo + hi) / 2
      if (outOfGamut(oklabToLin(L, a * m, b * m))) hi = m
      else lo = m
    }
    c = oklabToLin(L, a * lo, b * lo)
  }
  const cl = (v: number) => Math.min(1, Math.max(0, v))
  return out.setRGB(cl(c[0]), cl(c[1]), cl(c[2]))
}
