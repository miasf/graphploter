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
 * 回転・断面・スライダー（計画 G7）: 式の GLSL は、回転も断面も入れる前の元の座標 x, y, z, w, …（G_N 個）の式。
 * 表示中の座標 q から元の座標への 1 次式 P = A q + b（uPA・uPb）と、スライダーのパラメータ（uParam）を uniform にして、
 * どの組の回転（表示していない座標が絡むものも）・断面の値・スライダーを動かしても GLSL を作り直さない
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
  /** 区間演算の回数の合計の上限（主の探索と詰める分の合計） */
  uWork: { value: number }
  /** ループの回数の上限（いつも 4096。uniform にして展開させない） */
  uIters: { value: number }
  /** 切り口の平面（n · p + c ≥ 0 の側を残す。three.js の Plane と同じ）。uCutOn が 0 なら切らない */
  uCut: { value: THREE.Vector4 }
  uCutOn: { value: number }
  /** 1 なら打ち切った画素を赤で塗る（?debug=1） */
  uDebug: { value: number }
  /** 表示中の座標 q から式の座標 P への 1 次式 P_k = uPA[k] · q + uPb[k]（式の座標の数だけ） */
  uPA: { value: THREE.Vector3[] }
  uPb: { value: number[] }
  /** スライダーのパラメータ（GLSL の uniform float uParam[N]。式の GLSL にパラメータが無ければ使わない） */
  uParam: { value: number[] }
  /** カメラの手前の面までの距離（これより近い所は描かない） */
  uNear: { value: number }
  /** 1 なら、光を当てずに (打ち切ったか, 当たったか, 0, 1) を出す（打ち切りの割合を数えるとき。Viewer の probeGpu3d） */
  uProbe: { value: number }
}

/** 計算エンジンが返した 1 次式（式の座標ごとに [A_k0, A_k1, A_k2, b_k]）を uniform にする */
export function setRayAffine(ray: RaycastUniforms, affine: ArrayLike<number>) {
  const n = Math.floor(affine.length / 4)
  ray.uPA.value = Array.from({ length: n }, (_, k) => new THREE.Vector3(affine[4 * k], affine[4 * k + 1], affine[4 * k + 2]))
  ray.uPb.value = Array.from({ length: n }, (_, k) => affine[4 * k + 3])
}

/**
 * 回転だけが変わった結果（回転が表示中の座標どうしだけのとき。回す前の形 p から表示中の座標への q = M p + t。
 * [M の 1 行目, t₀, 2 行目, t₁, 3 行目, t₂]）で、表示中の座標の行だけを書き換える: p = Mᵀ (q − t)。
 * axes は表示中の座標の番号（p の a 番目が、式の座標 axes[a]）。表示していない座標の行（断面の値）はそのまま
 */
export function applyShownRotation(ray: RaycastUniforms, axes: number[], t: ArrayLike<number>) {
  const m = (r: number, c: number) => t[4 * r + c]
  const tv = [t[3], t[7], t[11]]
  for (let a = 0; a < 3; a++) {
    const k = axes[a]
    if (k >= ray.uPA.value.length) continue
    // p_a = Σ_b M[b][a] (q_b − t_b)
    ray.uPA.value[k].set(m(0, a), m(1, a), m(2, a))
    ray.uPb.value[k] = -(m(0, a) * tv[0] + m(1, a) * tv[1] + m(2, a) * tv[2])
  }
}



/**
 * 光線が箱・カメラの手前の面・切り口の平面で切られた区間 [t0, t1]（空なら t0 ≥ t1）。
 * カメラが箱の中にいるとき、光線はカメラの位置（t = 0）から始まる。1 画素の大きさ（uPixel · t）は t = 0 で 0 になるので、
 * 面が密な式（tan(xyz) = 1 など）ではカメラのすぐ手前を際限なく細かく分け、繰り返しの上限まで使い切っていた（拡大して 1 コマ 0.5 秒）。
 * メッシュと同じく、カメラの手前の面（near）より近い所は描かない
 */
const RAY_RANGE = /* glsl */ `
vec2 gRayRange(vec3 ro, vec3 rd) {
  vec2 tr = gRayBox(ro, rd);
  tr.x = max(tr.x, uNear);
  if (uCutOn > 0.5) {
    float s0 = dot(uCut.xyz, ro) + uCut.w;
    float sd = dot(uCut.xyz, rd);
    if (sd == 0.0) {
      if (s0 < 0.0) return vec2(1.0, 0.0);
    } else {
      float tc = -s0 / sd;
      if (sd > 0.0) tr.x = max(tr.x, tc); else tr.y = min(tr.y, tc);
    }
  }
  return tr;
}
`

/** main の最初に差し込む: 光線を飛ばし、当たらなければ捨てる。当たれば、光の計算・格子線が使う値を差し替え、深さを書く */
const RAY_MAIN = /* glsl */ `
{
  vec3 ro = cameraPosition;
  vec3 rd = normalize(vRayWorld - cameraPosition);
  // 隣の画素への光線の向きの差（輪郭の画素で、画素の中の別の位置へ光線を飛ばすのに使う）。捨てる前に求める
  // （2 × 2 の画素の組のどれかを先に捨てると、微分が決まらない）
  vec3 dRx = dFdx(rd);
  vec3 dRy = dFdy(rd);
  vec2 tr = gRayRange(ro, rd);
  // 光線は表示中の座標のまま。式の座標への 1 次式は gRayV・gRayI・gRayGrad の中で掛ける（勾配も表示中の座標についてのもの）
  float tHit = 0.0;
  int res = 0;
  if (tr.x < tr.y) res = gRay(ro, rd, tr.x, tr.y, tHit);
  // 輪郭のアンチエイリアス: 画素の中の 4 点（回した格子）へも光線を飛ばし、当たった割合を MSAA の標本に割り振る（alphaToCoverage）。
  // - 真ん中の光線が当たった画素: 4 本とも、まず当たった深さの前後 2 画素分だけを探す（面の上なら区間演算 数回で済む）。
  //   そこに無ければ（輪郭の外へはみ出した）、その先を全部探す。奥の別の面に当たれば覆われているとみなす（段差の継ぎ目を出さない）
  // - 外れた画素: 2 × 2 の画素の組の中で当たる画素と隣り合うとき（fwidth）だけ、4 本を全部探す（背景の画素を全部探すと重い）
  // 2 × 2 の組の境目にある輪郭は、外側の画素では見つからないが、内側（当たった画素）では必ず調べるので、輪郭はなめらかになる
  float hitF = res != 0 ? 1.0 : 0.0;
  bool missEdge = res == 0 && fwidth(hitF) > 0.0;
  float cover = hitF;
  // 視点を動かしている間（光線の回数を減らしているとき。uMaxIter が RAY_STILL の 256 より少ない）はしない（輪郭の画素で光線が最大 5 倍になり、動きが重くなる）
  if ((res != 0 || missEdge) && uProbe < 0.5 && uMaxIter >= 256) {
    float bestT = 1e30;
    vec3 bestD = rd;
    int bestRes = 0;
    float wNear = 2.0 * uPixel * tHit;
    int n = 0;
    // 8 回: 光線 k = s / 2 の、手前の探索（s が偶数）と、その先の探索（奇数。手前で見つからなかったときだけ）
    bool found = false;
    for (int s = 0; s < 8; s++) {
      int k = s / 2;
      bool second = s - 2 * k == 1;
      if (second && (found || res == 0)) continue;
      vec2 o = k == 0 ? vec2(-0.125, -0.375) : k == 1 ? vec2(0.375, -0.125) : k == 2 ? vec2(0.125, 0.375) : vec2(-0.375, 0.125);
      vec3 d = normalize(rd + dRx * o.x + dRy * o.y);
      vec2 trk = gRayRange(ro, d);
      if (res != 0) trk = second ? vec2(max(trk.x, tHit + wNear), trk.y) : vec2(max(trk.x, tHit - wNear), min(trk.y, tHit + wNear));
      float tk = 0.0;
      int rk = 0;
      if (trk.x < trk.y) rk = gRay(ro, d, trk.x, trk.y, tk);
      found = rk != 0;
      if (found) {
        n++;
        if (tk < bestT) { bestT = tk; bestD = d; bestRes = rk; }
      }
    }
    cover = (hitF + float(n)) * 0.2;
    // 真ん中の光線が外れた輪郭の画素は、当たった中でいちばん手前の光線の点で塗る
    if (res == 0 && bestRes != 0) { rd = bestD; tHit = bestT; res = bestRes; }
  }
  if (res == 0) discard;
  gRayCover = cover;
  vec3 hit = ro + tHit * rd;
  vec3 g = gRayGrad(hit);
  gRayBack = dot(g, rd) > 0.0;
  vec3 nw = gRayBack ? -g : g;
  // 勾配が 0（式がまるごと 0 になる範囲の表面 floor(x) + floor(y) + floor(z) = 0 など）・求まらないときは、
  // 隣の画素との当たった点の差から面の向きを求める（カメラの側に向ける）。それも求まらなければ、カメラの向き
  vec3 dn = cross(dFdx(hit), dFdy(hit));
  if (!(dot(nw, nw) > 0.0) || !(dot(nw, nw) < 1e38)) {
    gRayBack = false;
    // 値が飛ぶ面（floor の和の立方体など）: 当たった点の前後 1 画素で、値がいちばん大きく変わる軸を面の向きにする。
    // 隣の画素との当たった点の差（dn）は、奥の別の面と隣り合う画素（輪郭・段差）で向きがでたらめになり、黒い点々になっていた
    float e = uPixel * tHit;
    vec3 jump = vec3(
      abs(gRayV(hit + vec3(e, 0.0, 0.0)) - gRayV(hit - vec3(e, 0.0, 0.0))),
      abs(gRayV(hit + vec3(0.0, e, 0.0)) - gRayV(hit - vec3(0.0, e, 0.0))),
      abs(gRayV(hit + vec3(0.0, 0.0, e)) - gRayV(hit - vec3(0.0, 0.0, e))));
    jump = vec3(jump.x == jump.x ? jump.x : 0.0, jump.y == jump.y ? jump.y : 0.0, jump.z == jump.z ? jump.z : 0.0);
    float jm = max(jump.x, max(jump.y, jump.z));
    if (jm > 0.0) {
      vec3 ax = jump.x == jm ? vec3(1.0, 0.0, 0.0) : jump.y == jm ? vec3(0.0, 1.0, 0.0) : vec3(0.0, 0.0, 1.0);
      nw = dot(ax, rd) > 0.0 ? -ax : ax;
    } else {
      nw = dot(dn, rd) > 0.0 ? -dn : dn;
      if (!(dot(nw, nw) > 0.0) || !(dot(nw, nw) < 1e38)) nw = -rd;
    }
  }
  gRayN = normalize((viewMatrix * vec4(nw, 0.0)).xyz);
  // 当たった点での 1 画素の大きさ（面に沿った長さ）。格子線の太さの見積もりの上限に使う
  gRayFoot = uPixel * tHit / max(abs(dot(normalize(nw), rd)), 0.05);
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
 * glsl は計算エンジンが作った GLSL（gR・gRd・gRi と下請けの関数）、n は式の座標の数（gR の引数の配列の長さ）
 */
export function createRaycastMaterial(glsl: string, n: number, grid: GridUniforms, ray: RaycastUniforms): THREE.MeshStandardMaterial {
  const m = new THREE.MeshStandardMaterial({ side: THREE.BackSide, roughness: 0.55, metalness: 0.05 })
  // 輪郭の画素の当たった割合（gRayCover）を、MSAA の標本の数に割り振る（並べ替えの要らないアンチエイリアス）
  m.alphaToCoverage = true
  m.onBeforeCompile = (shader) => {
    Object.assign(shader.uniforms, grid, ray)
    shader.vertexShader = shader.vertexShader
      .replace('#include <common>', '#include <common>\nvarying vec3 vRayWorld;')
      .replace('#include <begin_vertex>', '#include <begin_vertex>\nvRayWorld = (modelMatrix * vec4(transformed, 1.0)).xyz;')
    // 曲面の材質の差し込み: 格子線の座標と複素数の色は、頂点の値ではなく当たった点で決める。裏面の判定は ∇F の向き
    // 格子線は、隣の画素との座標の差（fwidth）で太さを決め、座標が変わらない面（格子線の座標の上にある面）には引かない。
    // 光線で当たった点の座標は 32 ビットの精度で揺れる（x = 1 の面で 1.000001 など）ので、そのままでは面の上に格子線の点々が出た
    // （立方体 max(|x|, |y|, |z|) = 1・floor の和の立体）。当たった点での 1 画素の大きさ（gRayFoot）の 2 % より変化が小さければ
    // 「変わらない」とみなす。隣の画素と深さが食い違って差が大きくなりすぎた分は、1 画素の大きさに抑える
    const header = FRAGMENT_HEADER.replace('varying vec3 vGridPos;', 'vec3 vGridPos;')
      .replace('varying vec3 vComplex;', 'vec3 vComplex = vec3(0.0);')
      .replace('float w = fwidth(c);', 'float w = min(fwidth(c), gRayFoot / spacing);')
      .replace('if (w <= 1e-5 * max(abs(c), 1.0)) return 0.0;', 'if (w <= max(1e-5 * max(abs(c), 1.0), 0.02 * gRayFoot / spacing)) return 0.0;')
    if (!header.includes('0.02 * gRayFoot')) throw new Error('格子線の差し込みの場所が見つからない（surfaceMaterial.ts の gridLine）')
    const apply = FRAGMENT_APPLY.replace('!gl_FrontFacing', 'gRayBack')
    shader.fragmentShader = shader.fragmentShader
      // 光の計算が使う点の位置を、箱の面ではなく当たった点にする（同じ名前の変数にして、three.js の部品をそのまま使う）
      .replace('varying vec3 vViewPosition;', 'vec3 vViewPosition;')
      .replace('#include <common>', `#include <common>\nuniform mat4 projectionMatrix;\nuniform float uNear;\nuniform float uProbe;\n#define G_N ${n}\nbool gRayAborted = false;\nfloat gRayFoot = 1e30;\nfloat gRayCover = 1.0;\n${header}\n${glsl}\n${RAY_GLSL}\n${RAY_RANGE}`)
      .replace('#include <clipping_planes_fragment>', RAY_MAIN)
      .replace('#include <color_fragment>', `#include <color_fragment>\n${apply}\nif (gRayAborted && uDebug > 0.5) diffuseColor.rgb = vec3(1.0, 0.0, 0.0);\ndiffuseColor.a *= gRayCover;`)
      .replace('#include <normal_fragment_begin>', '#include <normal_fragment_begin>\nnormal = gRayN;\nnonPerturbedNormal = gRayN;')
      .replace('#include <dithering_fragment>', '#include <dithering_fragment>\nif (uProbe > 0.5) gl_FragColor = vec4(gRayAborted ? 1.0 : 0.0, 1.0, 0.0, 1.0);')
  }
  // 式ごとに GLSL が違うので、three.js のシェーダーの使い回しの鍵にも入れる
  m.customProgramCacheKey = () => 'raycast:' + glsl
  return m
}

/**
 * 光線の回数の上限。止まっているとき（RAY_STILL）と、視点を動かしている間（RAY_MOVING）。
 * 動かしている間は軽くして、画面全体の画素密度を下げてぼやけさせる代わりにする（打ち切った画素は手前の候補に描く。止まったら描き直す）。
 * 止まっているときの合計の上限は、ループの上限（uIters）と同じにする: mod(x y, z) = 0 のように、最初の面に当たるまでに値が飛ぶ所を
 * たくさん通る式は、飛ぶ所ごとに 1/16 画素まで詰めるので、1 本の光線に 1500 回を超える区間演算が要る（1536 にしたら全部の画素が打ち切りになった）
 */
export const RAY_STILL = { maxIter: 256, work: 4096 }
export const RAY_MOVING = { maxIter: 64, work: 192 }

export function createRaycastUniforms(): RaycastUniforms {
  return {
    uBoxLo: { value: new THREE.Vector3() },
    uBoxHi: { value: new THREE.Vector3() },
    uPixel: { value: 1e-3 },
    uMaxIter: { value: RAY_STILL.maxIter },
    uWork: { value: RAY_STILL.work },
    uIters: { value: 4096 },
    uCut: { value: new THREE.Vector4() },
    uCutOn: { value: 0 },
    uDebug: { value: typeof location !== 'undefined' && new URLSearchParams(location.search).get('debug') === '1' ? 1 : 0 },
    uPA: { value: [new THREE.Vector3(1, 0, 0), new THREE.Vector3(0, 1, 0), new THREE.Vector3(0, 0, 1)] },
    uPb: { value: [0, 0, 0] },
    uParam: { value: [] },
    uNear: { value: 0 },
    uProbe: { value: 0 },
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
    // 小さな描画先で打ち切りを数えるときは、画面の大きさでの 1 画素の大きさを使う（userData.pixelH。同じ深さまで詰める）
    const h = (mesh.userData.pixelH as number | undefined) ?? vp.w
    ray.uPixel.value = (2 * Math.tan(THREE.MathUtils.degToRad(fov) / 2)) / Math.max(1, h)
    ray.uNear.value = (camera as THREE.PerspectiveCamera).near ?? 0
  }
  return mesh
}
