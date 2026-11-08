// Three.js を使った3D表示。
// 「座標軸・目盛り・枠」と「各式の曲面」を管理する。
// 座標は数学と同じく z 軸が上向き。

import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js'
import { LineSegments2 } from 'three/examples/jsm/lines/LineSegments2.js'
import { LineSegmentsGeometry } from 'three/examples/jsm/lines/LineSegmentsGeometry.js'
import { createGridUniforms, createSurfaceMaterial, type GridUniforms } from './surfaceMaterial'
import type { CapData, CutRequest, Domain, MeshData, ParamSpan, PlotStyle } from './types'

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

/**
 * 目盛りの数字・軸の名前の文字。文字を描いた小さな画像（スプライト）として WebGL で描く。
 * （HTML の要素にすると、グラフを回すたびに全部の位置を書き換えることになって重い）
 * 画面上の大きさを一定にするため、`w`, `h`（CSS ピクセル）から毎回の描画前に大きさを決める。
 */
interface TextLabel {
  sprite: THREE.Sprite
  w: number
  h: number
}

function makeLabel(text: string, font: string, color: string, pos: THREE.Vector3): TextLabel {
  const dpr = 2 // くっきり見えるよう、2倍の細かさで描いておく
  const canvas = document.createElement('canvas')
  const ctx = canvas.getContext('2d')!
  ctx.font = font
  const size = Number(/(\d+)px/.exec(font)?.[1] ?? 12)
  const w = Math.ceil(ctx.measureText(text).width) + 4
  const h = Math.ceil(size * 1.4)
  canvas.width = w * dpr
  canvas.height = h * dpr
  ctx.scale(dpr, dpr)
  ctx.font = font
  ctx.fillStyle = color
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.fillText(text, w / 2, h / 2)
  const tex = new THREE.CanvasTexture(canvas)
  tex.colorSpace = THREE.SRGBColorSpace
  const mat = new THREE.SpriteMaterial({ map: tex, sizeAttenuation: false, depthTest: false, depthWrite: false })
  const sprite = new THREE.Sprite(mat)
  sprite.position.copy(pos)
  sprite.renderOrder = 10 // いつも手前に描く
  return { sprite, w, h }
}

interface Theme {
  background: number
  box: number
  grid: number
  /** 目盛りの数字の色 */
  tick: string
  /** 軸の名前の色 */
  text: string
}
const LIGHT: Theme = { background: 0xf7f7f4, box: 0x9a9aa2, grid: 0xd6d6d2, tick: '#6b6b75', text: '#1f1f24' }
const DARK: Theme = { background: 0x1b1c20, box: 0x6a6b72, grid: 0x34353b, tick: '#9a9aa3', text: '#e8e8ea' }
const TICK_FONT = "11px ui-monospace, 'SF Mono', Consolas, monospace"
const AXIS_FONT = "italic 600 15px 'Times New Roman', serif"

/**
 * 媒介変数表示の立体の切り口のふた（ステンシルバッファを使う）。
 *
 * 切り口の平面上の点 P が立体の中にあるかは、カメラから P への視線が、残した側で立体の面を何回出入りしたかで分かる。
 * 1. inc: 視線が立体に「入る」向きの面を描き、その画素のステンシルを 1 増やす（色・奥行きは書かない）
 * 2. dec: 「出る」向きの面を描き、ステンシルを 1 減らす（0 より下には減らさない）
 * 3. quad: 切り口の平面（表示範囲の断面の長方形）を、ステンシルが 0 でない画素にだけ描き、ステンシルを 0 に戻す
 * カメラが切り落とした側にいるときは P から奥へ、残した側にいるときはカメラから P までを数えるので、
 * 「入る」面は前者なら裏面、後者なら表面になる（毎フレーム確かめて入れ替える）
 */
interface StencilCap {
  inc: THREE.Mesh
  dec: THREE.Mesh
  quad: THREE.Mesh
}

/** 1つの式の描画物 */
type PlotObject =
  | {
      kind: 'surface'
      group: THREE.Group // 曲面（と切り口のふた）
      grid: GridUniforms
      /** 媒介変数表示なら u, v（立体なら3つ目も）の範囲の長さ（格子線を媒介変数に沿って引く） */
      uvSpan: ParamSpan | null
      /** 媒介変数表示の立体か（切り口のふたをステンシルで作る） */
      solid: boolean
      /** 陰関数・範囲で重ねた立体の切り口のふた（計算担当が作ったもの） */
      cap: THREE.Mesh | null
      stencil: StencilCap | null
      /** ステンシルで描く順番を決める番号 */
      order: number
    }
  | { kind: 'curve'; line: LineSegments2 }

export class Viewer {
  private renderer: THREE.WebGLRenderer
  /** 目盛りの数字・軸の名前 */
  private labels: TextLabel[] = []
  private scene = new THREE.Scene()
  private camera = new THREE.PerspectiveCamera(45, 1, 0.01, 1000)
  private controls: OrbitControls
  private frame = new THREE.Group() // 軸・目盛り・枠
  private plots = new Map<number, PlotObject>() // 式ごとの曲面・曲線
  /** 表示範囲の外を切り落とす6枚の平面 */
  private boxPlanes: THREE.Plane[] = []
  /** 切り口の平面（切らないなら null） */
  private cutPlane: THREE.Plane | null = null
  private cut: CutRequest = null
  /** 曲面・曲線を切り落とす平面（表示範囲 + 切り口） */
  private clipPlanes: THREE.Plane[] = []
  private nextOrder = 0
  /** 2D 表示（真上から見て、回転させない） */
  private flat = false
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

  // ---- 描画を軽くするための状態 ----
  /** 描き直しが必要か（何も変わっていなければ描かない） */
  private dirty = true
  /** ふつうのときの画素密度（スマホの高い密度は 2 倍までに抑える） */
  private basePixelRatio = Math.min(window.devicePixelRatio, 2)
  /** 指やマウスでグラフを動かしている最中か */
  private interacting = false
  /** スライダーを動かしている・回転を再生しているなど、グラフが次々に変わっている最中か */
  private busy = false
  /** 動かしている最中は画素密度を下げて描く */
  private lowRes = false
  /** カメラが（ほぼ）止まってから何フレームたったか */
  private stillFrames = 0
  /** 前のフレームのカメラの位置と向き（止まったかどうかの判定に使う） */
  private lastCamPos = new THREE.Vector3()
  private lastCamQuat = new THREE.Quaternion()

  constructor(private container: HTMLElement) {
    // 描画の土台
    // ステンシルバッファは、媒介変数表示の立体の切り口のふたに使う
    this.renderer = new THREE.WebGLRenderer({ antialias: true, stencil: true })
    this.renderer.setPixelRatio(this.basePixelRatio)
    this.renderer.localClippingEnabled = true // 表示範囲の外を切り落とすため
    container.appendChild(this.renderer.domElement)

    // 文字（目盛りの数字）用の重ね合わせ

    this.theme = LIGHT
    this.scene.background = new THREE.Color(this.theme.background)

    // カメラとマウス操作（左ドラッグ: 回転 / 右ドラッグ: 移動 / ホイール: 拡大縮小）
    this.camera.up.set(0, 0, 1)
    this.controls = new OrbitControls(this.camera, this.renderer.domElement)
    this.controls.enableDamping = true
    this.controls.dampingFactor = 0.1 // 指を離した後の慣性の回転を、少し早めに止める
    // グラフを動かし始めたら画素密度を下げ、止まったら元に戻す（動いている間は細部が見えにくいので、軽さを優先）
    this.controls.addEventListener('start', () => {
      this.interacting = true
      this.setLowRes(true)
    })
    this.controls.addEventListener('end', () => (this.interacting = false))

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

  /** 次のフレームで描き直す */
  invalidate() {
    this.dirty = true
  }

  /**
   * スライダーを動かしている間など、グラフが次々に変わる間は true にする。
   * その間は、グラフを回しているときと同じく画素密度を下げて描く（止まったら元に戻す）
   */
  setBusy(on: boolean) {
    this.busy = on
    if (on) this.setLowRes(true)
    this.dirty = true
  }

  private setLowRes(on: boolean) {
    if (this.lowRes === on) return
    this.lowRes = on
    this.renderer.setPixelRatio(on ? Math.min(this.basePixelRatio, 1) : this.basePixelRatio)
    this.dirty = true
  }

  private resize() {
    const w = this.container.clientWidth
    const h = this.container.clientHeight
    if (w === 0 || h === 0) return
    this.dirty = true
    this.camera.aspect = w / h
    this.renderer.setSize(w, h)
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
    this.dirty = true
  }

  private loop = () => {
    if (this.stopped) return
    requestAnimationFrame(this.loop)
    // カメラが動いたか（慣性で動き続けている間も true）
    if (this.controls.update()) this.dirty = true
    // 目に見えるほど動いたか。慣性の終わりのごくわずかな動きは「止まった」とみなす
    const dist = this.camera.position.distanceTo(this.controls.target)
    const visible =
      this.camera.position.distanceTo(this.lastCamPos) > dist * 3e-4 ||
      this.camera.quaternion.angleTo(this.lastCamQuat) > 3e-4
    this.lastCamPos.copy(this.camera.position)
    this.lastCamQuat.copy(this.camera.quaternion)
    if (visible) {
      this.stillFrames = 0
    } else if (++this.stillFrames > 4 && !this.interacting && !this.busy) {
      this.setLowRes(false) // 止まったら、元の画素密度で描き直す
    }
    // 何も変わっていなければ描かない（止まっているときに電池を使わない）
    if (!this.dirty) return
    this.dirty = false
    this.updateClipRange()
    this.updateLabelSizes()
    this.updateStencilSides()
    this.renderer.render(this.scene, this.camera)
  }

  /**
   * 文字の画面上の大きさを、CSS ピクセルで一定にする。
   * sizeAttenuation: false のスプライトは、大きさ 1 が「画面の高さ × 射影行列の [1][1] / 2」になるので、その逆数をかける
   */
  private updateLabelSizes() {
    const k = 2 / (this.container.clientHeight * this.camera.projectionMatrix.elements[5])
    for (const l of this.labels) l.sprite.scale.set(l.w * k, l.h * k, 1)
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
    this.dirty = true
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
    this.dirty = true
    this.axisNames = [...names]
    if (this.domain) this.buildFrame(this.domain)
  }

  /** 背景の明暗を切り替える */
  setDark(dark: boolean) {
    this.dirty = true
    this.theme = dark ? DARK : LIGHT
    ;(this.scene.background as THREE.Color).set(this.theme.background)
    if (this.domain) this.buildFrame(this.domain)
  }

  /** 表示範囲を設定し、軸・目盛り・枠を作り直す */
  setDomain(d: Domain, resetCamera = false) {
    this.dirty = true
    const first = this.domain === null
    this.domain = { ...d }

    // 範囲の外を切り落とす6枚の平面
    this.boxPlanes = [
      new THREE.Plane(new THREE.Vector3(1, 0, 0), -d.xmin),
      new THREE.Plane(new THREE.Vector3(-1, 0, 0), d.xmax),
      new THREE.Plane(new THREE.Vector3(0, 1, 0), -d.ymin),
      new THREE.Plane(new THREE.Vector3(0, -1, 0), d.ymax),
    ]
    // 2D 表示では、図形は z = 0 の平面にあるので z の範囲では切らない
    if (!this.flat) {
      this.boxPlanes.push(
        new THREE.Plane(new THREE.Vector3(0, 0, 1), -d.zmin),
        new THREE.Plane(new THREE.Vector3(0, 0, -1), d.zmax),
      )
    }
    this.applyClipping()
    for (const p of this.plots.values()) {
      if (p.kind === 'surface') p.grid.uHeatRange.value.set(d.zmin, d.zmax)
    }

    this.buildFrame(d)
    if (first || resetCamera) this.resetCamera()
  }

  /**
   * 2D 表示に切り替える。真上から見て、ドラッグは回転ではなく移動にする。
   * 図形は計算担当が z = 0 の平面に作る（縦の座標を 0 にする）
   */
  setFlat(on: boolean) {
    if (this.flat === on) return
    this.flat = on
    this.controls.enableRotate = !on
    this.controls.mouseButtons.LEFT = on ? THREE.MOUSE.PAN : THREE.MOUSE.ROTATE
    this.controls.touches.ONE = on ? THREE.TOUCH.PAN : THREE.TOUCH.ROTATE
    this.camera.up.set(0, on ? 1 : 0, on ? 0 : 1)
    for (const p of this.plots.values()) if (p.kind === 'surface') p.grid.uNoBackShade.value = on ? 1 : 0
    if (this.domain) this.setDomain(this.domain, true)
  }

  /** カメラを斜め上からの見やすい位置に戻す（2D 表示なら真上から） */
  resetCamera() {
    this.dirty = true
    const d = this.domain
    if (!d) return
    if (this.flat) {
      // 表示範囲の長方形が画面に収まる距離（縦長の画面では横幅に合わせる）
      const c = new THREE.Vector3((d.xmin + d.xmax) / 2, (d.ymin + d.ymax) / 2, 0)
      const half = Math.max((d.ymax - d.ymin) / 2, (d.xmax - d.xmin) / 2 / this.camera.aspect) * 1.12
      this.controls.target.copy(c)
      this.camera.position.copy(c).add(new THREE.Vector3(0, 0, half / this.perspective))
      this.camera.lookAt(c)
      this.updateClipRange()
      return
    }
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
    for (const l of this.labels) {
      l.sprite.material.map?.dispose()
      l.sprite.material.dispose()
    }
    this.labels = []
    this.frame.clear()
    const label = (text: string, kind: 'tick' | 'axis-name', pos: THREE.Vector3) => {
      const l =
        kind === 'tick'
          ? makeLabel(text, TICK_FONT, this.theme.tick, pos)
          : makeLabel(text, AXIS_FONT, this.theme.text, pos)
      this.labels.push(l)
      return l.sprite
    }

    const t = this.theme
    const lo = new THREE.Vector3(d.xmin, d.ymin, d.zmin)
    const hi = new THREE.Vector3(d.xmax, d.ymax, d.zmax)

    // 外枠（2D 表示では z = 0 の長方形）
    if (this.flat) {
      lo.z = hi.z = 0
    }
    const box = new THREE.Box3Helper(new THREE.Box3(lo, hi), t.box)
    this.frame.add(box)

    // 床（z = 0、範囲外なら一番下）の方眼
    const zFloor = this.flat ? 0 : d.zmin <= 0 && 0 <= d.zmax ? 0 : d.zmin
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
    const o = new THREE.Vector3(
      clamp(0, d.xmin, d.xmax),
      clamp(0, d.ymin, d.ymax),
      this.flat ? 0 : clamp(0, d.zmin, d.zmax),
    )
    const axes: [THREE.Vector3, THREE.Vector3, number, string][] = [
      [new THREE.Vector3(d.xmin, o.y, o.z), new THREE.Vector3(d.xmax, o.y, o.z), 0xd9534f, this.axisNames[0]],
      [new THREE.Vector3(o.x, d.ymin, o.z), new THREE.Vector3(o.x, d.ymax, o.z), 0x4caf50, this.axisNames[1]],
      [new THREE.Vector3(o.x, o.y, d.zmin), new THREE.Vector3(o.x, o.y, d.zmax), 0x3f7fd9, this.axisNames[2]],
    ]
    if (this.flat) axes.pop() // 2D 表示では縦の軸は無い
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
      // 2D 表示では左の辺に並べる
      const x = this.flat ? d.xmin - pad * 1.3 : d.xmax + pad
      this.frame.add(label(fmt(y), 'tick', new THREE.Vector3(x, y, zFloor)))
    }
    if (this.flat) {
      // 枠・方眼・軸は、z = 0 にある図形の下に描く（同じ高さで重なってちらつかないように）
      this.frame.traverse((o) => {
        if (o instanceof THREE.LineSegments) {
          const m = o.material as THREE.Material
          m.depthTest = false
          m.depthWrite = false
          o.renderOrder = -10000
        }
      })
      return
    }
    for (const z of ticks(d.zmin, d.zmax, niceStep(d.zmax - d.zmin, 6))) {
      this.frame.add(label(fmt(z), 'tick', new THREE.Vector3(d.xmin - pad, d.ymin - pad, z)))
    }
  }

  /** 式 id の曲面・曲線を設定（作り直し） */
  setPlot(id: number, mesh: MeshData, style: PlotStyle) {
    const old = this.plots.get(id)
    const kind = mesh.curve ? 'curve' : 'surface'
    // 同じ種類なら、材質（シェーダー）はそのまま使い、形のデータだけ差し替える。
    // 回転を再生しているときなど、毎フレーム形が変わるときに軽くなる
    if (old && old.kind === kind) {
      if (old.kind === 'curve') {
        old.line.geometry.dispose()
        old.line.geometry = this.curveGeometry(mesh.positions)
      } else {
        const m = old.group.children[0] as THREE.Mesh
        m.geometry.dispose()
        m.geometry = this.surfaceGeometry(mesh)
        old.uvSpan = mesh.params.length >= 2 ? mesh.paramSpan : null
        old.solid = mesh.solid
        old.grid.uComplex.value = mesh.complexValues.length > 0 ? 1 : 0
        old.grid.uHeat.value = mesh.heat ? 1 : 0
        if (old.stencil) old.stencil.inc.geometry = old.stencil.dec.geometry = m.geometry
      }
    } else {
      this.removePlot(id)
      if (mesh.curve) this.setCurve(id, mesh.positions)
      else this.setSurface(id, mesh)
    }
    this.updateStencil(id)
    this.setStyle(id, style)
  }

  // ---- 切り口 ----

  /** 曲面・曲線・ふたの材質に、切り落とす平面を設定し直す */
  private applyClipping() {
    this.clipPlanes = this.cutPlane ? [...this.boxPlanes, this.cutPlane] : this.boxPlanes
    for (const p of this.plots.values()) {
      if (p.kind === 'curve') {
        ;(p.line.material as THREE.Material).clippingPlanes = this.clipPlanes
        continue
      }
      ;((p.group.children[0] as THREE.Mesh).material as THREE.Material).clippingPlanes = this.clipPlanes
      // ふたは切り口の平面の上にあるので、切り口の平面では切らない
      if (p.cap) (p.cap.material as THREE.Material).clippingPlanes = this.boxPlanes
      if (p.stencil) {
        ;(p.stencil.inc.material as THREE.Material).clippingPlanes = this.clipPlanes
        ;(p.stencil.dec.material as THREE.Material).clippingPlanes = this.clipPlanes
        ;(p.stencil.quad.material as THREE.Material).clippingPlanes = this.boxPlanes
      }
    }
    for (const id of this.plots.keys()) this.updateStencil(id)
    this.dirty = true
  }

  /**
   * 切り口を設定する（null なら切らない）。画面の軸 axis の座標が pos の平面で切り、
   * keepBelow なら座標が pos 以下の側を残す
   */
  setCut(cut: CutRequest) {
    this.cut = cut && { ...cut }
    if (!cut) {
      this.cutPlane = null
    } else {
      const n = new THREE.Vector3().setComponent(cut.axis, cut.keepBelow ? -1 : 1)
      this.cutPlane = new THREE.Plane(n, cut.keepBelow ? cut.pos : -cut.pos)
    }
    this.stencilSideKept = null
    this.applyClipping()
  }

  /** 陰関数・範囲で重ねた立体の切り口のふたを設定する（null なら消す） */
  setCap(id: number, cap: CapData | null, style: PlotStyle) {
    const p = this.plots.get(id)
    if (!p || p.kind !== 'surface') return
    this.dirty = true
    if (p.cap) {
      p.cap.geometry.dispose()
      if (!cap || cap.indices.length === 0) {
        ;(p.cap.material as THREE.Material).dispose()
        p.group.remove(p.cap)
        p.cap = null
        return
      }
    }
    if (!cap || cap.indices.length === 0) return
    const geo = new THREE.BufferGeometry()
    const pos = new THREE.BufferAttribute(cap.positions, 3)
    geo.setAttribute('position', pos)
    geo.setAttribute('normal', new THREE.BufferAttribute(cap.normals, 3))
    geo.setAttribute('gridCoord', pos)
    geo.setAttribute('complexValue', pos)
    geo.setIndex(new THREE.BufferAttribute(cap.indices, 1))
    if (p.cap) {
      p.cap.geometry = geo
    } else {
      // 格子線は曲面と同じ設定（x, y, z の間隔）で引く
      p.cap = new THREE.Mesh(geo, createSurfaceMaterial(p.grid, this.boxPlanes))
      p.group.add(p.cap)
      this.setStyle(id, style)
    }
  }

  /** 媒介変数表示の立体のステンシルのふたを、切っているなら作り、切っていないなら消す */
  private updateStencil(id: number) {
    const p = this.plots.get(id)
    if (!p || p.kind !== 'surface') return
    const want = p.solid && this.cutPlane !== null && this.cut !== null && this.domain !== null
    if (!want) {
      if (p.stencil) {
        for (const m of [p.stencil.inc, p.stencil.dec, p.stencil.quad]) {
          ;(m.material as THREE.Material).dispose()
          p.group.remove(m)
        }
        p.stencil.quad.geometry.dispose()
        p.stencil = null
      }
      return
    }
    const main = p.group.children[0] as THREE.Mesh
    if (!p.stencil) {
      const pass = (op: THREE.StencilOp) =>
        new THREE.MeshBasicMaterial({
          colorWrite: false,
          depthWrite: false,
          depthTest: false,
          stencilWrite: true,
          stencilFunc: THREE.AlwaysStencilFunc,
          stencilFail: op,
          stencilZFail: op,
          stencilZPass: op,
          clippingPlanes: this.clipPlanes,
        })
      const inc = new THREE.Mesh(main.geometry, pass(THREE.IncrementWrapStencilOp))
      const dec = new THREE.Mesh(main.geometry, pass(THREE.DecrementStencilOp))
      const quadMat = new THREE.MeshStandardMaterial({
        side: THREE.DoubleSide,
        roughness: 0.7,
        metalness: 0.05,
        stencilWrite: true,
        stencilRef: 0,
        stencilFunc: THREE.NotEqualStencilFunc,
        stencilFail: THREE.ReplaceStencilOp,
        stencilZFail: THREE.ReplaceStencilOp,
        stencilZPass: THREE.ReplaceStencilOp,
        clippingPlanes: this.boxPlanes,
      })
      const quad = new THREE.Mesh(new THREE.BufferGeometry(), quadMat)
      // 次の立体のために、ステンシルを 0 に戻しておく（ふたが描かれなかった画素も含めて）
      quad.onAfterRender = (r) => r.clearStencil()
      const base = -3000 + 3 * p.order
      inc.renderOrder = base
      dec.renderOrder = base + 1
      quad.renderOrder = base + 2
      for (const m of [inc, dec, quad]) m.frustumCulled = false
      p.stencil = { inc, dec, quad }
      p.group.add(inc, dec, quad)
      this.stencilSideKept = null // 次の描画で向きを決め直す
      const style = this.styles.get(id)
      if (style) queueMicrotask(() => this.setStyle(id, style)) // 色・不透明度をふたにも反映する
    }
    // 切り口の長方形（表示範囲の断面）。法線は切り落とした側に向ける
    const d = this.domain!
    const cut = this.cut!
    const lo = [d.xmin, d.ymin, d.zmin]
    const hi = [d.xmax, d.ymax, d.zmax]
    const [a, b] = [0, 1, 2].filter((k) => k !== cut.axis)
    const corner = (u: number, v: number) => {
      const q = [0, 0, 0]
      q[cut.axis] = cut.pos
      q[a] = u ? hi[a] : lo[a]
      q[b] = v ? hi[b] : lo[b]
      return q
    }
    const nrm = [0, 0, 0]
    nrm[cut.axis] = cut.keepBelow ? 1 : -1
    const geo = new THREE.BufferGeometry()
    geo.setAttribute('position', new THREE.Float32BufferAttribute([...corner(0, 0), ...corner(1, 0), ...corner(1, 1), ...corner(0, 1)], 3))
    geo.setAttribute('normal', new THREE.Float32BufferAttribute([...nrm, ...nrm, ...nrm, ...nrm], 3))
    // 三角形の向き: (a, b, 切る軸) が x→y→z の順の巡回なら、頂点の順のままで +軸 の向き
    const up = (a + 1) % 3 === b
    geo.setIndex(up === cut.keepBelow ? [0, 1, 2, 0, 2, 3] : [0, 2, 1, 0, 3, 2])
    p.stencil.quad.geometry.dispose()
    p.stencil.quad.geometry = geo
    p.stencil.inc.geometry = p.stencil.dec.geometry = main.geometry
  }

  /** ステンシルの inc / dec で使っている面の向きが、カメラが残した側にいるときのものか */
  private stencilSideKept: boolean | null = null

  /** カメラが切り口のどちら側にいるかで、ステンシルで数える面の向きを入れ替える */
  private updateStencilSides() {
    if (!this.cutPlane) return
    const kept = this.cutPlane.distanceToPoint(this.camera.position) >= 0
    if (kept === this.stencilSideKept) return
    this.stencilSideKept = kept
    for (const p of this.plots.values()) {
      if (p.kind !== 'surface' || !p.stencil) continue
      const set = (m: THREE.Mesh, side: THREE.Side) => {
        const mat = m.material as THREE.Material
        if (mat.side !== side) {
          mat.side = side
          mat.needsUpdate = true
        }
      }
      // 「入る」面: カメラが残した側なら表面（カメラから P まで）、切り落とした側なら裏面（P から奥）
      set(p.stencil.inc, kept ? THREE.FrontSide : THREE.BackSide)
      set(p.stencil.dec, kept ? THREE.BackSide : THREE.FrontSide)
    }
  }

  private surfaceGeometry(mesh: MeshData): THREE.BufferGeometry {
    const geo = new THREE.BufferGeometry()
    const pos = new THREE.BufferAttribute(mesh.positions, 3)
    geo.setAttribute('position', pos)
    geo.setAttribute('normal', new THREE.BufferAttribute(mesh.normals, 3))
    geo.setAttribute('gridCoord', mesh.gridCoords.length > 0 ? new THREE.BufferAttribute(mesh.gridCoords, 3) : pos)
    // 複素数の色分けをしないときも、シェーダーが読む属性は用意しておく（使わないので位置で代用）
    geo.setAttribute('complexValue', mesh.complexValues.length > 0 ? new THREE.BufferAttribute(mesh.complexValues, 3) : pos)
    geo.setIndex(new THREE.BufferAttribute(mesh.indices, 1))
    return geo
  }

  private setSurface(id: number, mesh: MeshData) {
    // 表と裏は1つの材質で描く（裏面はシェーダーで暗くする）
    const grid = createGridUniforms()
    const group = new THREE.Group()
    group.add(new THREE.Mesh(this.surfaceGeometry(mesh), createSurfaceMaterial(grid, this.clipPlanes)))
    const uvSpan = mesh.params.length >= 2 ? mesh.paramSpan : null
    grid.uComplex.value = mesh.complexValues.length > 0 ? 1 : 0
    grid.uHeat.value = mesh.heat ? 1 : 0
    grid.uNoBackShade.value = this.flat ? 1 : 0
    if (this.domain) grid.uHeatRange.value.set(this.domain.zmin, this.domain.zmax)
    this.plots.set(id, {
      kind: 'surface',
      group,
      grid,
      uvSpan,
      solid: mesh.solid,
      cap: null,
      stencil: null,
      order: this.nextOrder++,
    })
    this.scene.add(group)
  }

  /** 曲線の形。点の列を、隣り合う2点ずつの線分にする（NaN の所は線を切る） */
  private curveGeometry(pts: Float32Array): LineSegmentsGeometry {
    // 線分の数を先に数えて、配列を1回で確保する（毎フレーム作り直すときに軽いように）
    const ok = (k: number) => Number.isFinite(pts[k]) && Number.isFinite(pts[k + 1]) && Number.isFinite(pts[k + 2])
    let n = 0
    for (let k = 0; k + 5 < pts.length; k += 3) if (ok(k) && ok(k + 3)) n++
    const seg = new Float32Array(n * 6)
    let o = 0
    for (let k = 0; k + 5 < pts.length; k += 3) {
      if (ok(k) && ok(k + 3)) {
        seg.set(pts.subarray(k, k + 6), o)
        o += 6
      }
    }
    const geo = new LineSegmentsGeometry()
    if (n > 0) geo.setPositions(seg)
    return geo
  }

  private setCurve(id: number, pts: Float32Array) {
    const mat = new LineMaterial({ linewidth: 3, clippingPlanes: this.clipPlanes })
    mat.resolution.set(this.container.clientWidth, this.container.clientHeight)
    const line = new LineSegments2(this.curveGeometry(pts), mat)
    this.plots.set(id, { kind: 'curve', line })
    this.scene.add(line)
  }

  /** 式ごとの最後の見た目（後から作ったふたにも同じ色を付けるため） */
  private styles = new Map<number, PlotStyle>()

  /** 色・表示/非表示・格子線などの見た目だけを変える（計算し直さない） */
  setStyle(id: number, style: PlotStyle) {
    const p = this.plots.get(id)
    if (!p) return
    this.styles.set(id, style)
    this.dirty = true
    const c = new THREE.Color(style.color)

    if (p.kind === 'curve') {
      p.line.visible = style.visible
      const m = p.line.material as LineMaterial
      m.color.copy(c)
      m.linewidth = style.lineWidth
      return
    }

    p.group.visible = style.visible
    const [mesh0] = p.group.children as THREE.Mesh[]
    ;(mesh0.material as THREE.MeshStandardMaterial).color.copy(c)
    // 切り口のふたは、少し明るい色で塗る（曲面の裏側と見分けやすいように）
    const capColor = c.clone().lerp(new THREE.Color(0xffffff), 0.3)
    if (p.cap) (p.cap.material as THREE.MeshStandardMaterial).color.copy(capColor)
    if (p.stencil) (p.stencil.quad.material as THREE.MeshStandardMaterial).color.copy(capColor)

    // 面の不透明度。1 未満なら半透明として描き、奥の面や線も透けて見えるようにする
    const opacity = Math.min(1, Math.max(0, style.opacity))
    if (p.stencil) {
      // ふたが半透明なら、ステンシルを数える面も同じ順番（半透明の物の列）で描く
      const transparent = opacity < 1
      for (const m of [p.stencil.inc, p.stencil.dec, p.stencil.quad]) {
        const mat = m.material as THREE.Material
        if (mat.transparent !== transparent) {
          mat.transparent = transparent
          mat.needsUpdate = true
        }
      }
      const q = p.stencil.quad.material as THREE.MeshStandardMaterial
      q.opacity = Math.max(opacity, 0.15)
      q.depthWrite = !transparent
    }
    for (const mesh of p.cap ? [mesh0, p.cap] : [mesh0]) {
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
      // 立体なら、3つ目の媒介変数の線を z の成分に入れている（等高線の代わり）
      const third = p.uvSpan.w !== undefined ? div(p.uvSpan.w, g.w ?? g.v) : g.z
      p.grid.uGridSpacing.value.set(sp(div(p.uvSpan.u, g.u)), sp(div(p.uvSpan.v, g.v)), sp(third))
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
    this.dirty = true
    if (p.kind === 'curve') {
      p.line.geometry.dispose()
      ;(p.line.material as THREE.Material).dispose()
      this.scene.remove(p.line)
    } else {
      for (const o of p.group.children as THREE.Mesh[]) {
        // ステンシルの inc / dec は曲面と形のデータを共有しているので、材質だけを捨てる
        if (!p.stencil || (o !== p.stencil.inc && o !== p.stencil.dec)) o.geometry.dispose()
        ;(o.material as THREE.Material).dispose()
      }
      this.scene.remove(p.group)
    }
    this.plots.delete(id)
    this.styles.delete(id)
  }
}
