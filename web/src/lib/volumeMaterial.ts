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
  /** 不透明度の形（0: 一様, 1: 値が大きいほど濃い, 2: 小さいほど濃い, 3: 何枚かの値の所だけ濃い） */
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
varying vec3 vWorld;
${COLORMAP_GLSL}

// 値の位置 t（0〜1）での、単位の長さあたりの濃さの形
float shape(float t) {
  if (uShape < 0.5) return 1.0;
  // 2 乗にして、端の値と中くらいの値の差をはっきりさせる（1 乗だと中くらいの値も濃く、霧のようになる）
  if (uShape < 1.5) return 2.0 * t * t;
  if (uShape < 2.5) return 2.0 * (1.0 - t) * (1.0 - t);
  float f = fract(t * uBands) - 0.5;
  return 4.0 * exp(-f * f / (2.0 * 0.07 * 0.07));
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
  for (int i = 0; i < MAX_STEPS; i++) {
    if (float(i) >= steps) break;
    float t = tn + (float(i) + jitter) * dt;
    vec3 p = cameraPosition + dir * t;
    float v = texture(uTex, (p - uBoxMin) / size).r;
    if (!(v == v)) continue;
    float s = span > 0.0 ? (v - uRange.x) / span : 0.5;
    if (s < 0.0 || s > 1.0) continue;
    float a = 1.0 - exp(-k * shape(s) * dt);
    vec3 c = colormap(s, uColormap, uBase);
    acc.rgb += (1.0 - acc.a) * a * c;
    acc.a += (1.0 - acc.a) * a;
    if (acc.a > 0.98) break;
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
  }
}

/** 見せ方の設定を、材質の値にする */
export function applyVolumeStyle(u: VolumeUniforms, h: HiddenStyle | undefined, color: THREE.Color, opacity: number) {
  u.uColormap.value = ['viridis', 'turbo', 'shade'].indexOf(h?.colormap ?? 'viridis')
  u.uShape.value = ['flat', 'high', 'low', 'bands'].indexOf(h?.opacity ?? 'flat')
  u.uDensity.value = h?.density ?? 0.5
  u.uBands.value = Math.max(1, h?.count ?? 5)
  u.uBase.value.copy(color)
  u.uOpacity.value = Math.max(0, Math.min(1, opacity))
}
