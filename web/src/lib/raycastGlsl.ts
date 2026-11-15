/**
 * 3D の陰関数を光線で描く GLSL（計画 12 章の G5。説明は raycastMaterial.ts）。
 * import の無いファイルにしてあるのは、検査（tools/gpu-g5/check.mjs）が Node からそのまま読むため
 */
/** 光線の計算（gR・gRd・gRi の後ろにつなぐ。tools/gpu-g5 の検査もこれを使う） */
export const RAY_GLSL = /* glsl */ `
uniform vec3 uBoxLo;
uniform vec3 uBoxHi;
uniform float uPixel;
uniform int uMaxIter;
// 区間演算の回数の合計の上限（主の探索と、1/16 画素まで詰める分の合計）
uniform int uWork;
uniform int uIters;
uniform vec4 uCut;
uniform float uCutOn;
uniform float uDebug;

float gRayV(vec3 q) {
  float p[3];
  p[0] = q.x;
  p[1] = q.y;
  p[2] = q.z;
  return gR(p);
}

// 光線の区間 [a, b] の上の箱
Div gRayI(vec3 o, vec3 d, float a, float b) {
  vec3 pa = o + a * d;
  vec3 pb = o + b * d;
  vec3 lo = min(pa, pb);
  vec3 hi = max(pa, pb);
  vec2 p[3];
  p[0] = vec2(lo.x, hi.x);
  p[1] = vec2(lo.y, hi.y);
  p[2] = vec2(lo.z, hi.z);
  return gRi(p);
}

vec3 gRayGrad(vec3 q) {
  vec4 p[3];
  p[0] = vec4(q.x, 1.0, 0.0, 0.0);
  p[1] = vec4(q.y, 0.0, 1.0, 0.0);
  p[2] = vec4(q.z, 0.0, 0.0, 1.0);
  return gRd(p).yzw;
}

// 光線 o + t d（t ∈ [t0, t1]）が F = 0 に最初に当たる所。戻り値 0: 当たらない、1: 当たった（tHit）、2: 打ち切った（tHit は手前の候補）。
// 区間演算（gRayI）・値（gRayV）を呼ぶ所は 1 か所ずつ（呼ぶ所ごとに中身が複製されて、コンパイルが重くなるため）
int gRay(vec3 o, vec3 d, float t0, float t1, out float tHit) {
  // 区間のスタック。詰める分（1/16 画素まで）の区間は、右の端を負にして見分ける（t は 0 以上）。
  // 配列に書く所は、ループの最後の 1 か所だけにする。Direct3D に変換したとき（Windows の Chrome・RTX 2060 で確かめた）、
  // 分かれ道の中で配列の変数の番号に書いた値が残らないことがあり、詰める分を積めずに同じ区間を取り出し続けて止まらなくなった
  float sa[32];
  float sb[32];
  sa[0] = t0;
  sb[0] = t1;
  int n = 1;
  int mainIt = 0;
  int work = 0;
  int budget = 0;
  float epsR = 0.0;
  bool bis = false;
  // 値がちょうど 0 の所に当たった（式がまるごと 0 になる範囲など）: 0 でない所から 0 の所への境目を二分で詰める
  bool zeroEdge = false;
  float blo = 0.0;
  float bhi = 0.0;
  float flo = 0.0;
  tHit = t0;
  for (int it = 0; it < uIters; it++) {
    if (n == 0) break;
    n--;
    float a = sa[n];
    float bs = sb[n];
    bool refine = bs < 0.0;
    float b = abs(bs);
    // この回に積む区間（push 個。2 つなら (pa1, pb1) が上＝先に調べる）
    int push = 0;
    float pa0 = 0.0;
    float pb0 = 0.0;
    float pa1 = 0.0;
    float pb1 = 0.0;
    bool skip = false;
    if (!refine) {
      mainIt++;
      if (mainIt > uMaxIter) { tHit = a; return 2; }
    } else if (budget <= 0) {
      // 詰める分の回数を使いきった: 残りの詰める分は捨てる
      skip = true;
    } else {
      budget--;
    }
    if (!skip) {
      // 仕事の総量の上限: 極が密な式（tan(xyz) = 1 の奥など）では、極を含む区間ごとに詰める分が走り、1 本の光線の仕事が膨らむ
      work++;
      if (work > uWork) { tHit = a; return 2; }
      Div r = gRayI(o, d, a, b);
      if (!gi_none(r) && r.lo <= 0.0 && 0.0 <= r.hi) {
        float eps = max(uPixel * max(a, 1e-9), 1e-12);
        bool cont = r.d >= 2;
        float mid = 0.5 * (a + b);
        if (!refine && b - a > eps) {
          // 1 画素より広い: 二分し、手前の半分を先に調べる
          push = 2;
          pa0 = mid; pb0 = b;
          pa1 = a; pb1 = mid;
        } else {
          // 両端の値（連続なときだけ使う）
          float fa = 0.0;
          float fb = 0.0;
          if (cont) {
            for (int k = 0; k < uIters; k++) {
              if (k >= 2) break;
              float v = gRayV(o + (k == 0 ? a : b) * d);
              if (k == 0) fa = v; else fb = v;
            }
          }
          if (cont && (fa == 0.0 || fb == 0.0)) {
            // 区間の端で値がちょうど 0（1 画素の幅でも、詰める分でも）。その端をそのまま当たりにすると、当たる所が区間の幅だけずれて
            // 画素ごとに揺れる（floor(x) + floor(y) + floor(z) = 0 の立体の面がざらついた）。
            // 0 でない所から 0 の所への境目を二分で詰める。手前の端が 0 なら、その手前の同じ幅の区間を調べる
            bis = true;
            zeroEdge = true;
            blo = fa == 0.0 ? max(a - (b - a), t0) : a;
            bhi = fa == 0.0 ? a : b;
          } else if (cont && fa * fb < 0.0) {
            bis = true; blo = a; bhi = b; flo = fa;
          } else if (!refine) {
            // 1 画素の幅で、連続でない・符号が同じ: 1/16 画素まで詰めて決める
            push = 1;
            pa0 = a; pb0 = -b;
            budget = 64;
            epsR = eps / 16.0;
          } else if (b - a <= epsR) {
            // 詰めきった: 連続なら接するだけの零点、連続でなければ極・段差そのもの（描かない）
            if (cont) { tHit = mid; return 1; }
          } else {
            push = 2;
            pa0 = mid; pb0 = -b;
            pa1 = a; pb1 = -mid;
          }
        }
      }
    }
    if (bis) break;
    if (n + push > 32) {
      if (!refine) { tHit = a; return 2; }
      push = 0;
    }
    if (push >= 1) { sa[n] = pa0; sb[n] = pb0; n++; }
    if (push >= 2) { sa[n] = pa1; sb[n] = pb1; n++; }
  }
  if (bis) {
    // 二分法で詰める（32 ビットなので、区間が数えるほどの刻みになったら終える）
    for (int k = 0; k < uIters; k++) {
      if (k >= 40) break;
      float mid = 0.5 * (blo + bhi);
      if (mid <= blo || mid >= bhi) break;
      float fm = gRayV(o + mid * d);
      if (zeroEdge) {
        // 0 の所の手前の端を探す
        if (fm == 0.0) bhi = mid; else blo = mid;
        continue;
      }
      if (fm == 0.0) { blo = mid; bhi = mid; break; }
      if (flo * fm < 0.0) bhi = mid; else { blo = mid; flo = fm; }
    }
    tHit = zeroEdge ? bhi : 0.5 * (blo + bhi);
    return 1;
  }
  return n > 0 ? 2 : 0;
}

// 光線と箱の交わる範囲（交わらなければ t0 > t1）
vec2 gRayBox(vec3 o, vec3 d) {
  vec3 inv = 1.0 / d;
  vec3 a = (uBoxLo - o) * inv;
  vec3 b = (uBoxHi - o) * inv;
  vec3 lo = min(a, b);
  vec3 hi = max(a, b);
  return vec2(max(max(lo.x, lo.y), max(lo.z, 0.0)), min(min(hi.x, hi.y), hi.z));
}

varying vec3 vRayWorld;
// 当たった点の、裏側を見ているか（∇F がカメラと反対を向いている）・法線（カメラの空間）
bool gRayBack;
vec3 gRayN;
`
