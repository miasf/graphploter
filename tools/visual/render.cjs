// 見た目の比較: dump_revs.sh で書き出したメッシュを、全部のコミットで同じ視点・同じ光で描いて並べる。
//
//   node tools/visual/render.cjs <dump_revs.sh の出力先> <コミット>...   （コミットは並べたい順。短いハッシュ or WORK）
//
// <出力先>/report/（環境変数 VISUAL_REPORT で名前を変えられる）に、式ごとの画像（横にコミット、縦に視点 4 つ）と index.html（変化の大きい順の一覧）ができる。
// 環境変数 VISUAL_ONLY（名前の一部）で式を絞り、VISUAL_VIEW（"カメラの x,y,z,見る点の x,y,z"）で視点を 1 つに決められる。
// 「変化」は、隣のコミットどうしで色が大きく違う画素の割合（%）。描画の計算が変わった式を見つけるのに使う。
// 見つけた式は画像を見て、良くなったか悪くなったかを人が判断する（変化の大きさだけでは決めない）。
//
// 描画には web/node_modules の three.js と、Playwright の Chromium を使う。

const fs = require('fs')
const path = require('path')
const http = require('http')
const { chromium } = require('playwright')

const [dir, ...revs] = process.argv.slice(2)
if (!dir || revs.length === 0) {
  console.error('使い方: node tools/visual/render.cjs <出力先> <コミット>...')
  process.exit(1)
}
const root = path.resolve(__dirname, '../..')
const threePath = path.join(root, 'web/node_modules/three/build/three.module.js')
const threeCore = path.join(root, 'web/node_modules/three/build/three.core.js')
const reportDir = path.join(dir, process.env.VISUAL_REPORT || 'report') // 出力のフォルダ名（環境変数 VISUAL_REPORT で変えられる）
fs.mkdirSync(reportDir, { recursive: true })

/** 式の一覧（名前・壁・式）。見出しの行（# で始まる）は区切りとして残す */
const cases = []
for (const line of fs.readFileSync(path.join(root, 'tools/visual/cases.tsv'), 'utf8').split('\n')) {
  if (!line.trim()) continue
  if (line.startsWith('#')) {
    cases.push({ heading: line.slice(1).trim() })
    continue
  }
  const [name, walls, src] = line.split('\t')
  cases.push({ name, walls: walls === '1', src })
}
cases.shift() // 先頭の説明の行

/** コミットごとの meta.tsv（名前 → 三角形の数・時間・説明） */
const meta = {}
const revTitle = {}
for (const r of revs) {
  meta[r] = {}
  const f = path.join(dir, r, 'meta.tsv')
  if (!fs.existsSync(f)) throw new Error(`${f} が無い（dump_revs.sh を先に回す）`)
  for (const line of fs.readFileSync(f, 'utf8').trim().split('\n')) {
    const [name, tris, ms, kind] = line.split('\t')
    meta[r][name] = { tris: +tris, ms: +ms, kind }
  }
  revTitle[r] = fs.existsSync(path.join(dir, r, 'rev.txt')) ? fs.readFileSync(path.join(dir, r, 'rev.txt'), 'utf8').trim() : r
}

// three.js とメッシュのファイルを渡すだけの小さなサーバー
const server = http.createServer((req, res) => {
  const url = decodeURIComponent(req.url.split('?')[0])
  let file = null
  if (url === '/three.module.js') file = threePath
  else if (url === '/three.core.js') file = threeCore
  else if (url.startsWith('/mesh/')) file = path.join(dir, url.slice(6))
  if (url === '/') {
    res.setHeader('content-type', 'text/html')
    res.end('<!doctype html><body style="margin:0;background:#fff"></body>')
    return
  }
  if (!file || !fs.existsSync(file)) {
    res.statusCode = 404
    res.end()
    return
  }
  res.setHeader('content-type', file.endsWith('.js') ? 'text/javascript' : 'application/octet-stream')
  fs.createReadStream(file).pipe(res)
})

/** ページの中で動く描画（式 1 つ分の全コミット・全視点を描き、画像と変化の割合を返す） */
async function setupPage(page) {
  await page.addScriptTag({
    type: 'module',
    content: `
      import * as THREE from '/three.module.js'
      const W = 380, H = 300
      const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true })
      renderer.setSize(W, H)
      renderer.setPixelRatio(1)
      // 表示範囲（アプリの既定）。z は 6 で切る
      const LO = [-10, -10, -6], HI = [10, 10, 6]
      // 視点 [カメラの位置, 見る点]: 斜め上（アプリの最初の視点に近い）・真横（段差・壁を見る）・斜め下・
      // 原点の近くに寄ったもの（細かい崩れを見る）
      const VIEWS = [
        [[24, -30, 22], [0, 0, 0]],
        [[0, -40, 2], [0, 0, 0]],
        [[-26, 24, -20], [0, 0, 0]],
        [[7, -9, 7], [0.5, 0.5, 0]],
      ]
      // 環境変数 VISUAL_VIEW（"カメラの x,y,z,見る点の x,y,z"）があれば、その視点 1 つだけにする
      if (window.customView) VIEWS.splice(0, VIEWS.length, window.customView)
      function frame() {
        const g = new THREE.BufferGeometry()
        const c = []
        for (let a = 0; a < 3; a++) {
          const b = (a + 1) % 3, d = (a + 2) % 3
          for (const sb of [LO[b], HI[b]]) for (const sd of [LO[d], HI[d]]) {
            const p = [0, 0, 0], q = [0, 0, 0]
            p[a] = LO[a]; q[a] = HI[a]; p[b] = q[b] = sb; p[d] = q[d] = sd
            c.push(...p, ...q)
          }
        }
        g.setAttribute('position', new THREE.Float32BufferAttribute(c, 3))
        return new THREE.LineSegments(g, new THREE.LineBasicMaterial({ color: 0x999999 }))
      }
      async function load(url) {
        const r = await fetch(url)
        if (!r.ok) return null
        const buf = await r.arrayBuffer()
        const dv = new DataView(buf)
        const nv = dv.getUint32(0, true), ni = dv.getUint32(4, true)
        const pos = new Float32Array(buf, 8, nv * 3)
        const idx = new Uint32Array(buf, 8 + nv * 12, ni)
        // 法線は描画側でそろえて作る（コミットごとの法線の違いではなく、形の違いを見る）。
        // 三角形ごとの平らな陰影にする（細かい折れ・破片・裏返りが見えるように）
        const flat = new Float32Array(ni * 3)
        let k = 0
        for (let t = 0; t < ni; t += 3) {
          for (let j = 0; j < 3; j++) {
            const v = idx[t + j] * 3
            flat[k++] = pos[v]; flat[k++] = pos[v + 1]; flat[k++] = pos[v + 2]
          }
        }
        const g = new THREE.BufferGeometry()
        g.setAttribute('position', new THREE.BufferAttribute(flat, 3))
        g.computeVertexNormals()
        return g
      }
      // アプリと同じく、表示範囲の箱の外は切り落とす（エンジンは箱の外まで面を出すことがある）
      renderer.localClippingEnabled = true
      const clip = [0, 1, 2].flatMap((a) => {
        const n = [0, 0, 0]
        n[a] = 1
        const m = [0, 0, 0]
        m[a] = -1
        return [new THREE.Plane(new THREE.Vector3(...n), -LO[a]), new THREE.Plane(new THREE.Vector3(...m), HI[a])]
      })
      const front = new THREE.MeshLambertMaterial({ color: 0x1a9ea0, side: THREE.FrontSide, clippingPlanes: clip })
      const back = new THREE.MeshLambertMaterial({ color: 0xd88a3a, side: THREE.BackSide, clippingPlanes: clip })
      window.drawCase = async (name, revs) => {
        const out = []
        for (const rev of revs) {
          const g = await load('/mesh/' + rev + '/' + encodeURIComponent(name) + '.bin')
          const imgs = []
          for (const v of VIEWS) {
            const scene = new THREE.Scene()
            scene.background = new THREE.Color(0xffffff)
            scene.add(frame())
            if (g) {
              scene.add(new THREE.Mesh(g, front))
              scene.add(new THREE.Mesh(g, back))
            }
            scene.add(new THREE.HemisphereLight(0xffffff, 0x555555, 1.6))
            const light = new THREE.DirectionalLight(0xffffff, 1.6)
            light.position.set(0.4, -0.5, 1)
            scene.add(light)
            const cam = new THREE.PerspectiveCamera(30, W / H, 1, 500)
            cam.up.set(0, 0, 1)
            cam.position.set(...v[0])
            cam.lookAt(...v[1])
            renderer.render(scene, cam)
            const c = document.createElement('canvas')
            c.width = W; c.height = H
            c.getContext('2d').drawImage(renderer.domElement, 0, 0)
            imgs.push(c)
          }
          out.push({ rev, imgs, empty: !g })
          g?.dispose()
        }
        // 隣のコミットとの変化: 色の差が大きい画素の割合（全部の視点の平均）
        const pixels = (c) => c.getContext('2d').getImageData(0, 0, W, H).data
        const changes = out.map((o, i) => {
          if (i === 0) return null
          let diff = 0
          for (let v = 0; v < VIEWS.length; v++) {
            const a = pixels(out[i - 1].imgs[v]), b = pixels(o.imgs[v])
            for (let p = 0; p < a.length; p += 4) {
              if (Math.abs(a[p] - b[p]) + Math.abs(a[p + 1] - b[p + 1]) + Math.abs(a[p + 2] - b[p + 2]) > 60) diff++
            }
          }
          return (100 * diff) / (W * H * VIEWS.length)
        })
        // 横にコミット、縦に視点を並べた 1 枚の画像
        const sheet = document.createElement('canvas')
        sheet.width = W * out.length
        sheet.height = H * VIEWS.length
        const ctx = sheet.getContext('2d')
        out.forEach((o, i) => o.imgs.forEach((c, v) => ctx.drawImage(c, i * W, v * H)))
        ctx.strokeStyle = '#ccc'
        for (let i = 1; i < out.length; i++) {
          ctx.beginPath(); ctx.moveTo(i * W + 0.5, 0); ctx.lineTo(i * W + 0.5, sheet.height); ctx.stroke()
        }
        return { png: sheet.toDataURL('image/png'), changes }
      }
      window.ready = true
    `,
  })
  await page.waitForFunction(() => window.ready)
}

const esc = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;')

;(async () => {
  await new Promise((r) => server.listen(0, r))
  const port = server.address().port
  const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] })
  const page = await browser.newPage()
  await page.goto(`http://localhost:${port}/`)
  if (process.env.VISUAL_VIEW) {
    const v = process.env.VISUAL_VIEW.split(',').map(Number)
    await page.evaluate((v) => (window.customView = [v.slice(0, 3), v.slice(3, 6)]), v)
  }
  await setupPage(page)
  const only = process.env.VISUAL_ONLY
  const results = []
  for (const c of cases) {
    if (c.heading || (only && !c.name.includes(only))) continue
    const { png, changes } = await page.evaluate(([n, r]) => window.drawCase(n, r), [c.name, revs])
    fs.writeFileSync(path.join(reportDir, `${c.name}.png`), Buffer.from(png.split(',')[1], 'base64'))
    results.push({ ...c, changes })
    process.stderr.write(`${c.name} `)
  }
  process.stderr.write('\n')
  await browser.close()
  server.close()

  // 一覧: 変化の大きい順（一番大きい隣どうしの変化）
  const maxChange = (r) => Math.max(0, ...r.changes.filter((x) => x !== null))
  results.sort((a, b) => maxChange(b) - maxChange(a))
  const cell = (rev, name) => {
    const m = meta[rev][name]
    if (!m) return '<td>—</td>'
    return `<td>${m.tris.toLocaleString()}<br>${m.ms} ms${m.kind !== 'z = f(x, y)' && m.kind !== '陰関数' ? `<br><small>${esc(m.kind)}</small>` : ''}</td>`
  }
  const html = `<!doctype html><meta charset="utf-8"><title>見た目の比較</title>
<style>
body{font-family:system-ui,sans-serif;margin:16px;color:#222}
table{border-collapse:collapse;font-size:12px}td,th{border:1px solid #ddd;padding:2px 6px;text-align:right;vertical-align:top}
th{background:#f4f4f4}.case{margin:24px 0}.case img{max-width:100%;border:1px solid #ccc}
.hot{background:#ffe0d0}h2{font-size:16px;margin:4px 0}code{background:#f4f4f4;padding:1px 4px}
</style>
<h1>見た目の比較</h1>
<p>横: コミット（左から古い順）。縦: 視点（斜め上・真横・斜め下・原点の近く）。青緑が表、橙が裏。三角形ごとの平らな陰影。</p>
<ol>${revs.map((r) => `<li><code>${esc(revTitle[r])}</code></li>`).join('')}</ol>
<p>「変化」は隣のコミットとの、色が大きく違う画素の割合（%）。5% 以上を色付けしている。変化の大きい順に並べている。</p>
${results
  .map(
    (r) => `<div class="case"><h2>${esc(r.name)}: <code>${esc(r.src)}</code>${r.walls ? '（壁つき）' : ''}</h2>
<table><tr><th></th>${revs.map((v) => `<th>${esc(v)}</th>`).join('')}</tr>
<tr><th>変化</th>${r.changes.map((x) => (x === null ? '<td></td>' : `<td class="${x >= 5 ? 'hot' : ''}">${x.toFixed(1)}</td>`)).join('')}</tr>
<tr><th>三角形・時間</th>${revs.map((v) => cell(v, r.name)).join('')}</tr></table>
<img src="${esc(r.name)}.png" loading="lazy"></div>`,
  )
  .join('\n')}`
  fs.writeFileSync(path.join(reportDir, 'index.html'), html)
  const summary = results.map((r) => `${r.name}\t${r.changes.map((x) => (x === null ? '-' : x.toFixed(1))).join('\t')}\t${r.src}`)
  fs.writeFileSync(path.join(reportDir, 'changes.tsv'), `名前\t${revs.join('\t')}\t式\n${summary.join('\n')}\n`)
  console.log(`${path.join(reportDir, 'index.html')} に書いた`)
})()
