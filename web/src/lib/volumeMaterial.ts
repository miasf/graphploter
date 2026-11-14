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
  /** 視線を進める歩数（格子の 1 辺の点の数の 1.5 倍。64〜192） */
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

// 値の勾配（表示範囲の座標あたり）。格子の 1 マスの幅の中心差分
vec3 gradAt(vec3 q, vec3 size) {
  float h = 1.0 / uN;
  return vec3(
    texture(uTex, q + vec3(h, 0.0, 0.0)).r - texture(uTex, q - vec3(h, 0.0, 0.0)).r,
    texture(uTex, q + vec3(0.0, h, 0.0)).r - texture(uTex, q - vec3(0.0, h, 0.0)).r,
    texture(uTex, q + vec3(0.0, 0.0, h)).r - texture(uTex, q - vec3(0.0, 0.0, h)).r) / (2.0 * h * size);
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
  vec3 t0 = (uBoxMin - cameraPosition) * inv;
  vec3 t1 = (uBoxMax - cameraPosition) * inv;
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
  const int MAX_STEPS = 192;
  float steps = clamp(uSteps, 1.0, float(MAX_STEPS));
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
  for (int i = 0; i < MAX_STEPS; i++) {
    if (float(i) >= steps) break;
    float t = tn + (float(i) + jitter) * dt;
    vec3 p = cameraPosition + dir * t;
    vec3 q = (p - uBoxMin) / size;
    float v = texture(uTex, q).r;
    if (!(v == v)) continue;
    float s = span > 0.0 ? (v - uRange.x) / span : 0.5;
    if (uClamp > 0.5) s = clamp(s, 0.0, 1.0);
    if (s < 0.0 || s > 1.0) continue;
    if (uBlend > 0.5) {
      if (uBlend < 1.5) best = max(best, s);
      else if (uBlend < 2.5) best = min(best, s);
      sum += s;
      cnt += 1.0;
      continue;
    }
    bool sh = uShape > 3.5;
    float a = 1.0 - exp(-k * (sh ? shell(v, q, size) : shape(s)) * dt);
    vec3 c = colormap(s, uColormap, uBase);
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
  if (uBlend > 0.5) {
    if (cnt < 0.5) discard;
    float s = uBlend > 2.5 ? sum / cnt : best;
    // 不透明度は、まとめた値での濃さの形を最大 1 にそろえて 1.5 乗したもの × 濃さの 2 倍（濃さ 50% で、形の最大の所が不透明）。
    // 最大値は視線ごとに大きめの値になりやすいので、急に上げないと中くらいの値まで濃くなり、箱全体が塗りつぶされる
    float peak = uShape < 0.5 ? 1.0 : uShape < 2.5 ? 2.0 : 4.0;
    float a = min(1.0, 2.0 * uDensity * pow(shape(s) / peak, 1.5));
    acc = vec4(colormap(s, uColormap, uBase) * a, a);
  }
  acc.a *= uOpacity;
  if (acc.a < 0.003) discard;
  gl_FragColor = vec4(acc.rgb / max(acc.a / uOpacity, 1e-6), acc.a);
  #include <colorspace_fragment>
}
`

/** 値の格子（1 辺 n 点。x が最も速く変わる）から 3 次元のテクスチャを作る。定義されない所は NaN のまま（描かない） */
export function volumeTexture(values: Float32Array, n: number): THREE.Data3DTexture {
  const half = new Uint16Array(values.length)
  for (let i = 0; i < values.length; i++) half[i] = THREE.DataUtils.toHalfFloat(values[i])
  const tex = new THREE.Data3DTexture(half, n, n, n)
  tex.format = THREE.RedFormat
  tex.type = THREE.HalfFloatType
  tex.minFilter = THREE.LinearFilter
  tex.magFilter = THREE.LinearFilter
  tex.wrapS = tex.wrapT = tex.wrapR = THREE.ClampToEdgeWrapping
  tex.unpackAlignment = 1
  tex.needsUpdate = true
  return tex
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
