/**
 * 3D の陰関数 F = 0 を、メッシュを作らずに光線で描く材質（計画 docs/plan-hard-tasks.md 12 章の G5）。
 *
 * 計算エンジン（core の Program::to_glsl。名前 gR）が作った GLSL の gR（値）・gRd（二重数）・gRi（飾り付きの区間）を呼ぶ。
 * 表示範囲の箱の裏の面を描き、画素ごとにカメラから光線 p(t) = o + t d を飛ばす（G2 の core/src/pixel.rs の ray_hit を移したもの）:
 *
 * - 光線の区間 T の上の箱 o + T d で F を区間演算し、0 を含まなければ捨てる。含めば、T の幅がその深さでの 1 画素より大きい間は
 *   二分し、手前の半分を先に調べる（スタックは 32 段）
 * - 1 画素の幅になったら: 連続（飾りが Dac 以上）で両端の符号が逆なら二分法で詰めて当たり。連続でない（極・段差・定義域の縁）か、
 *   連続で符号が同じ（接するだけの零点・かすめるだけ）なら、1/16 画素まで詰めて決める（64 回まで）
 * - 繰り返しの上限（uMaxIter）で打ち切ったら、そのときの区間の手前の端に描く（?debug=1 なら赤）
 *
 * 見た目は曲面の材質（surfaceMaterial.ts。three.js の MeshStandardMaterial に格子線を足したもの）と同じ計算にする:
 * 光の計算が使う点の位置（vViewPosition）と法線（normal）、格子線の座標（vGridPos）を、当たった点のものに差し替える。
 * 法線は二重数の勾配 ∇F。∇F がカメラの側を向いている面を表、反対を裏とする（裏は曲面の材質と同じく暗くする）。
 * 当たった点の深さを gl_FragDepth に書き、メッシュの図形と正しく前後させる。
 *
 * コンパイルの時間: 2D（gpu2dMaterial.ts）と同じく、区間演算を呼ぶ所は 1 か所、ループの回数は uniform（展開させない）
 */
import * as THREE from 'three'
import { RAY_GLSL } from './raycastGlsl'
import { FRAGMENT_APPLY, FRAGMENT_HEADER, type GridUniforms } from './surfaceMaterial'

export { RAY_GLSL }

export interface RaycastUniforms {
  [k: string]: THREE.IUniform
  /** 表示範囲の箱 */
  uBoxLo: { value: THREE.Vector3 }
  uBoxHi: { value: THREE.Vector3 }
  /** 距離 t での 1 画素（描画の画素）の大きさは uPixel · t */
  uPixel: { value: number }
  /** 光線の区間を分ける回数の上限（主の探索の分。詰める分は別に 64 回まで） */
  uMaxIter: { value: number }
  /** ループの回数の上限（いつも 4096。uniform にして展開させない） */
  uIters: { value: number }
  /** 切り口の平面（n · p + c ≥ 0 の側を残す。three.js の Plane と同じ）。uCutOn が 0 なら切らない */
  uCut: { value: THREE.Vector4 }
  uCutOn: { value: number }
  /** 1 なら打ち切った画素を赤で塗る（?debug=1） */
  uDebug: { value: number }
}



/** main の最初に差し込む: 光線を飛ばし、当たらなければ捨てる。当たれば、光の計算・格子線が使う値を差し替え、深さを書く */
const RAY_MAIN = /* glsl */ `
{
  vec3 ro = cameraPosition;
  vec3 rd = normalize(vRayWorld - cameraPosition);
  vec2 tr = gRayBox(ro, rd);
  if (uCutOn > 0.5) {
    float s0 = dot(uCut.xyz, ro) + uCut.w;
    float sd = dot(uCut.xyz, rd);
    if (sd == 0.0) {
      if (s0 < 0.0) discard;
    } else {
      float tc = -s0 / sd;
      if (sd > 0.0) tr.x = max(tr.x, tc); else tr.y = min(tr.y, tc);
    }
  }
  if (!(tr.x < tr.y)) discard;
  float tHit;
  int res = gRay(ro, rd, tr.x, tr.y, tHit);
  if (res == 0) discard;
  vec3 hit = ro + tHit * rd;
  vec3 g = gRayGrad(hit);
  gRayBack = dot(g, rd) > 0.0;
  vec3 nw = gRayBack ? -g : g;
  if (!(dot(nw, nw) > 0.0) || !(dot(nw, nw) < 1e38)) nw = -rd;
  gRayN = normalize((viewMatrix * vec4(nw, 0.0)).xyz);
  vec4 hv = viewMatrix * vec4(hit, 1.0);
  vViewPosition = -hv.xyz;
  vGridPos = hit;
  vec4 clip = projectionMatrix * hv;
  gl_FragDepth = clamp(0.5 * (clip.z / clip.w) + 0.5, 0.0, 1.0);
  gRayAborted = res == 2;
}
`

/**
 * 材質を作る。grid は曲面の材質と同じ形の格子線などの値（式の ⚙ の設定がそのまま効く）。
 * glsl は計算エンジンが作った GLSL（gR・gRd・gRi と下請けの関数）
 */
export function createRaycastMaterial(glsl: string, grid: GridUniforms, ray: RaycastUniforms): THREE.MeshStandardMaterial {
  const m = new THREE.MeshStandardMaterial({ side: THREE.BackSide, roughness: 0.55, metalness: 0.05 })
  m.onBeforeCompile = (shader) => {
    Object.assign(shader.uniforms, grid, ray)
    shader.vertexShader = shader.vertexShader
      .replace('#include <common>', '#include <common>\nvarying vec3 vRayWorld;')
      .replace('#include <begin_vertex>', '#include <begin_vertex>\nvRayWorld = (modelMatrix * vec4(transformed, 1.0)).xyz;')
    // 曲面の材質の差し込み: 格子線の座標と複素数の色は、頂点の値ではなく当たった点で決める。裏面の判定は ∇F の向き
    const header = FRAGMENT_HEADER.replace('varying vec3 vGridPos;', 'vec3 vGridPos;').replace('varying vec3 vComplex;', 'vec3 vComplex = vec3(0.0);')
    const apply = FRAGMENT_APPLY.replace('!gl_FrontFacing', 'gRayBack')
    shader.fragmentShader = shader.fragmentShader
      // 光の計算が使う点の位置を、箱の面ではなく当たった点にする（同じ名前の変数にして、three.js の部品をそのまま使う）
      .replace('varying vec3 vViewPosition;', 'vec3 vViewPosition;')
      .replace('#include <common>', `#include <common>\nuniform mat4 projectionMatrix;\nbool gRayAborted = false;\n${header}\n${glsl}\n${RAY_GLSL}`)
      .replace('#include <clipping_planes_fragment>', RAY_MAIN)
      .replace('#include <color_fragment>', `#include <color_fragment>\n${apply}\nif (gRayAborted && uDebug > 0.5) diffuseColor.rgb = vec3(1.0, 0.0, 0.0);`)
      .replace('#include <normal_fragment_begin>', '#include <normal_fragment_begin>\nnormal = gRayN;\nnonPerturbedNormal = gRayN;')
  }
  // 式ごとに GLSL が違うので、three.js のシェーダーの使い回しの鍵にも入れる
  m.customProgramCacheKey = () => 'raycast:' + glsl
  return m
}

export function createRaycastUniforms(): RaycastUniforms {
  return {
    uBoxLo: { value: new THREE.Vector3() },
    uBoxHi: { value: new THREE.Vector3() },
    uPixel: { value: 1e-3 },
    uMaxIter: { value: 256 },
    uIters: { value: 4096 },
    uCut: { value: new THREE.Vector4() },
    uCutOn: { value: 0 },
    uDebug: { value: typeof location !== 'undefined' && new URLSearchParams(location.search).get('debug') === '1' ? 1 : 0 },
  }
}

/**
 * 表示範囲の箱（裏の面を描く）に材質を貼る。描くたびに、1 画素の大きさを今のカメラと描画の大きさから決める
 */
export function raycastMesh(mat: THREE.MeshStandardMaterial, ray: RaycastUniforms, d: { xmin: number; xmax: number; ymin: number; ymax: number; zmin: number; zmax: number }): THREE.Mesh {
  const geo = new THREE.BoxGeometry(d.xmax - d.xmin, d.ymax - d.ymin, d.zmax - d.zmin)
  geo.translate((d.xmin + d.xmax) / 2, (d.ymin + d.ymax) / 2, (d.zmin + d.zmax) / 2)
  ray.uBoxLo.value.set(d.xmin, d.ymin, d.zmin)
  ray.uBoxHi.value.set(d.xmax, d.ymax, d.zmax)
  const mesh = new THREE.Mesh(geo, mat)
  mesh.raycast = () => {} // カーソルの下の点は、隠したメッシュで求める
  mesh.frustumCulled = false
  const vp = new THREE.Vector4()
  mesh.onBeforeRender = (renderer, _scene, camera) => {
    renderer.getCurrentViewport(vp)
    const fov = (camera as THREE.PerspectiveCamera).fov ?? 45
    ray.uPixel.value = (2 * Math.tan(THREE.MathUtils.degToRad(fov) / 2)) / Math.max(1, vp.w)
  }
  return mesh
}
