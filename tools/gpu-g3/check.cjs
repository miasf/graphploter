// 計画 12 章の G3: 命令列から作った GLSL を、ヘッドレスの Chromium（ソフトウェアの WebGL2）で計算し、Rust の結果と比べる。
//
//   cd web && npm run test:gpu            （= node ../tools/gpu-g3/check.cjs）
//   node tools/gpu-g3/check.cjs [fixture.json]   （fixture を省くと cargo run --release --example glsl_fixture で作る）
//
// 確かめること（式ごと。点・箱は 1000 個ずつ）:
// - 値 F: Rust の値が有限な点で、相対誤差（|GLSL − Rust| / max(1, |Rust|)）が 1e-4 以下
// - 二重数 Fd の勾配: Rust の記号微分の勾配が有限な点で、相対誤差 1e-3 以下
// - 区間 Fi: 箱の中の点の Rust の値が、全部 GLSL の区間に入る。飾りが Def 以上なら、箱の中に定義されない点が無い。
//   区間が Rust より広がった分（32 ビットで広げる分。広いほど G5 で二分が深くなる）も出す
// 「精度の確かめに使う」式（strict）で、区間の見落としが 1 つでもあるか、値・勾配の外れが 1% を超えると失敗にする。
// CI では回さない（GPU・ブラウザが要る）。
const { execFileSync } = require('child_process')
const fs = require('fs')
const os = require('os')
const path = require('path')
const { chromium } = require('playwright')

const root = path.resolve(__dirname, '../..')
let fixture = process.argv[2]
if (!fixture) {
  fixture = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'glsl-')), 'fixture.json')
  execFileSync('cargo', ['run', '--release', '-q', '--example', 'glsl_fixture', '--', fixture], { cwd: path.join(root, 'core'), stdio: 'inherit' })
}
const cases = JSON.parse(fs.readFileSync(fixture, 'utf8'))

/** ページの中で 1 式を計算する（点の値・勾配、箱の区間） */
function runCase(c) {
  const n = c.points.length
  const canvas = document.createElement('canvas')
  canvas.width = n
  canvas.height = 2
  const gl = canvas.getContext('webgl2')
  if (!gl) return { error: 'WebGL2 が使えない' }
  if (!gl.getExtension('EXT_color_buffer_float')) return { error: 'EXT_color_buffer_float が無い' }
  const vs = `#version 300 es
in vec2 aPos; void main() { gl_Position = vec4(aPos, 0.0, 1.0); }`
  const fs = `#version 300 es
precision highp float; precision highp int; precision highp sampler2D;
uniform sampler2D uIn;
out vec4 o;
${c.glsl}
void main() {
  ivec2 q = ivec2(gl_FragCoord.xy);
  if (q.y == 0) {
    vec4 t = texelFetch(uIn, ivec2(q.x, 0), 0);
    float p[3]; p[0] = t.x; p[1] = t.y; p[2] = t.z;
    vec4 d[3]; d[0] = vec4(t.x, 1.0, 0.0, 0.0); d[1] = vec4(t.y, 0.0, 1.0, 0.0); d[2] = vec4(t.z, 0.0, 0.0, 1.0);
    vec4 g = Fd(d);
    o = vec4(F(p), g.yzw);
  } else {
    vec4 lo = texelFetch(uIn, ivec2(q.x, 1), 0);
    vec4 hi = texelFetch(uIn, ivec2(q.x, 2), 0);
    vec2 b[3]; b[0] = vec2(lo.x, hi.x); b[1] = vec2(lo.y, hi.y); b[2] = vec2(lo.z, hi.z);
    Div r = Fi(b);
    o = vec4(r.lo, r.hi, float(r.d), 0.0);
  }
}`
  const sh = (type, src) => {
    const s = gl.createShader(type)
    gl.shaderSource(s, src)
    gl.compileShader(s)
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s))
    return s
  }
  const prog = gl.createProgram()
  try {
    gl.attachShader(prog, sh(gl.VERTEX_SHADER, vs))
    gl.attachShader(prog, sh(gl.FRAGMENT_SHADER, fs))
  } catch (e) {
    return { error: String(e.message || e) }
  }
  gl.linkProgram(prog)
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) return { error: gl.getProgramInfoLog(prog) }
  gl.useProgram(prog)
  // 入力: 1 行目に点、2・3 行目に箱の下・上
  const data = new Float32Array(n * 3 * 4)
  for (let i = 0; i < n; i++) {
    const p = c.points[i]
    const b = c.boxes[i]
    for (let k = 0; k < 3; k++) {
      data[i * 4 + k] = p[k]
      data[(n + i) * 4 + k] = b[k]
      data[(2 * n + i) * 4 + k] = b[3 + k]
    }
  }
  const tex = gl.createTexture()
  gl.bindTexture(gl.TEXTURE_2D, tex)
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA32F, n, 3, 0, gl.RGBA, gl.FLOAT, data)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST)
  gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST)
  gl.uniform1i(gl.getUniformLocation(prog, 'uIn'), 0)
  if (c.params.length) gl.uniform1fv(gl.getUniformLocation(prog, 'uParam'), new Float32Array(c.params))
  const outTex = gl.createTexture()
  gl.bindTexture(gl.TEXTURE_2D, outTex)
  gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA32F, n, 2, 0, gl.RGBA, gl.FLOAT, null)
  const fb = gl.createFramebuffer()
  gl.bindFramebuffer(gl.FRAMEBUFFER, fb)
  gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, outTex, 0)
  gl.activeTexture(gl.TEXTURE0)
  gl.bindTexture(gl.TEXTURE_2D, tex)
  const buf = gl.createBuffer()
  gl.bindBuffer(gl.ARRAY_BUFFER, buf)
  gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 3, -1, -1, 3]), gl.STATIC_DRAW)
  const loc = gl.getAttribLocation(prog, 'aPos')
  gl.enableVertexAttribArray(loc)
  gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0)
  gl.viewport(0, 0, n, 2)
  gl.drawArrays(gl.TRIANGLES, 0, 3)
  const out = new Float32Array(n * 2 * 4)
  gl.readPixels(0, 0, n, 2, gl.RGBA, gl.FLOAT, out)
  // NaN・無限大は JSON にできないので文字にする
  const enc = (v) => (Number.isFinite(v) ? v : String(v))
  return { out: Array.from(out, enc) }
}

const dec = (v) => (typeof v === 'string' ? Number(v) : v)
const rel = (g, r) => Math.abs(g - r) / Math.max(1, Math.abs(r))

;(async () => {
  const browser = await chromium.launch()
  const page = await browser.newPage()
  await page.setContent('<!doctype html><title>g3</title>')
  const rows = []
  let failed = 0
  for (const c of cases) {
    const res = await page.evaluate(runCase, c)
    if (res.error) {
      console.log(`✗ ${c.src}: ${res.error}`)
      failed++
      continue
    }
    const o = res.out.map(dec)
    const n = c.points.length
    let vN = 0, vBad = 0, vMax = 0
    let gN = 0, gBad = 0, gMax = 0
    let iMiss = 0, iDef = 0, iSame = 0
    const ratios = []
    for (let i = 0; i < n; i++) {
      const r = c.values[i]
      const g = o[i * 4]
      if (r !== null) {
        vN++
        const e = Number.isFinite(g) ? rel(g, r) : Infinity
        vMax = Math.max(vMax, e)
        if (!(e <= 1e-4)) vBad++
      }
      const rg = c.grads[i]
      if (r !== null && rg.every((v) => v !== null)) {
        gN++
        let e = 0
        for (let k = 0; k < 3; k++) {
          const gv = o[i * 4 + 1 + k]
          e = Math.max(e, Number.isFinite(gv) ? rel(gv, rg[k]) : Infinity)
        }
        gMax = Math.max(gMax, e)
        if (!(e <= 1e-3)) gBad++
      }
      const [mn, mx, undef, rdec, rlo, rhi] = c.boxStats[i]
      const b = (n + i) * 4
      const [lo, hi, d] = [o[b], o[b + 1], o[b + 2]]
      if (mn !== null && !(lo <= mn && mx <= hi)) iMiss++
      if (d >= 1 && undef) iDef++
      if (d === rdec) iSame++
      // 区間が Rust より広がった分（値の大きさ + 1 で割る）。両方とも有界な箱だけ
      if (rlo !== null && rhi !== null && Number.isFinite(lo) && Number.isFinite(hi)) {
        ratios.push((hi - lo - (rhi - rlo)) / (Math.max(Math.abs(rlo), Math.abs(rhi)) + 1))
      }
    }
    const vFrac = vN ? vBad / vN : 0
    const gFrac = gN ? gBad / gN : 0
    const ok = !c.strict || (iMiss === 0 && iDef === 0 && vFrac <= 0.01 && gFrac <= 0.01)
    if (!ok) failed++
    ratios.sort((a, b) => a - b)
    const wMed = ratios.length ? ratios[ratios.length >> 1] : NaN
    const wMax = ratios.length ? ratios[ratios.length - 1] : NaN
    rows.push({ src: c.src, strict: c.strict, ok, vMax, vFrac, gMax, gFrac, iMiss, iDef, same: iSame / n, wMed, wMax })
  }
  await browser.close()
  const pct = (x) => (100 * x).toFixed(2)
  const fx = (x) => (Number.isFinite(x) ? x.toExponential(1) : '-')
  const sci = (x) => (Number.isFinite(x) ? x.toExponential(1) : '∞')
  console.log('| 式 | 値の相対誤差の最大 | 値の外れ % | 勾配の相対誤差の最大 | 勾配の外れ % | 区間の見落とし | 飾りの誤り | 飾りが Rust と同じ % | 区間が広がった分 /（\|値\| + 1）（中央値・最大） |')
  console.log('|---|---|---|---|---|---|---|---|---|')
  for (const r of rows) {
    console.log(`| ${r.ok ? '' : '✗ '}\`${r.src}\`${r.strict ? '' : '（参考）'} | ${sci(r.vMax)} | ${pct(r.vFrac)} | ${sci(r.gMax)} | ${pct(r.gFrac)} | ${r.iMiss} | ${r.iDef} | ${pct(r.same)} | ${fx(r.wMed)}・${fx(r.wMax)} |`)
  }
  console.log(failed ? `\n${failed} 式が失敗` : `\n全部の式で合格（${rows.length} 式）`)
  process.exit(failed ? 1 : 0)
})()
