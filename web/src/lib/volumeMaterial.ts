// 伝達関数（ボリュームレンダリング）の材質。表示していない座標の値 w = f(x, y, z) を、表示範囲の箱の中の格子の値として 3 次元のテクスチャに入れ、
// 画素ごとに視線を箱の中で少しずつ進めて、値に割り当てた色と不透明度を手前から重ねる。
// 箱の表の面（FrontSide）を描くので、視点が箱の中に入ると描かれない。光は当てない（自分で光る霧のように見せる）

import * as THREE from 'three'
import { COLORMAP_GLSL } from './surfaceMaterial'
import type { HiddenStyle } from './types'

export interface VolumeUniforms {
  uTex: { value: THREE.Data3DTexture }
  uBoxMin: { value: THREE.Vector3 }
  uBoxMax: { value: THREE.Vector3 }
  /** 光線を進める箱（格子の箱 uBoxMin〜uBoxMax の中。不等式の領域の霧では、領域を囲む箱に縮めて、何も無い所を歩かない） */
  uRayMin: { value: THREE.Vector3 }
  uRayMax: { value: THREE.Vector3 }
  /** 色を付ける値の範囲（外の値は描かない） */
  uRange: { value: THREE.Vector2 }
  uColormap: { value: number }
  /** 不透明度の形（0: 一様, 1: 値が大きいほど濃い, 2: 小さいほど濃い, 3: 何枚かの値の所だけ濃い, 4: 値 0 の近くだけ濃い） */
  uShape: { value: number }
  /** 濃さ（0〜1） */
  uDensity: { value: number }
  /** 濃い所の数（形が 3 のとき） */
  uBands: { value: number }
  /** 式の色（線形）。色の対応表が「式の色の明暗」のとき使う */
  uBase: { value: THREE.Color }
  /** 切り口の平面 (n, c)（n · p + c ≥ 0 の側を残す）。uCutOn が 0 なら切らない */
  uCut: { value: THREE.Vector4 }
  uCutOn: { value: number }
  /** 全体の不透明度（式の「面の濃さ」） */
  uOpacity: { value: number }
  /** 視線を進める歩数（格子の 1 辺の点の数の 1.5 倍。64〜192。不等式の領域の霧は 2.5 倍、256 まで） */
  uSteps: { value: number }
  /** 重ね方（0: 手前から重ね塗り, 1: 最大値, 2: 最小値, 3: 平均値） */
  uBlend: { value: number }
  /** 1 なら範囲の外の値も端の色で描く（陰関数の F の値。範囲は |F| の分布から決めた目安なので） */
  uClamp: { value: number }
  /** 格子の 1 辺の点の数（勾配を求める差分の幅に使う） */
  uN: { value: number }
  /** ぼやけた面（形 4）の幅。格子の 1 マスの何倍か */
  uWidth: { value: number }
  /** 1 なら、値の勾配を面の法線とみなして光を当てる（重ね塗りのとき） */
  uLit: { value: number }
  /**
   * 1 なら値が複素数（テクスチャの r, g が実部・虚部）。色は偏角（陽関数の複素数の色分けと同じ色）、不透明度の形は |f| で決める
   * （値の範囲は [0, q]。何枚かの値の所だけ濃い形は、|f| が 2 のべき乗の所）
   */
  uComplex: { value: number }
  /**
   * 1 なら不等式の領域の霧（テクスチャの値は境目までの距離の見積もり G / |∇G|。負なら中身）。中身を式の色で薄く塗り、
   * 境目（格子の 1 マスくらいの幅）を濃くして、勾配で陰影を付ける。重ね方・色の対応表・不透明度の形は使わない
   */
  uRegion: { value: number }
}

const VERTEX = /* glsl */ `
varying vec3 vWorld;
void main() {
  vec4 w = modelMatrix * vec4(position, 1.0);
  vWorld = w.xyz;
  gl_Position = projectionMatrix * viewMatrix * w;
}
`

const FRAGMENT = /* glsl */ `
precision highp float;
precision highp sampler3D;
uniform sampler3D uTex;
uniform vec3 uBoxMin;
uniform vec3 uBoxMax;
uniform vec3 uRayMin;
uniform vec3 uRayMax;
uniform vec2 uRange;
uniform float uColormap;
uniform float uShape;
uniform float uDensity;
uniform float uBands;
uniform vec3 uBase;
uniform vec4 uCut;
uniform float uCutOn;
uniform float uOpacity;
uniform float uSteps;
uniform float uBlend;
uniform float uClamp;
uniform float uN;
uniform float uWidth;
uniform float uLit;
uniform float uComplex;
uniform float uRegion;
varying vec3 vWorld;
${COLORMAP_GLSL}

// 値の位置 t（0〜1）での、単位の長さあたりの濃さの形。値 0 の近く（形 4）は、ここでは値の位置での近さ（重ね方が最大値などのとき）
float shape(float t) {
  if (uShape < 0.5) return 1.0;
  // 2 乗にして、端の値と中くらいの値の差をはっきりさせる（1 乗だと中くらいの値も濃く、霧のようになる）
  if (uShape < 1.5) return 2.0 * t * t;
  if (uShape < 2.5) return 2.0 * (1.0 - t) * (1.0 - t);
  if (uShape < 3.5) {
    float f = fract(t * uBands) - 0.5;
    return 4.0 * exp(-f * f / (2.0 * 0.07 * 0.07));
  }
  float t0 = uRange.y > uRange.x ? -uRange.x / (uRange.y - uRange.x) : 0.5;
  float d = (t - t0) / (0.02 * uWidth);
  return 4.0 * exp(-0.5 * d * d);
}

// 濃さの形。値が複素数で「何枚かの値の所だけ濃い」なら、|f| = 2^k の所（色の明るさの縞と同じ所）を濃くする
float shapeAt(float s, float v) {
  if (uComplex > 0.5 && uShape > 2.5 && uShape < 3.5) {
    float f = fract(log2(max(v, 1e-30)) + 0.5) - 0.5;
    return 4.0 * exp(-f * f / (2.0 * 0.07 * 0.07));
  }
  return shape(s);
}

// 複素数の色: 色相 = 偏角（0 で赤、π/2 で黄緑、π で水色、−π/2 で紫）、|z| が 2 倍になるごとに明るさが少しずつ変わる縞
// （surfaceMaterial の陽関数の複素数の色分けと同じ）
vec3 argColor(vec2 z) {
  float hue = atan(z.y, z.x) / 6.28318530718;
  vec3 rgb = clamp(abs(mod(hue * 6.0 + vec3(0.0, 4.0, 2.0), 6.0) - 3.0) - 1.0, 0.0, 1.0);
  rgb = mix(vec3(1.0), rgb, 0.85);
  return rgb * (0.72 + 0.28 * fract(log2(max(length(z), 1e-30))));
}

// 点 q での値（複素数なら |f|。実部・虚部を補間してから大きさにする。偏角や |f| を補間すると、偏角が ±π をまたぐ所や 0 の近くで崩れる）
float valueAt(vec3 q) {
  vec4 t = texture(uTex, q);
  return uComplex > 0.5 ? length(t.rg) : t.r;
}

// 値の勾配（表示範囲の座標あたり）。格子の 1 マスの幅の中心差分
vec3 gradAt(vec3 q, vec3 size) {
  float h = 1.0 / uN;
  return vec3(
    valueAt(q + vec3(h, 0.0, 0.0)) - valueAt(q - vec3(h, 0.0, 0.0)),
    valueAt(q + vec3(0.0, h, 0.0)) - valueAt(q - vec3(0.0, h, 0.0)),
    valueAt(q + vec3(0.0, 0.0, h)) - valueAt(q - vec3(0.0, 0.0, h))) / (2.0 * h * size);
}

// 値 0 の近くだけ濃い形（重ね塗りのとき）: 0 の面からの距離を |v| / |∇v| で見積もり、格子の uWidth マスの幅のぼやけた面にする
// （値の大きさで幅を決めると、勾配の大きい所は薄く、小さい所は厚くなる）。勾配を gGrad に入れる（光を当てるときに使い回す）
vec3 gGrad;
float shell(float v, vec3 q, vec3 size) {
  gGrad = gradAt(q, size);
  float gl = length(gGrad);
  if (!(gl > 0.0)) return 0.0;
  float w = uWidth * max(max(size.x, size.y), size.z) / uN;
  float d = v / gl / w;
  return 4.0 * exp(-0.5 * d * d);
}

// 勾配 g を法線とみなした光の当たり方を、色 c に掛ける。光は視点の少し右上から（真正面からだと、面の傾きの差が出にくい）。
// 裏表は区別しない（法線を視点の側に向ける）。勾配が値の範囲に比べて小さい所（ほぼ平らな所）は法線があてにならないので、
// 光を弱めて元の色に近づける（全部に当てると、なめらかな霧に細かいムラが出る）。always が 1 なら勾配の大きさによらず当てる
vec3 lit(vec3 c, vec3 g, vec3 dir, vec3 size, float span, float always) {
  float gl = length(g);
  if (!(gl > 0.0)) return c;
  vec3 n = g / gl;
  if (dot(n, dir) > 0.0) n = -n;
  vec3 right = vec3(viewMatrix[0][0], viewMatrix[1][0], viewMatrix[2][0]);
  vec3 up = vec3(viewMatrix[0][1], viewMatrix[1][1], viewMatrix[2][1]);
  vec3 L = normalize(-dir + 0.45 * up + 0.3 * right);
  float diff = max(dot(n, L), 0.0);
  float spec = pow(max(dot(n, normalize(L - dir)), 0.0), 60.0);
  // 暗い側の下限を高めにし、光に向いた所は元の色より少し明るくする（下限を低くすると、重なった面の多くが横向きなので全体が暗くくすむ）
  vec3 shaded = min(c * (0.55 + 0.6 * diff), vec3(1.0)) + 0.3 * spec;
  // 格子の 1 マスで値の位置（0〜1）がどれだけ変わるか
  float perCell = gl * max(max(size.x, size.y), size.z) / uN / max(span, 1e-30);
  float m = always > 0.5 ? 1.0 : smoothstep(0.002, 0.02, perCell);
  return mix(c, shaded, m);
}

void main() {
  vec3 dir = normalize(vWorld - cameraPosition);
  vec3 inv = 1.0 / dir;
  vec3 t0 = (uRayMin - cameraPosition) * inv;
  vec3 t1 = (uRayMax - cameraPosition) * inv;
  vec3 tlo = min(t0, t1);
  vec3 thi = max(t0, t1);
  float tn = max(max(max(tlo.x, tlo.y), tlo.z), 0.0);
  float tf = min(min(thi.x, thi.y), thi.z);
  // 切り口: 残す側だけを進む
  if (uCutOn > 0.5) {
    float dn = dot(uCut.xyz, dir);
    float d0 = dot(uCut.xyz, cameraPosition) + uCut.w;
    if (abs(dn) < 1e-9) {
      if (d0 < 0.0) discard;
    } else {
      float tc = -d0 / dn;
      if (dn > 0.0) tn = max(tn, tc); else tf = min(tf, tc);
    }
  }
  if (tf <= tn) discard;
  vec3 size = uBoxMax - uBoxMin;
  float diag = length(size);
  // 歩数は格子の細かさに合わせる（格子の 1 マスより細かく進んでも、値は補間なので変わらない）
  const int MAX_STEPS = 256;
  // 歩幅は格子の箱のいちばん長い辺 / uSteps でそろえる（箱の真ん中を通る光線は今までどおり uSteps 歩。
  // 光線が短い所・縮めた箱では歩数が減る）
  float steps = clamp(ceil(uSteps * (tf - tn) / max(max(size.x, size.y), size.z)), 1.0, float(MAX_STEPS));
  float dt = (tf - tn) / steps;
  // 縞が出ないよう、画素ごとに出発点を少しずらす。白色の乱数だとザラつきが目立つので、隣の画素どうしで値が散らばる模様
  // （Interleaved Gradient Noise）にする
  float jitter = fract(52.9829189 * fract(dot(gl_FragCoord.xy, vec2(0.06711056, 0.00583715))));
  float k = 24.0 * uDensity * uDensity / diag;
  vec4 acc = vec4(0.0);
  float span = uRange.y - uRange.x;
  // 最大値・最小値・平均値: 視線上の値を 1 つにまとめる
  float best = uBlend < 1.5 ? -1e30 : 1e30;
  float sum = 0.0;
  float cnt = 0.0;
  // 複素数: まとめた値をとった点の値（最大値・最小値）と、値の和（平均値）。色はこの偏角
  vec2 bestZ = vec2(0.0);
  vec2 sumZ = vec2(0.0);
  for (int i = 0; i < MAX_STEPS; i++) {
    if (float(i) >= steps) break;
    float t = tn + (float(i) + jitter) * dt;
    vec3 p = cameraPosition + dir * t;
    vec3 q = (p - uBoxMin) / size;
    vec2 z = texture(uTex, q).rg;
    float v = uComplex > 0.5 ? length(z) : z.x;
    if (!(v == v)) continue;
    if (uRegion > 0.5) {
      // 境目からの距離（格子の 1.5 マスを 1 とする）。中身は薄く、境目は濃く塗る（中身だけだと形の縁が分からず、ただの色の塊に見える）。
      // 境目の幅が視線の 1 歩より細いと、歩く所によって境目を踏んだり外したりして、点々のムラになる
      float x = v / (1.5 * max(max(size.x, size.y), size.z) / uN);
      float skin = exp(-0.5 * x * x);
      float dens = 0.35 * (1.0 - smoothstep(-1.0, 1.0, x)) + 1.6 * skin;
      if (dens < 1e-4) continue;
      float a = 1.0 - exp(-k * dens * dt);
      vec3 c = uBase;
      // 陰影は境目の近くだけ（値は距離なので、勾配がそのまま面の法線。中の霧は勾配があてにならず、計算も重い）
      if (uLit > 0.5 && abs(x) < 4.0 && a > 0.002) c = mix(c, lit(c, gradAt(q, size), dir, size, 1.0, 1.0), exp(-0.125 * x * x));
      acc.rgb += (1.0 - acc.a) * a * c;
      acc.a += (1.0 - acc.a) * a;
      if (acc.a > 0.98) break;
      continue;
    }
    float s = span > 0.0 ? (v - uRange.x) / span : 0.5;
    // 複素数の |f| は範囲 [0, q] の外（q より大きい値）も端として描く（q は分布から決めた目安なので）
    if (uClamp > 0.5 || uComplex > 0.5) s = clamp(s, 0.0, 1.0);
    if (s < 0.0 || s > 1.0) continue;
    if (uBlend > 0.5) {
      if (uBlend < 1.5 ? s > best : uBlend < 2.5 ? s < best : false) {
        best = s;
        bestZ = z;
      }
      sum += s;
      sumZ += z;
      cnt += 1.0;
      continue;
    }
    bool sh = uShape > 3.5;
    float a = 1.0 - exp(-k * (sh ? shell(v, q, size) : shapeAt(s, v)) * dt);
    vec3 c = uComplex > 0.5 ? argColor(z) : colormap(s, uColormap, uBase);
    if (uLit > 0.5) {
      // ほとんど透明な点は光の計算を省く（勾配に 6 回テクスチャを読むので重い）
      if (a > 0.002) c = lit(c, sh ? gGrad : gradAt(q, size), dir, size, span, sh ? 1.0 : 0.0);
    } else if (sh) {
      // 光を当てない設定でも、ぼやけた面には視線の向きからの光で陰を付ける（付けないと、面の向きが分からず円盤のように見える）
      float gl = length(gGrad);
      if (gl > 0.0) c *= 0.3 + 0.7 * abs(dot(gGrad / gl, dir));
    }
    acc.rgb += (1.0 - acc.a) * a * c;
    acc.a += (1.0 - acc.a) * a;
    if (acc.a > 0.98) break;
  }
  // 不等式の領域の霧は、重ね方によらず重ね塗り
  if (uBlend > 0.5 && uRegion < 0.5) {
    if (cnt < 0.5) discard;
    float s = uBlend > 2.5 ? sum / cnt : best;
    // 不透明度は、まとめた値での濃さの形を最大 1 にそろえて 1.5 乗したもの × 濃さの 2 倍（濃さ 50% で、形の最大の所が不透明）。
    // 最大値は視線ごとに大きめの値になりやすいので、急に上げないと中くらいの値まで濃くなり、箱全体が塗りつぶされる
    float peak = uShape < 0.5 ? 1.0 : uShape < 2.5 ? 2.0 : 4.0;
    vec2 z = uBlend > 2.5 ? sumZ / cnt : bestZ;
    float a = min(1.0, 2.0 * uDensity * pow(shapeAt(s, length(z)) / peak, 1.5));
    acc = vec4((uComplex > 0.5 ? argColor(z) : colormap(s, uColormap, uBase)) * a, a);
  }
  acc.a *= uOpacity;
  if (acc.a < 0.003) discard;
  gl_FragColor = vec4(acc.rgb / max(acc.a / uOpacity, 1e-6), acc.a);
  #include <colorspace_fragment>
}
`

/**
 * 値の格子（1 辺 n 点。x が最も速く変わる）から 3 次元のテクスチャを作る。定義されない所は NaN のまま（描かない）。
 * 長さが 2n³ なら複素数（実部・虚部の組）として、2 チャンネル（r, g）にする
 */
export function volumeTexture(values: Float32Array, n: number): THREE.Data3DTexture {
  const half = new Uint16Array(values.length)
  for (let i = 0; i < values.length; i++) half[i] = THREE.DataUtils.toHalfFloat(values[i])
  const tex = new THREE.Data3DTexture(half, n, n, n)
  tex.format = values.length === 2 * n * n * n ? THREE.RGFormat : THREE.RedFormat
  tex.type = THREE.HalfFloatType
  tex.minFilter = THREE.LinearFilter
  tex.magFilter = THREE.LinearFilter
  tex.wrapS = tex.wrapT = tex.wrapR = THREE.ClampToEdgeWrapping
  tex.unpackAlignment = 1
  tex.needsUpdate = true
  return tex
}

/**
 * 不等式の領域の霧の格子（値は境目までの距離。負なら中身）で、中身と境目から 3 マスまでの点を囲む箱（表示範囲の座標。
 * 格子の箱 lo〜hi からはみ出さない）。そういう点が無ければ null（何も描かない）
 */
export function regionBounds(values: Float32Array, n: number, lo: number[], hi: number[]): [[number, number, number], [number, number, number]] | null {
  const cell = Math.max(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]) / (n - 1)
  const limit = 3 * cell
  const mn = [n, n, n]
  const mx = [-1, -1, -1]
  let i = 0
  for (let z = 0; z < n; z++)
    for (let y = 0; y < n; y++)
      for (let x = 0; x < n; x++, i++) {
        if (!(values[i] < limit)) continue
        if (x < mn[0]) mn[0] = x
        if (x > mx[0]) mx[0] = x
        if (y < mn[1]) mn[1] = y
        if (y > mx[1]) mx[1] = y
        if (z < mn[2]) mn[2] = z
        if (z > mx[2]) mx[2] = z
      }
  if (mx[0] < 0) return null
  // 格子点の間も少し含める（1 マス広げる）
  const at = (a: number, k: number) => lo[a] + ((hi[a] - lo[a]) * Math.min(n - 1, Math.max(0, k))) / (n - 1)
  return [
    [at(0, mn[0] - 1), at(1, mn[1] - 1), at(2, mn[2] - 1)],
    [at(0, mx[0] + 1), at(1, mx[1] + 1), at(2, mx[2] + 1)],
  ]
}

export function createVolumeMaterial(u: VolumeUniforms): THREE.ShaderMaterial {
  return new THREE.ShaderMaterial({
    uniforms: u as unknown as Record<string, THREE.IUniform>,
    vertexShader: VERTEX,
    fragmentShader: FRAGMENT,
    side: THREE.FrontSide,
    transparent: true,
    depthWrite: false,
  })
}

export function createVolumeUniforms(tex: THREE.Data3DTexture): VolumeUniforms {
  return {
    uTex: { value: tex },
    uBoxMin: { value: new THREE.Vector3() },
    uBoxMax: { value: new THREE.Vector3(1, 1, 1) },
    uRayMin: { value: new THREE.Vector3() },
    uRayMax: { value: new THREE.Vector3(1, 1, 1) },
    uRange: { value: new THREE.Vector2(0, 1) },
    uColormap: { value: 0 },
    uShape: { value: 0 },
    uDensity: { value: 0.5 },
    uBands: { value: 5 },
    uBase: { value: new THREE.Color() },
    uCut: { value: new THREE.Vector4() },
    uCutOn: { value: 0 },
    uOpacity: { value: 1 },
    uSteps: { value: 120 },
    uBlend: { value: 0 },
    uClamp: { value: 0 },
    uN: { value: 64 },
    uWidth: { value: 2 },
    uLit: { value: 1 },
    uComplex: { value: 0 },
    uRegion: { value: 0 },
  }
}

/** 見せ方の設定を、材質の値にする */
export function applyVolumeStyle(u: VolumeUniforms, h: HiddenStyle | undefined, color: THREE.Color, opacity: number) {
  u.uColormap.value = ['viridis', 'turbo', 'shade'].indexOf(h?.colormap ?? 'viridis')
  u.uShape.value = Math.max(0, ['flat', 'high', 'low', 'bands', 'zero'].indexOf(h?.opacity ?? 'flat'))
  u.uDensity.value = h?.density ?? 0.5
  u.uBands.value = Math.max(1, h?.count ?? 5)
  u.uBlend.value = Math.max(0, ['over', 'max', 'min', 'mean'].indexOf(h?.blend ?? 'over'))
  u.uClamp.value = h?.mode === 'field' ? 1 : 0
  // ぼやけた面の幅: 枚数の欄の 1 目盛りで 0.5 マス
  u.uWidth.value = 0.5 * Math.max(1, h?.count ?? 4)
  u.uLit.value = h?.lit === false ? 0 : 1
  u.uBase.value.copy(color)
  u.uOpacity.value = Math.max(0, Math.min(1, opacity))
}
