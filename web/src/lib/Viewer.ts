// Three.js を使った3D表示。
// 「座標軸・目盛り・枠」と「各式の曲面」を管理する。
// 座標は数学と同じく z 軸が上向き。

import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js'
import { LineSegments2 } from 'three/examples/jsm/lines/LineSegments2.js'
import { LineSegmentsGeometry } from 'three/examples/jsm/lines/LineSegmentsGeometry.js'
import { CSS2DObject, CSS2DRenderer } from 'three/examples/jsm/renderers/CSS2DRenderer.js'
import { createGridUniforms, createSurfaceMaterial, type GridUniforms } from './surfaceMaterial'
import type { Domain, MeshData, PlotStyle } from './types'

/** 範囲 range をおよそ target 個に区切る「きりの良い」間隔（1, 2, 5 × 10^n） */
function niceStep(range: number, target = 8): number {
  const raw = range / target
  const mag = 10 ** Math.floor(Math.log10(raw))
  const norm = raw / mag
  return (norm < 1.5 ? 1 : norm < 3 ? 2 : norm < 7 ? 5 : 10) * mag
}

/** 目盛りの数字を見やすく整形（0.30000000000000004 → 0.3） */
function fmt(v: number): string {
  return String(Number(v.toPrecision(10)))
}

function label(text: string, className: string, pos: THREE.Vector3): CSS2DObject {
  const el = document.createElement('div')
  el.className = className
  el.textContent = text
  const obj = new CSS2DObject(el)
  obj.position.copy(pos)
  return obj
}

interface Theme {
  background: number
  box: number
  grid: number
}
const LIGHT: Theme = { background: 0xf7f7f4, box: 0x9a9aa2, grid: 0xd6d6d2 }
const DARK: Theme = { background: 0x1b1c20, box: 0x6a6b72, grid: 0x34353b }

/** 1つの式の描画物 */
type PlotObject =
  | {
      kind: 'surface'
      group: THREE.Group // 表面用と裏面用の2つのメッシュ
      grid: GridUniforms
      /** 媒介変数表示なら u, v の範囲の長さ（格子線を u, v に沿って引く） */
      uvSpan: { u: number; v: number } | null
    }
  | { kind: 'curve'; line: LineSegments2 }

export class Viewer {
  private renderer: THREE.WebGLRenderer
  private labelRenderer: CSS2DRenderer
  private scene = new THREE.Scene()
  private camera = new THREE.PerspectiveCamera(45, 1, 0.01, 1000)
  private controls: OrbitControls
  private frame = new THREE.Group() // 軸・目盛り・枠
  private plots = new Map<number, PlotObject>() // 式ごとの曲面・曲線
  private clipPlanes: THREE.Plane[] = []
  private domain: Domain | null = null
  /** 画面の横・奥・縦の軸の名前（4次元以上で w などを表示するとき変わる） */
  private axisNames = ['x', 'y', 'z']
  private theme: Theme
  /** 遠近感の強さ = tan(視野角 / 2)。小さいほど平行投影に近い */
  private perspective = Math.tan(THREE.MathUtils.degToRad(22.5))
  /** 画面の下側が何 px 隠れているか（スマホで式の一覧のシートが重なる分） */
  private bottomInset = 0
  private resizeObserver: ResizeObserver
  private stopped = false

  constructor(private container: HTMLElement) {
    // 描画の土台
    this.renderer = new THREE.WebGLRenderer({ antialias: true })
    this.renderer.setPixelRatio(window.devicePixelRatio)
    this.renderer.localClippingEnabled = true // 表示範囲の外を切り落とすため
    container.appendChild(this.renderer.domElement)

    // 文字（目盛りの数字）用の重ね合わせ
    this.labelRenderer = new CSS2DRenderer()
    this.labelRenderer.domElement.className = 'label-layer'
    container.appendChild(this.labelRenderer.domElement)

    this.theme = LIGHT
    this.scene.background = new THREE.Color(this.theme.background)

    // カメラとマウス操作（左ドラッグ: 回転 / 右ドラッグ: 移動 / ホイール: 拡大縮小）
    this.camera.up.set(0, 0, 1)
    this.controls = new OrbitControls(this.camera, this.renderer.domElement)
    this.controls.enableDamping = true

    // 照明: 空と地面からの柔らかい光 + カメラと一緒に動く光（どの向きから見ても明るい）
    this.scene.add(new THREE.HemisphereLight(0xffffff, 0x8888aa, 1.4))
    const head = new THREE.DirectionalLight(0xffffff, 2.2)
    head.position.set(3, 4, 0)
    head.target.position.set(0, 0, -10)
    this.camera.add(head, head.target)
    this.scene.add(this.camera, this.frame)

    this.resizeObserver = new ResizeObserver(() => this.resize())
    this.resizeObserver.observe(container)
    this.resize()
    this.loop()
  }

  dispose() {
    this.stopped = true
    this.resizeObserver.disconnect()
    this.controls.dispose()
    this.renderer.dispose()
    this.container.replaceChildren()
  }

  private resize() {
    const w = this.container.clientWidth
    const h = this.container.clientHeight
    if (w === 0 || h === 0) return
    this.camera.aspect = w / h
    this.renderer.setSize(w, h)
    this.labelRenderer.setSize(w, h)
    for (const p of this.plots.values()) {
      if (p.kind === 'curve') (p.line.material as LineMaterial).resolution.set(w, h)
    }
    this.applyViewOffset()
  }

  /**
   * 画面の下側 px 分が隠れていることを伝える。グラフが見えている部分の中央に来るよう、
   * 描画の中心を上にずらす。
   */
  setBottomInset(px: number) {
    this.bottomInset = Math.max(0, px)
    this.applyViewOffset()
  }

  private applyViewOffset() {
    const w = this.container.clientWidth
    const h = this.container.clientHeight
    if (w === 0 || h === 0) return
    if (this.bottomInset > 0) {
      this.camera.setViewOffset(w, h, 0, this.bottomInset / 2, w, h)
    } else {
      this.camera.clearViewOffset()
    }
    this.camera.updateProjectionMatrix()
  }

  private loop = () => {
    if (this.stopped) return
    requestAnimationFrame(this.loop)
    this.controls.update()
    this.updateClipRange()
    this.renderer.render(this.scene, this.camera)
    this.labelRenderer.render(this.scene, this.camera)
  }

  /** 表示範囲の対角線の長さ */
  private sceneSize(): number {
    const d = this.domain
    return d ? Math.hypot(d.xmax - d.xmin, d.ymax - d.ymin, d.zmax - d.zmin) : 10
  }

  /** カメラが描く奥行きの範囲を、今の距離に合わせる（平行投影に近いとカメラがとても遠くなるため） */
  private updateClipRange() {
    const dist = this.camera.position.distanceTo(this.controls.target)
    const size = this.sceneSize()
    const near = Math.max(dist - 2 * size, dist * 1e-3)
    const far = dist + 4 * size
    if (near !== this.camera.near || far !== this.camera.far) {
      this.camera.near = near
      this.camera.far = far
      this.camera.updateProjectionMatrix()
    }
  }

  /**
   * 遠近感の強さを変える（0: 平行投影 〜 1: 視野角 90° の強い透視投影）。
   * 見た目の大きさが変わらないよう、視野角を変えると同時にカメラを前後に動かす。
   */
  setPerspective(strength: number) {
    const tanNew = Math.max(strength, 0.004) // ちょうど 0 は計算できないので、ほぼ平行投影で代用
    const target = this.controls.target
    const offset = this.camera.position.clone().sub(target)
    const halfHeight = offset.length() * this.perspective // 注視点の位置で見えている高さの半分
    offset.setLength(halfHeight / tanNew)
    this.camera.position.copy(target).add(offset)
    this.perspective = tanNew
    this.camera.fov = THREE.MathUtils.radToDeg(2 * Math.atan(tanNew))
    this.camera.updateProjectionMatrix()
  }

  /** 軸の名前を変える */
  setAxisNames(names: string[]) {
    this.axisNames = [...names]
    if (this.domain) this.buildFrame(this.domain)
  }

  /** 背景の明暗を切り替える */
  setDark(dark: boolean) {
    this.theme = dark ? DARK : LIGHT
    ;(this.scene.background as THREE.Color).set(this.theme.background)
    if (this.domain) this.buildFrame(this.domain)
  }

  /** 表示範囲を設定し、軸・目盛り・枠を作り直す */
  setDomain(d: Domain, resetCamera = false) {
    const first = this.domain === null
    this.domain = { ...d }

    // 範囲の外を切り落とす6枚の平面
    this.clipPlanes = [
      new THREE.Plane(new THREE.Vector3(1, 0, 0), -d.xmin),
      new THREE.Plane(new THREE.Vector3(-1, 0, 0), d.xmax),
      new THREE.Plane(new THREE.Vector3(0, 1, 0), -d.ymin),
      new THREE.Plane(new THREE.Vector3(0, -1, 0), d.ymax),
      new THREE.Plane(new THREE.Vector3(0, 0, 1), -d.zmin),
      new THREE.Plane(new THREE.Vector3(0, 0, -1), d.zmax),
    ]
    for (const p of this.plots.values()) {
      const obj = p.kind === 'surface' ? p.group : p.line
      obj.traverse((o) => {
        if (o instanceof THREE.Mesh) (o.material as THREE.Material).clippingPlanes = this.clipPlanes
      })
    }

    this.buildFrame(d)
    if (first || resetCamera) this.resetCamera()
  }

  /** カメラを斜め上からの見やすい位置に戻す */
  resetCamera() {
    const d = this.domain
    if (!d) return
    const center = new THREE.Vector3((d.xmin + d.xmax) / 2, (d.ymin + d.ymax) / 2, (d.zmin + d.zmax) / 2)
    const size = this.sceneSize()
    // 縦長の画面（スマホ）では横幅に収まるよう、その分だけ遠くから見る
    const dist = (size * 0.56) / this.perspective / Math.min(1, this.camera.aspect)
    this.controls.target.copy(center)
    this.camera.position.copy(center).add(new THREE.Vector3(1.1, -1.5, 0.9).normalize().multiplyScalar(dist))
    this.updateClipRange()
  }

  private buildFrame(d: Domain) {
    // 古いものを片付ける
    this.frame.traverse((o) => {
      if (o instanceof THREE.LineSegments) {
        o.geometry.dispose()
        ;(o.material as THREE.Material).dispose()
      }
    })
    this.frame.clear()

    const t = this.theme
    const lo = new THREE.Vector3(d.xmin, d.ymin, d.zmin)
    const hi = new THREE.Vector3(d.xmax, d.ymax, d.zmax)

    // 外枠
    const box = new THREE.Box3Helper(new THREE.Box3(lo, hi), t.box)
    this.frame.add(box)

    // 床（z = 0、範囲外なら一番下）の方眼
    const zFloor = d.zmin <= 0 && 0 <= d.zmax ? 0 : d.zmin
    const step = niceStep(Math.max(d.xmax - d.xmin, d.ymax - d.ymin))
    const pts: number[] = []
    for (let x = Math.ceil(d.xmin / step) * step; x <= d.xmax + 1e-9; x += step) {
      pts.push(x, d.ymin, zFloor, x, d.ymax, zFloor)
    }
    for (let y = Math.ceil(d.ymin / step) * step; y <= d.ymax + 1e-9; y += step) {
      pts.push(d.xmin, y, zFloor, d.xmax, y, zFloor)
    }
    const gridGeo = new THREE.BufferGeometry()
    gridGeo.setAttribute('position', new THREE.Float32BufferAttribute(pts, 3))
    this.frame.add(new THREE.LineSegments(gridGeo, new THREE.LineBasicMaterial({ color: t.grid })))

    // 座標軸（原点を通る。原点が範囲外なら範囲の端に寄せる）
    const clamp = (v: number, a: number, b: number) => Math.min(b, Math.max(a, v))
    const o = new THREE.Vector3(clamp(0, d.xmin, d.xmax), clamp(0, d.ymin, d.ymax), clamp(0, d.zmin, d.zmax))
    const axes: [THREE.Vector3, THREE.Vector3, number, string][] = [
      [new THREE.Vector3(d.xmin, o.y, o.z), new THREE.Vector3(d.xmax, o.y, o.z), 0xd9534f, this.axisNames[0]],
      [new THREE.Vector3(o.x, d.ymin, o.z), new THREE.Vector3(o.x, d.ymax, o.z), 0x4caf50, this.axisNames[1]],
      [new THREE.Vector3(o.x, o.y, d.zmin), new THREE.Vector3(o.x, o.y, d.zmax), 0x3f7fd9, this.axisNames[2]],
    ]
    for (const [a, b, color, name] of axes) {
      const g = new THREE.BufferGeometry().setFromPoints([a, b])
      this.frame.add(new THREE.LineSegments(g, new THREE.LineBasicMaterial({ color })))
      const end = b.clone().add(b.clone().sub(a).normalize().multiplyScalar(step * 0.4))
      this.frame.add(label(name, 'axis-name', end))
    }

    // 目盛りの数字（x, y は床の手前の辺、z は左奥の縦の辺）
    const ticks = (min: number, max: number, s: number) => {
      const out: number[] = []
      for (let v = Math.ceil(min / s) * s; v <= max + 1e-9; v += s) out.push(v)
      return out
    }
    const pad = step * 0.35
    for (const x of ticks(d.xmin, d.xmax, step)) {
      this.frame.add(label(fmt(x), 'tick', new THREE.Vector3(x, d.ymin - pad, zFloor)))
    }
    for (const y of ticks(d.ymin, d.ymax, step)) {
      this.frame.add(label(fmt(y), 'tick', new THREE.Vector3(d.xmax + pad, y, zFloor)))
    }
    for (const z of ticks(d.zmin, d.zmax, niceStep(d.zmax - d.zmin, 6))) {
      this.frame.add(label(fmt(z), 'tick', new THREE.Vector3(d.xmin - pad, d.ymin - pad, z)))
    }
  }

  /** 式 id の曲面・曲線を設定（作り直し） */
  setPlot(id: number, mesh: MeshData, style: PlotStyle) {
    this.removePlot(id)
    if (mesh.curve) {
      this.setCurve(id, mesh.positions)
    } else {
      this.setSurface(id, mesh)
    }
    this.setStyle(id, style)
  }

  private setSurface(id: number, mesh: MeshData) {
    const geo = new THREE.BufferGeometry()
    const pos = new THREE.BufferAttribute(mesh.positions, 3)
    geo.setAttribute('position', pos)
    geo.setAttribute('normal', new THREE.BufferAttribute(mesh.normals, 3))
    geo.setAttribute('gridCoord', mesh.gridCoords.length > 0 ? new THREE.BufferAttribute(mesh.gridCoords, 3) : pos)
    geo.setIndex(new THREE.BufferAttribute(mesh.indices, 1))

    // 表面と裏面で色の明るさを変えて、どちら側を見ているか分かるようにする
    const grid = createGridUniforms()
    const front = createSurfaceMaterial(THREE.FrontSide, grid, this.clipPlanes)
    const back = createSurfaceMaterial(THREE.BackSide, grid, this.clipPlanes)
    const group = new THREE.Group()
    group.add(new THREE.Mesh(geo, front), new THREE.Mesh(geo, back))
    const uvSpan = mesh.params.length === 2 ? mesh.paramSpan : null
    this.plots.set(id, { kind: 'surface', group, grid, uvSpan })
    this.scene.add(group)
  }

  /** 曲線。点の列を、隣り合う2点ずつの線分にする（NaN の所は線を切る） */
  private setCurve(id: number, pts: Float32Array) {
    const seg: number[] = []
    const ok = (k: number) => Number.isFinite(pts[k]) && Number.isFinite(pts[k + 1]) && Number.isFinite(pts[k + 2])
    for (let k = 0; k + 5 < pts.length; k += 3) {
      if (ok(k) && ok(k + 3)) {
        seg.push(pts[k], pts[k + 1], pts[k + 2], pts[k + 3], pts[k + 4], pts[k + 5])
      }
    }
    const geo = new LineSegmentsGeometry()
    if (seg.length > 0) geo.setPositions(seg)
    const mat = new LineMaterial({ linewidth: 3, clippingPlanes: this.clipPlanes })
    mat.resolution.set(this.container.clientWidth, this.container.clientHeight)
    const line = new LineSegments2(geo, mat)
    this.plots.set(id, { kind: 'curve', line })
    this.scene.add(line)
  }

  /** 色・表示/非表示・格子線などの見た目だけを変える（計算し直さない） */
  setStyle(id: number, style: PlotStyle) {
    const p = this.plots.get(id)
    if (!p) return
    const c = new THREE.Color(style.color)

    if (p.kind === 'curve') {
      p.line.visible = style.visible
      const m = p.line.material as LineMaterial
      m.color.copy(c)
      m.linewidth = style.lineWidth
      return
    }

    p.group.visible = style.visible
    const [front, back] = p.group.children as THREE.Mesh[]
    ;(front.material as THREE.MeshStandardMaterial).color.copy(c)
    ;(back.material as THREE.MeshStandardMaterial).color.copy(c).multiplyScalar(0.55)

    // 面の不透明度。1 未満なら半透明として描き、奥の面や線も透けて見えるようにする
    const opacity = Math.min(1, Math.max(0, style.opacity))
    for (const mesh of [front, back]) {
      const m = mesh.material as THREE.MeshStandardMaterial
      const transparent = opacity < 1
      if (m.transparent !== transparent) {
        m.transparent = transparent
        m.depthWrite = !transparent
        m.needsUpdate = true // シェーダーの作り直しが必要
      }
      // 面が薄いときは、線が暗く沈まないよう少し自分で光らせる
      m.emissive.copy(c).multiplyScalar(0.45 * (1 - opacity))
    }
    p.grid.uFillOpacity.value = opacity

    // 格子線: 面が濃いときは曲面の色を暗くした色、面が薄いときは曲面の色そのもので描く。
    // 間隔が 0 以下・数値でないものは描かない
    const g = style.grid
    const sp = (v: number) => (g.enabled && Number.isFinite(v) && v > 0 ? v : 0)
    if (p.uvSpan) {
      // 媒介変数表示: u, v の範囲をそれぞれ g.u, g.v 等分する線
      const div = (span: number, n: number) => (n > 0 ? span / n : 0)
      p.grid.uGridSpacing.value.set(sp(div(p.uvSpan.u, g.u)), sp(div(p.uvSpan.v, g.v)), sp(g.z))
    } else {
      p.grid.uGridSpacing.value.set(sp(g.x), sp(g.y), sp(g.z))
    }
    p.grid.uGridColor.value.copy(c).multiplyScalar(0.3 + 0.7 * (1 - opacity))
    p.grid.uGridStrength.value = 0.75 + 0.25 * (1 - opacity)
    p.grid.uGridWidth.value = g.width
  }

  removePlot(id: number) {
    const p = this.plots.get(id)
    if (!p) return
    if (p.kind === 'curve') {
      p.line.geometry.dispose()
      ;(p.line.material as THREE.Material).dispose()
      this.scene.remove(p.line)
    } else {
      const [front, back] = p.group.children as THREE.Mesh[]
      front.geometry.dispose()
      ;(front.material as THREE.Material).dispose()
      ;(back.material as THREE.Material).dispose()
      this.scene.remove(p.group)
    }
    this.plots.delete(id)
  }
}
