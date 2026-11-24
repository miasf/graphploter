/**
 * 2D 表示の陰関数の曲線・不等式の領域を、メッシュを作らずに画素ごとに描く材質（計画 docs/plan-hard-tasks.md 12 章の G4）。
 *
 * 計算エンジン（core/src/gpu2d.rs）が作った GLSL の gF（値）・gFD（値と勾配）・gFI（矩形の上の飾り付きの区間）を呼ぶ。
 * 表示範囲の平面（縦の座標 0）に貼り、画素ごとに:
 *
 * - 曲線: 画素を線の太さの半分だけ広げた矩形 R で区間を求める。0 を含まなければ描かない。
 *   連続（飾りが Dac 以上）で 4 隅と中心の値の符号が逆なら「必ずある」。分からなければ 2 × 2 → 4 × 4 に分けて調べ、
 *   最後まで分からなければ、連続なら描き、連続でなければ（極・段差）描かない。
 *   「必ずある」ときは、二重数の勾配で F / |∇F| を画素単位の距離に直し、線の太さとの差でなめらかに塗る。
 *   符号の変わらない零点（連続で分からないまま）は、線の太さの内側の矩形で区間が 0 を含むかで塗る
 * - 領域: 画素の矩形で、不等式を全部満たすと区間で言えれば塗り、満たさないと言えれば塗らない。境目は 2 × 2 の点で塗る割合を決める。
 *   境目の線（≤・≥ の分）は曲線と同じ
 *
 * 拡大縮小・移動では作り直さない（画素 → 平面の座標の 1 次式を、描くたびに uniform で渡す）。
 *
 * コンパイルの時間: 区間演算（gFI）と二重数（gFD）を呼ぶ所を 1 か所ずつにし、ループの回数を uniform にしてある（gWork の説明）。
 * Windows の Chrome（Direct3D に変換する）で、呼ぶ所ごとに中身が複製されて、1 式のコンパイルに何秒もかかっていたため
 */
import * as THREE from 'three'

export interface Gpu2dInfo {
  glsl: string
  /** 等式の数（0 か 1） */
  eq: number
  /** 不等式の数 */
  cuts: number
  /** 境目を実線で引く不等式のビット */
  lines: number
  /**
   * 画素の座標 q = (横, 奥, 0) から式の座標（元の座標 x, y, z, w, …）への 1 次式（座標ごとに [A_k0, A_k1, A_k2, b_k]）と、
   * uniform にしたパラメータの名前・値（計画 G7。回転・断面・スライダーで GLSL を作り直さない）。パラメータを畳み込んだ式なら空
   */
  affine: number[]
  params: string[]
  values: number[]
}

export interface Gpu2dUniforms {
  [k: string]: THREE.IUniform
  uColor: { value: THREE.Color }
  /** 線の太さ（画素。CSS の画素で、uRatio を掛けて描画の画素にする） */
  uWidth: { value: number }
  uRatio: { value: number }
  /** 領域の塗りの不透明度 */
  uFill: { value: number }
  /** 領域の格子線（曲面の材質と同じ）: 間隔（横, 奥。0 なら引かない）・色・濃さ・太さ（画素） */
  uGridSpacing: { value: THREE.Vector2 }
  uGridColor: { value: THREE.Color }
  uGridStrength: { value: number }
  uGridWidth: { value: number }
  /** 画素 → 平面の座標の 1 次式: 画素 uC（gl_FragCoord）の座標が uQ0、1 画素右・上に進むと uDx・uDy だけ変わる */
  uQ0: { value: THREE.Vector2 }
  uC: { value: THREE.Vector2 }
  uDx: { value: THREE.Vector2 }
  uDy: { value: THREE.Vector2 }
  /** ループの回数の上限（いつも 400。uniform にして、ループを展開させない） */
  uIters: { value: number }
  /** 画素の座標から式の座標への 1 次式と、スライダーのパラメータ（Gpu2dInfo の affine・values） */
  uPA: { value: THREE.Vector3[] }
  uPb: { value: number[] }
  uParam: { value: number[] }
}

/** 計算エンジンが返した 1 次式・パラメータの値を uniform にする（1 次式が無い、畳み込んだ式なら何もしない） */
export function setGpu2dUniforms(u: Gpu2dUniforms, info: Gpu2dInfo) {
  if (info.affine.length > 0) {
    const n = Math.floor(info.affine.length / 4)
    u.uPA.value = Array.from({ length: n }, (_, k) => new THREE.Vector3(info.affine[4 * k], info.affine[4 * k + 1], info.affine[4 * k + 2]))
    u.uPb.value = Array.from({ length: n }, (_, k) => info.affine[4 * k + 3])
  }
  u.uParam.value = info.values.slice()
}

export const GPU2D_VERTEX = /* glsl */ `
void main() {
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`

/** 画素ごとの判定と塗り（計算エンジンの GLSL の後ろにつなぐ。tools/gpu-g4 の検査もこれを使う） */
export const GPU2D_FRAGMENT = /* glsl */ `
uniform vec3 uColor;
uniform float uWidth;
uniform float uRatio;
uniform float uFill;
uniform vec2 uGridSpacing;
uniform vec3 uGridColor;
uniform float uGridStrength;
uniform float uGridWidth;
uniform vec2 uQ0;
uniform vec2 uC;
uniform vec2 uDx;
uniform vec2 uDy;
// ループの回数の上限。いつも 400 だが、uniform にしておくと、GLSL を Direct3D に変換するときにループを展開しない
uniform int uIters;
// 画素の中心の平面の座標。四角形の上の補間（32 ビットで、表示範囲全体の大きさに比例した誤差が出る）ではなく、
// 画面の中央の画素からの差で求める（深く拡大しても、誤差は座標の値の大きさ × 2⁻²⁴ くらい）
vec2 vQ;

// 座標が spacing の整数倍の近くなら 1（surfaceMaterial.ts の gridLine と同じ）
float gGrid(float coord, float spacing) {
  if (spacing <= 0.0) return 0.0;
  float c = coord / spacing;
  float w = fwidth(c);
  if (w <= 1e-5 * max(abs(c), 1.0)) return 0.0;
  float d = abs(fract(c - 0.5) - 0.5) / w;
  float width = uGridWidth * uRatio;
  float hw = 0.5 * width;
  float line = (1.0 - smoothstep(hw - 0.5, hw + 0.5, d)) * min(width, 1.0);
  return mix(line, min(width * w, 1.0), smoothstep(0.2, 0.5, w));
}

// 点 q で、不等式を全部満たすか（skip と同じ番号の不等式は除く）。
// ループの回数を uniform（uIters）にして、展開させない（展開すると gF の中身が不等式の数だけ複製される）
// 不等式 k が点 q で成り立つか。厳密な不等式（G < 0）は境目の上を含めない（x > 0, x ≤ 0 の x = 0 を塗らない。#139）。
// tol は境目からの近さの許し（線の上に寄せた点で調べるときの、寄せた誤差の分）
bool gHolds(int k, vec2 q, float tol) {
  float v = gF(G_EQ + k, q);
  return ((G_STRICT >> k) & 1) == 1 ? v < -tol : v <= tol;
}

bool gInside(vec2 q, int skip) {
  for (int k = 0; k < uIters; k++) {
    if (k >= G_CUTS) break;
    if (k == skip) continue;
    if (!gHolds(k, q, 0.0)) return false;
  }
  return true;
}

// 曲線の上の点 q（ニュートン法で 1 回寄せた点）で、ほかの不等式が成り立つか。許しは距離で画素の 10⁻³
bool gInsideOn(vec2 q, int skip, vec2 px) {
  for (int k = 0; k < uIters; k++) {
    if (k >= G_CUTS) break;
    if (k == skip) continue;
    vec4 g = gFD(G_EQ + k, q);
    if (!gHolds(k, q, 1e-3 * length(g.yz * px))) return false;
  }
  return true;
}

// 境目の線を引く次の不等式の番号（from 以上。無ければ −1）
int gNextLine(int from) {
  for (int k = 0; k < uIters; k++) {
    if (k >= G_CUTS) break;
    if (k >= from && ((G_LINES >> k) & 1) == 1) return k;
  }
  return -1;
}

// 曲線の判定で調べる矩形の番号 t: 0 は画素を線の太さの半分だけ広げた矩形 R、1〜4 は R を 2 × 2 に分けたもの、
// 5〜20 は 4 × 4 に分けたもの、21 は線の太さの内側の矩形（符号の変わらない零点を塗るか決める）
void gRect(int t, vec2 c, vec2 h, vec2 px, float hw, out vec2 lo, out vec2 hi) {
  if (t == 0) {
    lo = c - h;
    hi = c + h;
  } else if (t <= 20) {
    int n = t <= 4 ? 2 : 4;
    int i = t <= 4 ? t - 1 : t - 5;
    vec2 st = 2.0 * h / float(n);
    lo = c - h + st * vec2(float(i % n), float(i / n));
    hi = lo + st;
  } else {
    vec2 a = px * max(hw - 0.5, 0.25);
    lo = c - a;
    hi = c + a;
  }
}

// 区間演算（gFI）・二重数（gFD）を呼ぶ所を 1 か所ずつにした、画素ごとの判定。
// WebGL を Direct3D に変換するとき（Windows の Chrome など）は、関数を呼ぶ所ごとに中身を複製し、決まった回数のループも展開する。
// 区間演算は式の命令ごとに大きな下請けの関数を呼ぶので、呼ぶ所が多いとコンパイルに何秒もかかっていた。
// そこで、調べることを「仕事」の列にして、1 本のループの中で 1 回に 1 つずつ行う:
//   段 0（領域だけ）: 不等式 k ごとに、画素の矩形の上の区間（全部満たす・全部満たさない・境目）
//   段 1: 曲線（等式、または境目の線を引く不等式）ごとに、矩形 t の判定（無い・必ずある・分からない（連続）・分からない（連続でない））。
//         R で分からなければ 2 × 2、それでも分からなければ 4 × 4 に分け、最後まで分からなければ、連続なら描き、連続でなければ描かない。
//         描くと決まったら、二重数の勾配で F / |∇F| を画素単位の距離に直し、線の太さとの差でなめらかに塗る
void gWork(vec2 c, vec2 px, float hw, out float line, out bool regOut, out bool regAll) {
  line = 0.0;
  regOut = false;
  regAll = true;
  vec2 h = px * (0.5 + hw);
#if G_EQ == 1
  int phase = 1;
  int k = 0;
#else
  int phase = 0;
  int k = 0;
#endif
  int t = 0;
  int sure = 0;
  int cont = 0;
  int broken = 0;
  int s = 0;
  for (int it = 0; it < uIters; it++) {
    if (phase == 2) break;
    int f;
    vec2 lo;
    vec2 hi;
    if (phase == 0) {
      f = G_EQ + k;
      lo = c - 0.5 * px;
      hi = c + 0.5 * px;
    } else {
      f = G_EQ == 1 ? 0 : k;
      gRect(t, c, h, px, hw, lo, hi);
    }
    Div r = gFI(f, lo, hi);
    if (phase == 0) {
      if (gi_none(r) || r.lo > 0.0) regOut = true;
      if (!(r.hi <= 0.0 && r.d >= 1)) regAll = false;
      k++;
      // 領域の外と分かったら、残りの不等式は調べない（境目の線は、線の太さの分だけ外にはみ出すので調べる）
      if (regOut || k >= G_CUTS) {
        phase = 1;
        k = gNextLine(0);
        if (k < 0) phase = 2;
        t = 0;
      }
      continue;
    }
    // 矩形 t の判定: 0 無い、1 必ずある、2 分からない（連続）、3 分からない（連続でない）
    int cls;
    if (gi_none(r) || !(r.lo <= 0.0 && 0.0 <= r.hi)) {
      cls = 0;
    } else if (r.d < 2) {
      cls = 3;
    } else {
      // 4 隅と中心の 5 点で、符号が逆の 2 点があるか
      bool pos = false;
      bool neg = false;
      for (int m = 0; m < uIters; m++) {
        if (m >= 5) break;
        vec2 p = m == 0 ? lo : m == 1 ? hi : m == 2 ? vec2(lo.x, hi.y) : m == 3 ? vec2(hi.x, lo.y) : 0.5 * (lo + hi);
        float v = gF(f, p);
        pos = pos || v > 0.0;
        neg = neg || v < 0.0;
      }
      cls = (pos && neg) ? 1 : 2;
    }
    // この曲線の結果: −1 まだ、0 描かない、1 距離で塗る、2 符号の変わらない零点として塗る
    int done = -1;
    if (t == 0) {
      if (cls == 0) done = 0;
      else if (cls == 1) { s = 1; done = 1; }
      else { t = 1; sure = 0; cont = 0; broken = 0; }
    } else if (t <= 20) {
      if (cls == 1) sure++;
      else if (cls == 2) cont++;
      else if (cls == 3) broken++;
      if (t == 4 || t == 20) {
        if (sure > 0) { s = 1; done = 1; }
        else if (cont + broken == 0) done = 0;
        else if (t == 4) { t = 5; sure = 0; cont = 0; broken = 0; }
        else if (cont == 0) done = 0;
        else t = 21;
      } else {
        t++;
      }
    } else {
      done = cls == 0 ? 0 : 2;
    }
    if (done < 0) continue;
    float a = 0.0;
    if (done > 0) {
      // 1 次の近似の距離 |F| / |∇F|（画素単位）
      vec4 g = gFD(f, c);
      float len = length(g.yz * px);
      float dist = g.x == 0.0 ? 0.0 : abs(g.x) / len;
      if (done == 2) {
        // 符号の変わらない零点（(x² + y² − 4)² = 0 など）。F = u^k では |F| / |∇F| が本当の距離の 1/k になるので、距離では塗らない。
        // 線の太さの内側の矩形に零点がありうるとき（t = 21）に塗る。近似の距離は本当の距離以下なので、遠いと言える所は除く
        a = (dist < 1e30 && dist > hw + 0.5) ? 0.0 : 1.0;
      } else {
        a = !(dist < 1e30) ? 1.0 : 1.0 - smoothstep(hw - 0.5, hw + 0.5, dist);
      }
      // ほかの不等式で切る（等式なら全部の不等式、境目の線ならその不等式以外）。画素の中心でなく、線の上に寄せた点
      // q = c − F ∇F / |∇F|² で調べる（x ≤ 0 の線 x = 0 を、x > 0 の側の画素の中心で x > 0 が成り立つからと残さない）
      if (a > 0.0 && G_CUTS > 0) {
        vec2 q = c;
        if (done == 1) {
          float n2 = dot(g.yz, g.yz);
          if (n2 > 0.0 && g.x == g.x && n2 < 1e30) q = c - g.x * g.yz / n2;
        }
        if (!gInsideOn(q, G_EQ == 1 ? -1 : k, px)) a = 0.0;
      }
    }
    line = max(line, a);
#if G_EQ == 1
    phase = 2;
#else
    k = gNextLine(k + 1);
    if (k < 0) phase = 2;
    t = 0;
#endif
  }
}

void main() {
  vec2 o = gl_FragCoord.xy - uC;
  vQ = uQ0 + o.x * uDx + o.y * uDy;
  vec2 px = abs(uDx) + abs(uDy);
  float hw = 0.5 * uWidth * uRatio;
  float line;
  bool regOut;
  bool regAll;
  gWork(vQ, px, hw, line, regOut, regAll);
  float fill = 0.0;
#if G_EQ == 0
  if (!regOut) {
    if (regAll) {
      fill = 1.0;
    } else {
      // 境目の画素: 2 × 2 の点で塗る割合
      for (int i = 0; i < uIters; i++) {
        if (i >= 4) break;
        vec2 d = (vec2(float(i & 1), float(i >> 1)) - 0.5) * 0.5 * px;
        if (gInside(vQ + d, -1)) fill += 0.25;
      }
    }
  }
  fill *= uFill;
#endif
  vec3 fillRgb = uColor;
  if (fill > 0.0) {
    // 格子線は、塗る割合の分だけ不透明にする（曲面の材質は線の上を不透明にしている）
    float gl = max(gGrid(vQ.x, uGridSpacing.x), gGrid(vQ.y, uGridSpacing.y));
    fillRgb = mix(uColor, uGridColor, gl * uGridStrength);
    fill = max(fill, gl * fill / max(uFill, 1e-3));
  }
  float alpha = line + (1.0 - line) * fill;
  if (alpha <= 0.0) discard;
  gl_FragColor = vec4((line * uColor + (1.0 - line) * fill * fillRgb) / alpha, alpha);
  #include <colorspace_fragment>
}
`

/** 材質を作る（GLSL のコンパイルは、最初に描くとき three.js が行う） */
export function gpu2dMaterial(info: Gpu2dInfo): THREE.ShaderMaterial {
  const uniforms: Gpu2dUniforms = {
    uColor: { value: new THREE.Color(1, 1, 1) },
    uWidth: { value: 3 },
    uRatio: { value: 1 },
    uFill: { value: 0.5 },
    uGridSpacing: { value: new THREE.Vector2() },
    uGridColor: { value: new THREE.Color() },
    uGridStrength: { value: 0.75 },
    uGridWidth: { value: 1 },
    uQ0: { value: new THREE.Vector2() },
    uC: { value: new THREE.Vector2() },
    uDx: { value: new THREE.Vector2(1, 0) },
    uDy: { value: new THREE.Vector2(0, 1) },
    uIters: { value: 400 },
    uPA: { value: [] },
    uPb: { value: [] },
    uParam: { value: [] },
  }
  return new THREE.ShaderMaterial({
    uniforms,
    vertexShader: GPU2D_VERTEX,
    fragmentShader: `precision highp int;\n${info.glsl}\n${GPU2D_FRAGMENT}`,
    transparent: true,
    depthWrite: false,
    depthTest: false,
    side: THREE.DoubleSide,
  })
}

/** 表示範囲の平面（縦の座標 0）に、材質を貼った 1 枚の四角形を置く */
export function gpu2dMesh(mat: THREE.ShaderMaterial, d: { xmin: number; xmax: number; ymin: number; ymax: number }): THREE.Mesh {
  const geo = new THREE.PlaneGeometry(d.xmax - d.xmin, d.ymax - d.ymin)
  geo.translate((d.xmin + d.xmax) / 2, (d.ymin + d.ymax) / 2, 0)
  const m = new THREE.Mesh(geo, mat)
  m.raycast = () => {} // カーソルの下の点は、隠したメッシュの線で求める
  m.frustumCulled = false
  m.renderOrder = 2
  const vp = new THREE.Vector4()
  const ray = new THREE.Vector3()
  // 画素 (fx, fy)（gl_FragCoord と同じ。左下が原点）を通る光線と、平面 z = 0 の交点（JS の 64 ビットで計算する）
  const onPlane = (cam: THREE.Camera, fx: number, fy: number): [number, number] => {
    ray.set(((fx - vp.x) / vp.z) * 2 - 1, ((fy - vp.y) / vp.w) * 2 - 1, 0.5).unproject(cam)
    const o = cam.position
    const t = -o.z / (ray.z - o.z)
    return [o.x + t * (ray.x - o.x), o.y + t * (ray.y - o.y)]
  }
  m.onBeforeRender = (renderer, _scene, camera) => {
    const u = mat.uniforms as Gpu2dUniforms
    u.uRatio.value = renderer.getPixelRatio()
    renderer.getCurrentViewport(vp)
    const cx = vp.x + vp.z / 2
    const cy = vp.y + vp.w / 2
    const q0 = onPlane(camera, cx, cy)
    const qx = onPlane(camera, cx + 1, cy)
    const qy = onPlane(camera, cx, cy + 1)
    u.uQ0.value.set(q0[0], q0[1])
    u.uC.value.set(cx, cy)
    u.uDx.value.set(qx[0] - q0[0], qx[1] - q0[1])
    u.uDy.value.set(qy[0] - q0[0], qy[1] - q0[1])
  }
  return m
}
