// Three.js を使った3D表示。
// 「座標軸・目盛り・枠」と「各式の曲面」を管理する。
// 座標は数学と同じく z 軸が上向き。

import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { CSS2DObject, CSS2DRenderer } from 'three/examples/jsm/renderers/CSS2DRenderer.js'
import type { Domain, MeshData } from './types'

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

export class Viewer {
  private renderer: THREE.WebGLRenderer
  private labelRenderer: CSS2DRenderer
  private scene = new THREE.Scene()
  private camera = new THREE.PerspectiveCamera(45, 1, 0.01, 1000)
  private controls: OrbitControls
  private frame = new THREE.Group() // 軸・目盛り・枠
  private plots = new Map<number, THREE.Group>() // 式ごとの曲面
  private clipPlanes: THREE.Plane[] = []
  private domain: Domain | null = null
  private theme: Theme
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

    const dark = window.matchMedia('(prefers-color-scheme: dark)')
    this.theme = dark.matches ? DARK : LIGHT
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
    this.camera.updateProjectionMatrix()
    this.renderer.setSize(w, h)
    this.labelRenderer.setSize(w, h)
  }

  private loop = () => {
    if (this.stopped) return
    requestAnimationFrame(this.loop)
    this.controls.update()
    this.renderer.render(this.scene, this.camera)
    this.labelRenderer.render(this.scene, this.camera)
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
    for (const g of this.plots.values()) {
      g.traverse((o) => {
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
    const size = Math.hypot(d.xmax - d.xmin, d.ymax - d.ymin, d.zmax - d.zmin)
    this.controls.target.copy(center)
    this.camera.position.copy(center).add(new THREE.Vector3(1.1, -1.5, 0.9).normalize().multiplyScalar(size * 1.35))
    this.camera.near = size / 1000
    this.camera.far = size * 100
    this.camera.updateProjectionMatrix()
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
      [new THREE.Vector3(d.xmin, o.y, o.z), new THREE.Vector3(d.xmax, o.y, o.z), 0xd9534f, 'x'],
      [new THREE.Vector3(o.x, d.ymin, o.z), new THREE.Vector3(o.x, d.ymax, o.z), 0x4caf50, 'y'],
      [new THREE.Vector3(o.x, o.y, d.zmin), new THREE.Vector3(o.x, o.y, d.zmax), 0x3f7fd9, 'z'],
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

  /** 式 id の曲面を設定（作り直し） */
  setPlot(id: number, mesh: MeshData, color: string, visible: boolean) {
    this.removePlot(id)

    const geo = new THREE.BufferGeometry()
    geo.setAttribute('position', new THREE.BufferAttribute(mesh.positions, 3))
    geo.setAttribute('normal', new THREE.BufferAttribute(mesh.normals, 3))
    geo.setIndex(new THREE.BufferAttribute(mesh.indices, 1))

    // 表面と裏面で色の明るさを変えて、どちら側を見ているか分かるようにする
    const front = new THREE.MeshStandardMaterial({
      roughness: 0.55,
      metalness: 0.05,
      side: THREE.FrontSide,
      clippingPlanes: this.clipPlanes,
    })
    const back = new THREE.MeshStandardMaterial({
      roughness: 0.7,
      metalness: 0.0,
      side: THREE.BackSide,
      clippingPlanes: this.clipPlanes,
    })
    const group = new THREE.Group()
    group.add(new THREE.Mesh(geo, front), new THREE.Mesh(geo, back))
    this.plots.set(id, group)
    this.scene.add(group)
    this.setStyle(id, color, visible)
  }

  /** 色と表示/非表示だけを変える（計算し直さない） */
  setStyle(id: number, color: string, visible: boolean) {
    const g = this.plots.get(id)
    if (!g) return
    g.visible = visible
    const [front, back] = g.children as THREE.Mesh[]
    const c = new THREE.Color(color)
    ;(front.material as THREE.MeshStandardMaterial).color.copy(c)
    ;(back.material as THREE.MeshStandardMaterial).color.copy(c).multiplyScalar(0.55)
  }

  removePlot(id: number) {
    const g = this.plots.get(id)
    if (!g) return
    const [front, back] = g.children as THREE.Mesh[]
    front.geometry.dispose()
    ;(front.material as THREE.Material).dispose()
    ;(back.material as THREE.Material).dispose()
    this.scene.remove(g)
    this.plots.delete(id)
  }
}
