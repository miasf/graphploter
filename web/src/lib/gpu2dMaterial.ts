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
 * 拡大縮小・移動では作り直さない（平面の上の座標から画素の大きさを fwidth で求める）。
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

// 矩形 [lo, hi] での判定: 0 無い、1 必ずある、2 分からない（連続）、3 分からない（連続でない）
int gClassify(int f, vec2 lo, vec2 hi) {
  Div r = gFI(f, lo, hi);
  if (gi_none(r) || !(r.lo <= 0.0 && 0.0 <= r.hi)) return 0;
  if (r.d < 2) return 3;
  float a = gF(f, lo);
  float b = gF(f, hi);
  float c = gF(f, vec2(lo.x, hi.y));
  float d = gF(f, vec2(hi.x, lo.y));
  float e = gF(f, 0.5 * (lo + hi));
  bool pos = a > 0.0 || b > 0.0 || c > 0.0 || d > 0.0 || e > 0.0;
  bool neg = a < 0.0 || b < 0.0 || c < 0.0 || d < 0.0 || e < 0.0;
  return (pos && neg) ? 1 : 2;
}

// 曲線 F_f = 0 の濃さ（0〜1）。px は 1 画素の大きさ（座標の単位）、hw は線の太さの半分（画素）
float gCurve(int f, vec2 c, vec2 px, float hw) {
  vec2 h = px * (0.5 + hw);
  int s = gClassify(f, c - h, c + h);
  if (s == 0) return 0.0;
  if (s != 1) {
    for (int lev = 0; lev < 2; lev++) {
      int n = lev == 0 ? 2 : 4;
      vec2 st = 2.0 * h / float(n);
      int sure = 0;
      int cont = 0;
      int broken = 0;
      for (int i = 0; i < 4; i++) {
        for (int j = 0; j < 4; j++) {
          if (i >= n || j >= n) continue;
          vec2 lo = c - h + st * vec2(float(i), float(j));
          int t = gClassify(f, lo, lo + st);
          if (t == 1) sure++;
          else if (t == 2) cont++;
          else if (t == 3) broken++;
        }
      }
      if (sure > 0) { s = 1; break; }
      if (cont + broken == 0) return 0.0;
      s = cont > 0 ? 2 : 3;
    }
    if (s == 3) return 0.0;
  }
  // 1 次の近似の距離 |F| / |∇F|（画素単位）
  vec4 g = gFD(f, c);
  float len = length(g.yz * px);
  float dist = g.x == 0.0 ? 0.0 : abs(g.x) / len;
  if (s == 2) {
    // 符号の変わらない零点（(x² + y² − 4)² = 0 など）。F = u^k では |F| / |∇F| が本当の距離の 1/k になるので、距離では塗らない。
    // 線の太さの内側の矩形に零点がありうるかで塗る（近似の距離は本当の距離以下なので、遠いと言える所を除くのには使える）
    vec2 hi = px * max(hw - 0.5, 0.25);
    if (gClassify(f, c - hi, c + hi) == 0) return 0.0;
    return (dist < 1e30 && dist > hw + 0.5) ? 0.0 : 1.0;
  }
  if (!(dist < 1e30)) return 1.0;
  return 1.0 - smoothstep(hw - 0.5, hw + 0.5, dist);
}

// 点 q で、不等式を全部満たすか（k と同じ番号の不等式は除く）
bool gInside(vec2 q, int skip) {
  for (int k = 0; k < G_CUTS; k++) {
    if (k == skip) continue;
    if (!(gF(G_EQ + k, q) <= 0.0)) return false;
  }
  return true;
}

// 領域の塗る割合（0〜1）
float gRegion(vec2 c, vec2 px) {
  vec2 h = 0.5 * px;
  bool all = true;
  for (int k = 0; k < G_CUTS; k++) {
    Div r = gFI(G_EQ + k, c - h, c + h);
    if (gi_none(r) || r.lo > 0.0) return 0.0;
    if (!(r.hi <= 0.0 && r.d >= 1)) all = false;
  }
  if (all) return 1.0;
  float cov = 0.0;
  for (int i = 0; i < 4; i++) {
    vec2 o = (vec2(float(i & 1), float(i >> 1)) - 0.5) * 0.5 * px;
    if (gInside(c + o, -1)) cov += 0.25;
  }
  return cov;
}

void main() {
  vec2 o = gl_FragCoord.xy - uC;
  vQ = uQ0 + o.x * uDx + o.y * uDy;
  vec2 px = abs(uDx) + abs(uDy);
  float hw = 0.5 * uWidth * uRatio;
  float line = 0.0;
  float fill = 0.0;
#if G_EQ == 1
  line = gCurve(0, vQ, px, hw);
  if (line > 0.0 && G_CUTS > 0 && !gInside(vQ, -1)) line = 0.0;
#else
  fill = gRegion(vQ, px) * uFill;
  for (int k = 0; k < G_CUTS; k++) {
    if (((G_LINES >> k) & 1) == 0) continue;
    float a = gCurve(G_EQ + k, vQ, px, hw);
    if (a > 0.0 && !gInside(vQ, k)) a = 0.0;
    line = max(line, a);
  }
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
