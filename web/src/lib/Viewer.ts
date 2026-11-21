// Three.js を使った3D表示。
// 「座標軸・目盛り・枠」と「各式の曲面」を管理する。
// 座標は数学と同じく z 軸が上向き。

import * as THREE from 'three'
import { OrbitControls } from 'three/examples/jsm/controls/OrbitControls.js'
import { LineMaterial } from 'three/examples/jsm/lines/LineMaterial.js'
import { LineSegments2 } from 'three/examples/jsm/lines/LineSegments2.js'
import { LineSegmentsGeometry } from 'three/examples/jsm/lines/LineSegmentsGeometry.js'
import { axisTicks, axisTickText } from './axisTicks'
import { mixPositions, vertexNormals, weldGroups, weldNormals } from './coords'
import { MotionQuality, pickLod, type MotionMode, type Stage } from './motionQuality'
import { SurfaceTransparency } from './SurfaceTransparency'
import { RotationSurfaceTrail, regionTrailTriangles } from './RotationSurfaceTrail'
import { gpu2dMaterial, gpu2dMesh, setGpu2dUniforms, type Gpu2dUniforms } from './gpu2dMaterial'
import { wantGpu } from './renderMode'
import { applyShownRotation, createRaycastMaterial, createRaycastUniforms, RAY_MOVING, RAY_STILL, raycastMesh, setRayAffine, type RaycastUniforms } from './raycastMaterial'
import { COLORMAP_INDEX, colormapColor, valueT, logRange, createGridUniforms, createSurfaceMaterial, type GridUniforms } from './surfaceMaterial'
import { applyVolumeStyle, createVolumeMaterial, createVolumeUniforms, regionBounds, volumeTexture, type VolumeUniforms } from './volumeMaterial'
import type { CapData, CutRequest, Domain, MeshData, ParamSpan, PlotStyle } from './types'

/**
 * 目盛りの数字・軸の名前の文字。文字を描いた小さな画像（スプライト）として WebGL で描く。
 * （HTML の要素にすると、グラフを回すたびに全部の位置を書き換えることになって重い）
 * 画面上の大きさを一定にするため、`w`, `h`（CSS ピクセル）から毎回の描画前に大きさを決める。
 */
interface TextLabel {
  sprite: THREE.Sprite
  anchor: THREE.Vector3
  w: number
  h: number
  priority: number
  nudgeX: number
  x: number
  y: number
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
  return { sprite, anchor: pos, w, h, priority: 0, nudgeX: 0, x: 0, y: 0 }
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
const EDGE_FONT = `600 ${TICK_FONT}`

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

/** 押した所にあった式の点。poi は注目する点の種類（0: 極大, 1: 極小, 2: 鞍点, 3: 零点, 4: 縦軸との交点, 5: そのほかの停留点） */
export interface PickResult {
  id: number
  point: [number, number, number]
  poi?: number
  /** 同じ点にある注目する点の種類（全部） */
  kinds?: number[]
}

/** 1つの式の描画物 */
type PlotObject =
  | {
      kind: 'surface'
      group: THREE.Group // 曲面（と切り口のふた）
      /** 曲面の形（そのままの細かさ）。粗い版があれば geo.userData.lod に入っている（頂点のデータは共有） */
      geo: THREE.BufferGeometry
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
      /** 2D 表示の不等式の領域の境目の線（実線・点線。group の子）。ベクトル場では流線 */
      outlines: LineSegments2[]
      /** ベクトル場（矢印・流線を、鮮やかな色の対応表で塗る） */
      vivid: boolean
      /** 伝達関数（表示範囲の箱。group の子）と、その材質の値 */
      volume: { box: THREE.Mesh; u: VolumeUniforms } | null
    }
  | {
      kind: 'curve'
      line: LineSegments2
      /** 長さの無い曲線（点。複素平面の点 1 + 2i や組 (1, 2, 3)）を描く丸。line の子 */
      dots: THREE.InstancedMesh
    }

type TrailFrame = { lines: Float32Array; dots: Float32Array }
type CurveTrail = {
  kind: 'curve'
  lines: THREE.LineSegments<THREE.BufferGeometry, THREE.LineBasicMaterial>
  dots: THREE.Points<THREE.BufferGeometry, THREE.PointsMaterial>
  frames: TrailFrame[]
  lastPoint?: number[]
}
type RotationTrail = {
  angles: number[]
  center: number[]
  at: number
} & (CurveTrail | { kind: 'surface'; surface: RotationSurfaceTrail })
const TRAIL_FRAMES = 80 // 約 1 回転分（再生は毎秒 30°）

/** ホイールの1画素あたりの拡大縮小の量（距離の対数）。1目盛り（約 100 px）で e^0.22 ≈ 1.25 倍 */
/** 拡大で中心の点にここまで（図形の大きさに対する割合）近づいたら、そこからは一定の速さで前に進む（通り抜けられる） */
const NEAR_FRACTION = 0.08
/**
 * 2D 表示で拡大できる下限: 1 画素の大きさが、表示範囲の大きさのこの倍まで。
 * 座標は 32 ビット（刻みは値の大きさの約 6 × 10⁻⁸）で GPU に渡るので、これより先は線・軸が何画素ずつ飛んで動く
 * （表示範囲 ±10 なら 1 画素 3 × 10⁻⁶ で、座標 10 の近くの刻みはその 3 分の 1 画素）
 */
const FLAT_MIN_PIXEL = 1e-7
const WHEEL_PER_PIXEL = 0.0022
/** ホイールの拡大縮小をならす時間（秒） */
const WHEEL_SMOOTH_S = 0.1

/** 曲面の形を捨てる（粗い版があれば、それも） */
function disposeGeometry(geo: THREE.BufferGeometry) {
  ;(geo.userData.lod as THREE.BufferGeometry | undefined)?.dispose()
  geo.dispose()
}

export class Viewer {
  private renderer: THREE.WebGLRenderer
  /** 目盛りの数字・軸の名前 */
  private labels: TextLabel[] = []
  private readonly positionedLabels: TextLabel[] = []
  private readonly labelProjection = new THREE.Vector3()
  private readonly labelView = new THREE.Matrix4()
  private readonly lastLabelView = new THREE.Matrix4()
  private labelLayoutWidth = 0
  private labelLayoutHeight = 0
  private scene = new THREE.Scene()
  private camera = new THREE.PerspectiveCamera(45, 1, 0.01, 1000)
  private controls: OrbitControls
  private frame = new THREE.Group() // 軸・目盛り・枠
  private plots = new Map<number, PlotObject>() // 式ごとの曲面・曲線
  private rotationTrails = new Map<number, RotationTrail>()
  /**
   * 2D 表示で、画素ごとに描く曲線・領域（計画 12 章の G4）。式の番号 → 平面と材質と、その GLSL・表示範囲。
   * あるときは、plots の線・面を隠して（カーソルの下の点にはそれを使う）、こちらを描く
   */
  private gpu2d = new Map<number, { mesh: THREE.Mesh; mat: THREE.ShaderMaterial; glsl: string; key: string; eq: number; ready: boolean; params: string[] }>()

  /**
   * 3D の陰関数を光線で描く箱（計画 12 章の G5）。式の番号 → 箱と材質と、その GLSL・表示範囲。
   * あるときは、plots の面（メッシュ）を隠して（カーソルの下の点・切り口のふたにはそれを使う）、こちらを描く
   */
  private gpu3d = new Map<
    number,
    { mesh: THREE.Mesh; mat: THREE.MeshStandardMaterial; ray: RaycastUniforms; glsl: string; key: string; ready: boolean; axes: number[]; params: string[] }
  >()

  /** 座標系を使う式の、計算エンジンが返した形（移す前の位置と移した先）と、継ぎ目の頂点の組（s = 1 で初めて作る） */
  private morphSources = new Map<number, { mesh: MeshData; weld: Int32Array | null }>()
  /** 式ごとの移行のスライダーの値（0〜1。無ければ 0） */
  private morphValues = new Map<number, number>()
  /** 座標系の格子（計画 S39。座標系の番号 → 線と、元の位置・移した先・移行の値） */
  private coordGrids = new Map<number, { line: THREE.LineSegments; q: Float32Array; target: Float32Array; s: number }>()
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
  private resizeObserver: ResizeObserver
  /**
   * 描く所の大きさ（CSS の画素。resize で覚える）。毎コマの処理で container.clientWidth・clientHeight を読むと、
   * 画面の DOM が変わった直後には、その場でレイアウトを計算し直させてしまう（式の欄を書き換えている間など）
   */
  private viewW = 0
  private viewH = 0
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
  /**
   * 動かしている最中の描き方（画素密度・粗い版）。1コマにかかる時間を測って、遅いときだけ段階的に下げ、
   * 余裕があれば戻す（軽いグラフは止まっているときと同じ絵のまま。重いグラフでも、視点はなめらかに動かせる）
   */
  private quality = new MotionQuality()
  /** 図形が増えた・減った・差し替わった（quality に三角形の合計を伝え直す） */
  private sceneChanged = true
  /** 前の回（requestAnimationFrame）の時刻 */
  private lastTickAt = 0
  /** カメラが（ほぼ）止まってから何フレームたったか */
  private stillFrames = 0
  /** 前のフレームのカメラの位置と向き（止まったかどうかの判定に使う） */
  private lastCamPos = new THREE.Vector3()
  private lastCamQuat = new THREE.Quaternion()

  constructor(private container: HTMLElement, private hasResizableSheet = () => false) {
    // 描画の土台
    // ステンシルバッファは、媒介変数表示の立体の切り口のふたに使う
    this.renderer = new THREE.WebGLRenderer({ antialias: true, stencil: true })
    this.transparency = new SurfaceTransparency(this.renderer)
    this.renderer.setPixelRatio(this.basePixelRatio)
    this.renderer.localClippingEnabled = true // 表示範囲の外を切り落とすため
    container.appendChild(this.renderer.domElement)

    // 文字（目盛りの数字）用の重ね合わせ

    this.theme = LIGHT
    this.scene.background = new THREE.Color(this.theme.background)

    // カメラとマウス操作。操作モード（setControlMode）で切り替える
    // A: 左ドラッグ: 回転 / 右ドラッグ・Shift + 左ドラッグ: 移動 / ホイール・中ドラッグ: 拡大縮小
    // B: setupMouse を参照（Google Earth に近い操作）
    this.camera.up.set(0, 0, 1)
    this.controls = new OrbitControls(this.camera, this.renderer.domElement)
    this.setupMouse()
    this.setupWheel()
    this.setupPick()
    this.applyButtons()
    this.controls.enableDamping = true
    this.controls.dampingFactor = 0.1 // 指を離した後の慣性の回転を、少し早めに止める
    // 画素密度・粗い版は、動かし始めても すぐには下げない（1コマが遅いときだけ、loop の中で段階的に下げる）
    this.controls.addEventListener('start', () => {
      this.interacting = true
    })
    this.controls.addEventListener('end', () => (this.interacting = false))

    // 照明: 空と地面からの柔らかい光 + カメラと一緒に動く光（どの向きから見ても明るい）
    this.scene.add(new THREE.HemisphereLight(0xffffff, 0x8888aa, 1.4))
    const head = new THREE.DirectionalLight(0xffffff, 2.2)
    head.position.set(3, 4, 0)
    head.target.position.set(0, 0, -10)
    this.camera.add(head, head.target)
    this.scene.add(this.camera, this.frame)

    // WebGL のコンテキストが失われて戻ったとき（スマホでアプリを切り替えて戻った、GPU がリセットされたなど）は、
    // three.js が GPU のデータを作り直すので、描き直す（何も変わっていないときは描かないので、そのままだと真っ白になる）
    this.renderer.domElement.addEventListener('webglcontextrestored', () => {
      this.dirty = true
    })

    this.resizeObserver = new ResizeObserver(() => this.resize())
    this.resizeObserver.observe(container)
    this.resize()
    this.loop()
    // 開発中だけ: 操作のテスト（Playwright）からカメラの状態を読めるようにする（本番のビルドには入らない）
    if (import.meta.env.DEV) (window as unknown as { __viewer: Viewer }).__viewer = this
  }

  /** マウスの操作モード（A: 標準 / B: Google Earth に近い操作） */
  private controlMode: 'A' | 'B' = 'B'

  /** マウスの操作モードを変える */
  setControlMode(mode: 'A' | 'B') {
    this.controlMode = mode
    this.applyButtons()
  }

  /** OrbitControls に任せるボタンの割り当て（操作モードと 2D 表示で変わる） */
  private applyButtons() {
    const a = this.controlMode === 'A'
    this.controls.mouseButtons = {
      // B の左ドラッグは、A の Shift + 左ドラッグと同じ平行移動（OrbitControls に任せる）
      LEFT: this.flat || !a ? THREE.MOUSE.PAN : THREE.MOUSE.ROTATE,
      MIDDLE: a ? THREE.MOUSE.DOLLY : THREE.MOUSE.PAN,
      RIGHT: a ? THREE.MOUSE.PAN : THREE.MOUSE.DOLLY, // B の右ドラッグは setupMouse で受け取る
    }
    // B は回転の感度を下げる（1 では敏感すぎて操作しにくい）
    this.controls.rotateSpeed = a ? 1 : 0.5
  }

  /**
   * 操作モード B のマウス（パソコン）用の操作。OrbitControls より先に受け取る（操作 A は OrbitControls そのまま）。
   * - 左ドラッグ: 平行移動（A の Shift + 左ドラッグと同じ。OrbitControls に任せる）
   * - 右ドラッグ: 押した位置を中心に、上下で縮小・拡大、左右で回転
   * - Ctrl（Mac は ⌘）+ 左ドラッグ: カメラの位置を固定して、見る向きだけを変える（首を振る）
   * - 左ボタンを2回押して、2回目を押したままドラッグ・Shift + 左ドラッグ: 押した位置を中心に回転する
   *   （視点を動かして中心を画面の中央に持ってくることはしない。押した点はその場にとどまる）
   */
  private setupMouse() {
    const el = this.renderer.domElement
    type Drag = {
      x: number
      y: number
      id: number
      mode: 'look' | 'right' | 'orbit' | 'zoom'
      pivot: THREE.Vector3
      /** 右ドラッグで拡大縮小するときに進む向き（押したときのカーソルの向き） */
      fwd: THREE.Vector3
    }
    let last: Drag | null = null
    /** 前に左ボタンを押した時刻と位置（2回押しの判定に使う） */
    let prevLeft = { t: -1e9, x: 0, y: 0 }
    el.addEventListener(
      'pointerdown',
      (e) => {
        // スマホ（指）の操作は、パソコン用の調整（注視点の置き直し・慣性の停止・回転の感度）の前の挙動のまま、
        // OrbitControls にそのまま任せる（パソコン用の調整を入れたら、スマホでは操作しにくくなったため）
        if (e.pointerType === 'touch') {
          this.controls.rotateSpeed = 1
          return
        }
        this.applyButtons() // 回転の感度を、マウスの操作モードのものに戻す（指で操作した後かもしれない）
        // 操作 A（標準）の回転・移動は、パソコン用の調整（注視点の置き直し・Ctrl + ドラッグの見回し・慣性の停止）を
        // 入れる前の、OrbitControls そのままの操作にする。拡大縮小（中ドラッグ・ホイール）だけは B と同じ:
        // 押した位置に向かって拡大縮小し、近づききったら図形を通り抜けて進む
        if (this.controlMode === 'A' && e.button !== 1) return
        this.stopInertia() // 指を離した後の慣性の動きが残っていたら止める（新しい操作と混ざらないように）
        const look = e.button === 0 && (e.ctrlKey || e.metaKey) && !this.flat
        // 左ボタンの2回押し（0.35 秒以内・ほぼ同じ位置）
        const double =
          e.button === 0 &&
          e.timeStamp - prevLeft.t < 350 &&
          Math.hypot(e.clientX - prevLeft.x, e.clientY - prevLeft.y) < 8
        if (e.button === 0) prevLeft = double ? { t: -1e9, x: 0, y: 0 } : { t: e.timeStamp, x: e.clientX, y: e.clientY }
        let mode: Drag['mode']
        if (this.controlMode === 'A') mode = 'zoom' // 中ドラッグ
        else if (look) mode = 'look'
        else if (e.button === 2) mode = 'right'
        else if (e.button === 0 && (double || e.shiftKey) && !this.flat) mode = 'orbit'
        // 左ドラッグ・中ドラッグ: 平行移動なので、OrbitControls に任せる。
        // 移動の速さは注視点までの距離で決まるので、カーソルの下の図形の奥行きに注視点を置く（つかんだ点がカーソルについてくる）
        else {
          this.anchorTarget(e.clientX, e.clientY)
          return
        }
        e.stopImmediatePropagation() // OrbitControls には渡さない
        e.preventDefault()
        el.setPointerCapture(e.pointerId)
        const pivot =
          mode === 'look' ? new THREE.Vector3() : (this.pickContent(e.clientX, e.clientY) ?? this.controls.target.clone())
        const fwd = this.rayAt(e.clientX, e.clientY).ray.direction.clone()
        last = { x: e.clientX, y: e.clientY, id: e.pointerId, mode, pivot, fwd }
        this.interacting = true
      },
      { capture: true },
    )
    el.addEventListener('pointermove', (e) => {
      if (!last || e.pointerId !== last.id) return
      const dx = e.clientX - last.x
      const dy = e.clientY - last.y
      last = { ...last, x: e.clientX, y: e.clientY }
      if (last.mode === 'look') {
        this.look(dx, dy)
        return
      }
      if (last.mode === 'orbit') {
        // 左ドラッグの回転と同じ感度（画面の高さ分で半回転）
        const k = Math.PI / (this.container.clientHeight || 1)
        this.rotateAround(last.pivot, -dx * k)
        this.tiltAround(last.pivot, -dy * k)
        return
      }
      // 右ドラッグ: 上下は拡大縮小（100 px で約 1.6 倍）、左右は回転（2D 表示では回さない）。
      // 操作 A の中ドラッグは、上下の拡大縮小だけ
      this.zoomAround(last.pivot, Math.pow(0.995, dy), last.fwd)
      if (last.mode === 'right' && !this.flat) {
        const angle = (-dx * Math.PI) / (this.container.clientWidth || 1)
        this.rotateAround(last.pivot, angle)
        last.fwd.applyAxisAngle(this.camera.up.clone().normalize(), angle) // 進む向きもカメラと一緒に回す
      }
    })
    const end = (e: PointerEvent) => {
      if (!last || e.pointerId !== last.id) return
      // 見回した後は、注視点が何も無い所に向いているかもしれない。画面の中央の図形の奥行きに置き直す
      // （そのままだと、後の回転・拡大縮小が何も無い所を中心にして、感度もおかしくなる）
      if (last.mode === 'look') this.anchorTarget()
      last = null
      this.interacting = false
    }
    el.addEventListener('pointerup', end)
    el.addEventListener('pointercancel', end)
  }

  // ---- 点を押すと値が出る（学習用。計画のおすすめ 1）----
  //
  // グラフをクリック（タップ）すると、押した所に一番近い曲線の点・曲面の点・注目する点（極値・零点など）を探して、
  // onPick で画面側に知らせる。画面側は値を計算して showPick で印と札を出す。ドラッグ（視点の操作）は押したことにしない

  /** 式ごとの、注目する点（[x, y, z, 種類, …]）と、曲線の点の列（途切れる所に NaN） */
  private poiData = new Map<number, Float64Array>()
  private curvePts = new Map<number, Float32Array>()
  /** 押した点の印・その式の注目する点の印・札 */
  private pickMarker: THREE.Mesh | null = null
  private poiDots: THREE.InstancedMesh | null = null
  private pickLabel: HTMLDivElement | null = null
  private pickAt: THREE.Vector3 | null = null
  /** 印を出している式 */
  private pickFor: number | null = null
  /** 押した所の結果を受け取る（何も無い所なら null） */
  onPick: ((r: PickResult | null) => void) | null = null

  /** 押したときに探す、曲線の点の列と注目する点を覚える（applyPlot の後に呼ぶ。applyPlot は種類が変わると removePlot を呼ぶため） */
  private notePickData(id: number, mesh: MeshData) {
    if (mesh.curve && mesh.positions.length > 0) this.curvePts.set(id, mesh.positions)
    else this.curvePts.delete(id)
    this.setPoi(id, mesh.poi)
  }

  private setPoi(id: number, poi: Float64Array | undefined) {
    if (poi && poi.length >= 4) this.poiData.set(id, poi)
    else this.poiData.delete(id)
    if (this.pickFor === id) this.placePoiDots()
  }

  private setupPick() {
    const el = this.renderer.domElement
    let down: { x: number; y: number; t: number; id: number; n: number } | null = null
    let pointers = 0
    el.addEventListener('pointerdown', (e) => {
      pointers++
      down = e.button === 0 && !e.shiftKey && !e.ctrlKey && !e.metaKey && !e.altKey ? { x: e.clientX, y: e.clientY, t: e.timeStamp, id: e.pointerId, n: pointers } : null
    })
    const up = (e: PointerEvent) => {
      pointers = Math.max(0, pointers - 1)
      const d = down
      down = null
      // 動かさずに短く押して離したときだけ（2 本指の操作・ドラッグは除く）
      if (!d || d.id !== e.pointerId || d.n > 1 || e.timeStamp - d.t > 450 || Math.hypot(e.clientX - d.x, e.clientY - d.y) > 5) return
      this.onPick?.(this.pickAtScreen(e.clientX, e.clientY))
    }
    el.addEventListener('pointerup', up)
    el.addEventListener('pointercancel', () => {
      pointers = Math.max(0, pointers - 1)
      down = null
    })
  }

  /** 空間の点の、画面の上の位置（CSS ピクセル。描く所の左上から）。カメラの後ろなら null */
  private toScreen(p: THREE.Vector3): { x: number; y: number } | null {
    const v = p.clone().project(this.camera)
    if (v.z > 1 || v.z < -1) return null
    return { x: ((v.x + 1) / 2) * this.viewW, y: ((1 - v.y) / 2) * this.viewH }
  }

  /** 画面の上の点 (clientX, clientY) にある、式の点（注目する点 → 曲線 → 曲面の順に探す） */
  pickAtScreen(clientX: number, clientY: number): PickResult | null {
    const rect = this.renderer.domElement.getBoundingClientRect()
    const sx = clientX - rect.left
    const sy = clientY - rect.top
    const shown = (id: number) => this.styles.get(id)?.visible !== false
    const inside = (pt: THREE.Vector3) => this.clipPlanes.every((pl) => pl.distanceToPoint(pt) >= -1e-6)
    // 注目する点（印を出している式の点は灰色の球が見えているので 12 画素以内、ほかの式の点は 6 画素以内）
    let best: { r: PickResult; d: number } | null = null
    for (const [id, poi] of this.poiData) {
      if (!shown(id)) continue
      const near = id === this.pickFor ? 12 : 6
      for (let k = 0; k + 3 < poi.length; k += 4) {
        const p = new THREE.Vector3(poi[k], poi[k + 1], this.flat ? 0 : poi[k + 2])
        const q = this.toScreen(p)
        if (!q) continue
        const d = Math.hypot(q.x - sx, q.y - sy)
        if (d <= near && (!best || d < best.d)) best = { r: { id, point: [poi[k], poi[k + 1], poi[k + 2]], poi: poi[k + 3] }, d }
      }
    }
    if (best) {
      // 同じ点にある注目する点の種類をまとめる（原点が零点でも縦軸との交点でもある、など）
      const poi = this.poiData.get(best.r.id)!
      const [x, y, z] = best.r.point
      const kinds: number[] = []
      for (let k = 0; k + 3 < poi.length; k += 4) {
        const same = Math.abs(poi[k] - x) + Math.abs(poi[k + 1] - y) + Math.abs(poi[k + 2] - z) <= 1e-9 * (1 + Math.abs(x) + Math.abs(y) + Math.abs(z))
        if (same && !kinds.includes(poi[k + 3])) kinds.push(poi[k + 3])
      }
      return { ...best.r, kinds: kinds.sort((a, b) => a - b) }
    }
    // 曲線（10 画素以内の、一番近い線分の上の点）
    for (const [id, pts] of this.curvePts) {
      const obj = this.plots.get(id)
      if (!shown(id) || !obj || obj.kind !== 'curve') continue
      obj.line.updateMatrixWorld()
      const m = obj.line.matrixWorld
      let prev: { p: THREE.Vector3; q: { x: number; y: number } | null } | null = null
      for (let k = 0; k + 2 < pts.length; k += 3) {
        if (!(Number.isFinite(pts[k]) && Number.isFinite(pts[k + 1]) && Number.isFinite(pts[k + 2]))) {
          prev = null
          continue
        }
        const p = new THREE.Vector3(pts[k], pts[k + 1], pts[k + 2]).applyMatrix4(m)
        const q = this.toScreen(p)
        if (prev && prev.q && q && inside(p)) {
          const [ax, ay, bx, by] = [prev.q.x, prev.q.y, q.x, q.y]
          const L = (bx - ax) ** 2 + (by - ay) ** 2
          const t = L > 0 ? Math.min(1, Math.max(0, ((sx - ax) * (bx - ax) + (sy - ay) * (by - ay)) / L)) : 0
          const d = Math.hypot(ax + t * (bx - ax) - sx, ay + t * (by - ay) - sy)
          if (d <= 10 && (!best || d < best.d)) {
            const w = prev.p.clone().lerp(p, t)
            best = { r: { id, point: [w.x, w.y, w.z] }, d }
          }
        }
        prev = { p, q }
      }
    }
    if (best) return best.r
    // 曲面（視線と三角形の交点のうち、表示範囲の中で一番手前）
    const ray = this.rayAt(clientX, clientY)
    const meshes: THREE.Object3D[] = []
    const owner = new Map<THREE.Object3D, number>()
    for (const [id, p] of this.plots) {
      if (p.kind === 'surface' && shown(id) && p.group.children[0]) {
        meshes.push(p.group.children[0])
        owner.set(p.group.children[0], id)
      }
    }
    for (const hit of ray.intersectObjects(meshes, false)) {
      if (!inside(hit.point)) continue
      const id = owner.get(hit.object)
      if (id !== undefined) return { id, point: [hit.point.x, hit.point.y, hit.point.z] }
    }
    return null
  }

  /** 表示範囲の大きさから決める、印の球の半径 */
  private markRadius(k: number) {
    const d = this.domain
    const span = d ? Math.max(d.xmax - d.xmin, d.ymax - d.ymin, this.flat ? 0 : d.zmax - d.zmin) : 10
    return span * k
  }

  /**
   * 押した点の印と札を出す（point が null なら消す）。`id` の式の注目する点も、灰色の小さな球で出す。
   * 札の中身は HTML ではなく文字（textContent）で入れる
   */
  showPick(point: [number, number, number] | null, text = '', color = '#222222', id: number | null = null) {
    this.dirty = true
    if (!point) {
      if (this.pickMarker) {
        this.scene.remove(this.pickMarker) // 球の形は共有なので捨てない
        ;(this.pickMarker.material as THREE.Material).dispose()
        this.pickMarker = null
      }
      this.pickAt = null
      this.pickFor = null
      this.placePoiDots()
      if (this.pickLabel) this.pickLabel.hidden = true
      return
    }
    const p = new THREE.Vector3(point[0], point[1], this.flat ? 0.001 : point[2])
    if (!this.pickMarker) {
      // 面に半分埋まらないよう、いつも手前に描く
      // （半透明の扱いにして、ほかの全部を描いた後に描く。不透明のままだと、後から描く面に上書きされる）
      this.pickMarker = new THREE.Mesh(dotSphere(), new THREE.MeshStandardMaterial({ roughness: 0.4, metalness: 0.05, depthTest: false, depthWrite: false, transparent: true }))
      this.pickMarker.renderOrder = 1e6
      this.scene.add(this.pickMarker)
    }
    // 曲面の上の点は、面と同じ色だと見えないので、濃い色にする
    const onSurface = id !== null && this.plots.get(id)?.kind === 'surface'
    ;(this.pickMarker.material as THREE.MeshStandardMaterial).color.set(onSurface ? '#1b1b1f' : color)
    const r = this.markRadius(0.012)
    this.pickMarker.scale.set(r, r, r)
    this.pickMarker.position.copy(p)
    this.pickAt = p
    this.pickFor = id
    this.placePoiDots()
    if (!this.pickLabel) {
      const l = document.createElement('div')
      l.className = 'pick-label'
      Object.assign(l.style, {
        position: 'absolute',
        left: '0',
        top: '0',
        pointerEvents: 'none',
        padding: '4px 9px',
        borderRadius: '6px',
        font: '13px/1.4 var(--mono, monospace)',
        whiteSpace: 'pre',
        background: 'var(--panel, #fff)',
        color: 'var(--text, #222)',
        border: '1px solid var(--border-strong, #ccc)',
        boxShadow: '0 2px 8px rgb(0 0 0 / 0.18)',
        zIndex: '5',
      })
      this.container.style.position ||= 'relative'
      this.container.appendChild(l)
      this.pickLabel = l
    }
    this.pickLabel.textContent = text
    this.pickLabel.style.borderLeft = `4px solid ${color}`
    this.pickLabel.hidden = false
    this.placePickLabel()
  }

  /** 印を出している式の注目する点を、灰色の小さな球で置く */
  private placePoiDots() {
    if (this.poiDots) {
      this.scene.remove(this.poiDots)
      this.poiDots.dispose()
      ;(this.poiDots.material as THREE.Material).dispose()
      this.poiDots = null
    }
    const poi = this.pickFor === null ? undefined : this.poiData.get(this.pickFor)
    if (!poi) return
    const n = Math.floor(poi.length / 4)
    const dots = new THREE.InstancedMesh(dotSphere(), new THREE.MeshStandardMaterial({ color: 0x8a8f98, roughness: 0.5 }), n)
    dots.frustumCulled = false
    dots.raycast = () => {}
    const r = this.markRadius(0.008)
    const m = new THREE.Matrix4()
    for (let i = 0; i < n; i++) {
      m.makeScale(r, r, r).setPosition(poi[4 * i], poi[4 * i + 1], this.flat ? 0.0005 : poi[4 * i + 2])
      dots.setMatrixAt(i, m)
    }
    dots.instanceMatrix.needsUpdate = true
    this.scene.add(dots)
    this.poiDots = dots
    this.dirty = true
  }

  /** 札を、押した点の右上に置く（視点が動いたら付いていく） */
  private placePickLabel() {
    const l = this.pickLabel
    if (!l || l.hidden || !this.pickAt) return
    const q = this.toScreen(this.pickAt)
    if (!q || q.x < 0 || q.y < 0 || q.x > this.viewW || q.y > this.viewH) {
      l.style.visibility = 'hidden'
      return
    }
    l.style.visibility = 'visible'
    const w = l.offsetWidth
    const h = l.offsetHeight
    // 右上に置けなければ、左・下にずらす
    const x = q.x + 12 + w > this.viewW ? q.x - 12 - w : q.x + 12
    const y = q.y - 12 - h < 0 ? q.y + 12 : q.y - 12 - h
    l.style.transform = `translate(${Math.round(x)}px, ${Math.round(y)}px)`
  }

  /** 点 pivot を通る、カメラの右向きの軸の周りに、カメラと注視点を angle（ラジアン）回す（見下ろす角度を変える）。
   *  真上・真下を越えないようにする */
  private tiltAround(pivot: THREE.Vector3, angle: number) {
    const up = this.camera.up.clone().normalize()
    const target = this.controls.target
    const dir = target.clone().sub(this.camera.position).normalize()
    const right = new THREE.Vector3().crossVectors(dir, up).normalize()
    const pitch = Math.asin(THREE.MathUtils.clamp(dir.dot(up), -1, 1))
    const limit = THREE.MathUtils.degToRad(88)
    const next = THREE.MathUtils.clamp(pitch + angle, -limit, limit)
    const a = next - pitch
    this.camera.position.sub(pivot).applyAxisAngle(right, a).add(pivot)
    target.sub(pivot).applyAxisAngle(right, a).add(pivot)
    this.camera.lookAt(target)
    this.dirty = true
  }

  /**
   * 画面上の点 (clientX, clientY) にある図形の点。曲面が無ければ xy 平面（z = 0。z の表示範囲が 0 を含まなければ一番下の面）との交点。
   * 交点が無い（上を向いている）か遠すぎる（地平線の近く）なら null。
   * 表示範囲の箱の側面・上面は使わない（押した所によって中心が箱の壁に飛び、操作しにくいため）。
   * 以前の「注視点を通る水平な面」は、Ctrl + ドラッグで見回した後はカメラと同じ高さになり、
   * 交点がカメラのすぐ近くになって、回転が「その場で首を振る」動きになっていた
   */
  private pickContent(clientX: number, clientY: number): THREE.Vector3 | null {
    const ray = this.rayAt(clientX, clientY)
    const meshes: THREE.Object3D[] = []
    for (const p of this.plots.values()) {
      if (p.kind === 'surface' && p.group.visible && p.group.children[0]) meshes.push(p.group.children[0])
    }
    const inside = (pt: THREE.Vector3) => this.clipPlanes.every((pl) => pl.distanceToPoint(pt) >= -1e-6)
    for (const hit of ray.intersectObjects(meshes, false)) {
      if (inside(hit.point)) return hit.point.clone()
    }
    // z の表示範囲が 0 を含まないなら、表示範囲の一番下の面にする（2D 表示では図形は z = 0 にある）
    const d = this.domain
    const z0 = this.flat || !d || (d.zmin <= 0 && 0 <= d.zmax) ? 0 : d.zmin
    const q = ray.ray.intersectPlane(new THREE.Plane(new THREE.Vector3(0, 0, 1), -z0), new THREE.Vector3())
    if (!q) return null
    // 地平線の近くでは交点がはるか遠くになり、そこを中心に回すと大きく振り回される
    const far = this.camera.position.distanceTo(this.controls.target) + 3 * this.sceneSize()
    return q.distanceTo(this.camera.position) <= far ? q : null
  }

  /**
   * 注視点（回転・拡大縮小の中心、平行移動の速さの基準）を、見ている向きは変えずに、
   * 画面上の点 (clientX, clientY)（省略なら画面の中央）にある図形の奥行きに置き直す。見た目は変わらない。
   * OrbitControls の平行移動は注視点までの距離に比例した速さなので、つかんだ点の奥行きにすると、その点がカーソルについてくる
   * （注視点が図形よりずっと奥や手前にあると、感度が高すぎ・低すぎになる）
   */
  private anchorTarget(clientX?: number, clientY?: number) {
    if (clientX === undefined || clientY === undefined) {
      const r = this.renderer.domElement.getBoundingClientRect()
      clientX = r.left + r.width / 2
      clientY = r.top + r.height / 2
    }
    const p = this.pickContent(clientX, clientY)
    if (!p) return
    const eye = this.camera.position
    const dir = this.controls.target.clone().sub(eye).normalize()
    const depth = p.clone().sub(eye).dot(dir)
    if (!(depth > this.sceneSize() * 1e-3)) return
    this.controls.target.copy(eye).addScaledVector(dir, depth)
    this.controls.update()
  }

  /** OrbitControls の慣性（指を離した後もしばらく回り続ける・動き続ける分）を止める */
  private stopInertia() {
    const c = this.controls as unknown as { _sphericalDelta?: THREE.Spherical; _panOffset?: THREE.Vector3; _scale?: number }
    c._sphericalDelta?.set(0, 0, 0)
    c._panOffset?.set(0, 0, 0)
    if (c._scale !== undefined) c._scale = 1
  }

  /** つかんで動かす面: 上から見下ろしているなら点 p を通る水平な面、横から見ているなら画面に平行な面 */
  private grabPlane(p: THREE.Vector3): THREE.Plane {
    const up = this.camera.up.clone().normalize()
    const dir = this.controls.target.clone().sub(this.camera.position).normalize()
    const normal = Math.abs(dir.dot(up)) > 0.25 ? up : dir
    return new THREE.Plane().setFromNormalAndCoplanarPoint(normal, p)
  }

  /** つかんだ点 p が、カーソルの位置 (clientX, clientY) の下に来るように視点を平行に動かす */
  private grab(p: THREE.Vector3, plane: THREE.Plane, clientX: number, clientY: number) {
    const q = this.rayAt(clientX, clientY).ray.intersectPlane(plane, new THREE.Vector3())
    if (!q) return
    const delta = p.clone().sub(q)
    // 地平線の近くでは、少しのカーソルの動きで遠くまで飛ぶので、1回で動かす量を抑える
    const max = this.camera.position.distanceTo(this.controls.target) * 0.3 // 遠くから見ているときは大きく動けるように
    if (delta.length() > max) delta.setLength(max)
    this.camera.position.add(delta)
    this.controls.target.add(delta)
    this.dirty = true
  }

  /** 画面上の点 (clientX, clientY) を通る視線 */
  private rayAt(clientX: number, clientY: number): THREE.Raycaster {
    const rect = this.renderer.domElement.getBoundingClientRect()
    const ndc = new THREE.Vector2(
      ((clientX - rect.left) / rect.width) * 2 - 1,
      -((clientY - rect.top) / rect.height) * 2 + 1,
    )
    this.camera.updateMatrixWorld()
    const ray = new THREE.Raycaster()
    ray.setFromCamera(ndc, this.camera)
    return ray
  }

  // ---- ホイールの拡大縮小（滑らかに動かす）----
  //
  // OrbitControls はホイールの1目盛りごとに一気に拡大縮小するので、ふつうのマウス（1目盛りが大きい）ではカクカクする。
  // 目盛りの分を「これから動かす量」としてため、毎フレーム少しずつ（指数的に近づけて）動かす

  /** これから動かす拡大縮小の量（距離の対数。正なら遠ざかる） */
  private wheelPending = 0
  private wheelLastTime = 0
  /** ホイールを回している間は、この時刻まで画素密度を下げたままにする
   *  （目盛りの合間に元の密度に戻ると、描き直しが重くなり、くっきり ⇔ ぼやけが切り替わってカクつく） */
  private wheelHoldUntil = 0

  private setupWheel() {
    this.renderer.domElement.addEventListener(
      'wheel',
      (e) => {
        // 操作 A（標準）のホイールも B と同じ（目盛りの分をためて滑らかに動かす。図形を通り抜けて進める）
        e.preventDefault()
        e.stopImmediatePropagation() // OrbitControls には渡さない
        // 行・ページ単位のホイールは画素に直す
        const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? this.container.clientHeight || 800 : 1
        let d = e.deltaY * unit
        if (e.ctrlKey) d *= 4 // トラックパッドのピンチ（値が小さい）
        // 1目盛り（約 100 px）で約 1.25 倍。一度に大きく動きすぎないよう、ためる量に上限をつける
        this.wheelPending = THREE.MathUtils.clamp(this.wheelPending + d * WHEEL_PER_PIXEL, -3, 3)
        if (this.wheelLastTime === 0) {
          // 回し始め: 拡大縮小の量は注視点までの距離に比例するので、画面の中央の図形の奥行きに注視点を置く
          // （見回した後などで注視点が何も無い遠くにあると、1目盛りで飛びすぎる）
          this.stopInertia()
          this.anchorTarget()
          this.wheelLastTime = performance.now()
        }
        this.wheelHoldUntil = performance.now() + 350
        this.dirty = true
      },
      { capture: true, passive: false },
    )
  }

  /** 毎フレーム: ためた拡大縮小を少しずつ進める（0.3 秒ほどで大部分を動かす） */
  private stepWheelZoom() {
    if (this.wheelPending === 0) {
      this.wheelLastTime = 0
      return
    }
    const now = performance.now()
    // 描くのが遅いコマでも、時間に見合った量だけ進める（上限が小さいと、重いグラフほど拡大縮小が遅くなる）
    const dt = Math.min(0.25, (now - this.wheelLastTime) / 1000)
    this.wheelLastTime = now
    // ならす時間 0.1 秒。長いと遅れて感じ、短すぎると目盛りごとに「急に動いて止まる」をくり返して波打って見える
    let step = this.wheelPending * (1 - Math.exp(-dt / WHEEL_SMOOTH_S))
    // 残りがごくわずかになったら、最後まで動かして終える（いつまでも微小に動き続けないように）
    if (Math.abs(this.wheelPending) < 2e-3) step = this.wheelPending
    this.wheelPending -= step
    this.zoomAround(this.controls.target.clone(), Math.exp(step))
  }

  /** 点 pivot に向かって、カメラと注視点を factor 倍の距離にする（1 より小さいと近づいて拡大） */
  private zoomAround(pivot: THREE.Vector3, factor: number, forward?: THREE.Vector3) {
    const target = this.controls.target
    const eye = this.camera.position
    const size = this.sceneSize()
    // 進む向き（省略なら中心の点の向き）。右ドラッグでは押したときのカーソルの向きに固定する
    // （中心の点の向きにすると、点を通り過ぎたとたんに向きが反転し、点の周りを行ったり来たりして止まる）
    const fwd = (forward ?? pivot.clone().sub(eye)).clone().normalize()
    if (fwd.lengthSq() === 0) fwd.copy(target).sub(eye).normalize()
    /** 中心の点の、進む向きに沿った奥行き（通り過ぎたら負） */
    const ahead = pivot.clone().sub(eye).dot(fwd)
    const floor = size * NEAR_FRACTION
    // 2D 表示: 平面を通り抜けると何も見えなくなる（カメラが平面の向こうへ出て、描く奥行きの範囲からも外れる）。
    // 通り抜けずに、注視点に向かって倍率どおりに近づく。近づける下限は、32 ビットの浮動小数点数で座標の違いが
    // 見分けられる所まで（1 画素が表示範囲の大きさの FLAT_MIN_PIXEL 倍）
    if (this.flat) {
      const dist = eye.distanceTo(target)
      const minDist = (size * FLAT_MIN_PIXEL) / (this.pixelAt(dist) / dist)
      const next = THREE.MathUtils.clamp(dist * factor, minDist, (size * 200) / this.perspective)
      const f = next / dist
      eye.sub(pivot).multiplyScalar(f).add(pivot)
      target.sub(pivot).multiplyScalar(f).add(pivot)
      this.dirty = true
      return
    }
    // 中心の点までの距離を factor 倍にするだけだと、近づくほど1回に進む量が小さくなり、
    // 図形の手前の見えない壁で止まったようになる（通り抜けられない）。
    // 近づききったら（図形の大きさの NEAR_FRACTION まで来たら・通り過ぎたら）、そこからは一定の速さで前後に進む
    if (ahead * factor < floor) {
      const move = -floor * Math.log(factor) // 拡大なら前へ、縮小なら後ろへ
      eye.addScaledVector(fwd, move)
      target.addScaledVector(fwd, move)
      // 注視点（回転の中心）がカメラのすぐ近く・後ろにならないよう、見ている向きの前に離しておく
      const view = target.clone().sub(eye)
      if (view.dot(fwd) < floor) target.copy(eye).addScaledVector(view.lengthSq() > 0 ? view.normalize() : fwd, floor)
      this.dirty = true
      return
    }
    // 遠ざかりすぎないようにする
    const dist = eye.distanceTo(target)
    const next = Math.min(dist * factor, (size * 200) / this.perspective)
    const f = next / dist
    eye.sub(pivot).multiplyScalar(f).add(pivot)
    target.sub(pivot).multiplyScalar(f).add(pivot)
    this.dirty = true
  }

  /** 点 pivot を通る縦の軸の周りに、カメラと注視点を angle（ラジアン）回す */
  private rotateAround(pivot: THREE.Vector3, angle: number) {
    const up = this.camera.up.clone().normalize()
    const target = this.controls.target
    this.camera.position.sub(pivot).applyAxisAngle(up, angle).add(pivot)
    target.sub(pivot).applyAxisAngle(up, angle).add(pivot)
    this.camera.lookAt(target)
    this.dirty = true
  }

  /** 見る向きを、画面上で (dx, dy) px ドラッグした分だけ変える（カメラの位置は動かさない） */
  private look(dx: number, dy: number) {
    const h = this.container.clientHeight || 1
    // 画面の高さ分のドラッグで、視野角と同じくらい向きが変わる
    const k = (THREE.MathUtils.degToRad(this.camera.fov) * 1.0) / h
    const eye = this.camera.position
    const dir = this.controls.target.clone().sub(eye)
    const up = this.camera.up.clone().normalize()
    // 左右: 上向きの軸の周りに回す（マウスを右に動かすと左を向き、左に動かすと右を向く）
    dir.applyAxisAngle(up, dx * k)
    // 上下: カメラの右向きの軸の周りに回す。真上・真下を越えないようにする
    const right = new THREE.Vector3().crossVectors(dir, up).normalize()
    const pitch = Math.asin(THREE.MathUtils.clamp(dir.clone().normalize().dot(up), -1, 1))
    const limit = THREE.MathUtils.degToRad(85)
    // マウスを上に動かすと下を向き、下に動かすと上を向く
    const next = THREE.MathUtils.clamp(pitch + dy * k, -limit, limit)
    dir.applyAxisAngle(right, next - pitch)
    this.controls.target.copy(eye).add(dir)
    this.camera.lookAt(this.controls.target)
    this.dirty = true
  }

  dispose() {
    this.stopped = true
    this.clearRotationTrails()
    this.resizeObserver.disconnect()
    this.controls.dispose()
    this.transparency.dispose()
    this.renderer.dispose()
    this.container.replaceChildren()
  }

  /** 次のフレームで描き直す */
  invalidate() {
    this.dirty = true
  }

  /**
   * スライダーを動かしている間など、グラフが次々に変わる間は true にする。
   * その間は操作の最中とみなす（段階を引き継ぐ）が、視点が止まっていれば元の画素密度・そのままの版で描く
   * （applyQuality を参照。1コマの時間は、カメラが動いているコマだけで数える）
   */
  setBusy(on: boolean) {
    if (this.busy === on) return
    this.busy = on
    this.updateOperation()
  }

  /**
   * 視点（カメラ）が動いているかどうかが変わったとき。動いている間、段階に応じて三角形の多い曲面は粗い版を描く。
   * スライダーや回転の再生でグラフの形が変わっているだけのときは、粗い版にしない
   * （遅いのは計算で、描くのを軽くしても速くならないため）
   */
  private setCamMoving(on: boolean) {
    if (this.camMoving === on) return
    this.camMoving = on
    this.updateOperation()
  }
  private camMoving = false

  /** 操作（視点の移動・スライダー・再生）の始まりと終わりを quality に伝え、描き方を合わせる */
  private updateOperation() {
    if (this.camMoving || this.busy) this.quality.begin()
    else this.quality.end()
    this.applyQuality()
  }

  /**
   * 今の段階の描き方にする。視点が動いていなければ、元の画素密度・そのままの版
   * （止まったら1回描き直す）。
   * スライダー・回転の再生でグラフの形が変わっているだけ（視点は止まっている）ときも、画素密度は下げない。
   * 軸・枠・方眼は動かないのにぼやけて見えるうえ、遅いのは計算なので、描くのを軽くしても速くならない
   * （粗い版にしないのと同じ理由）
   */
  private applyQuality() {
    const ratio = this.camMoving ? this.motionStage().ratio : this.basePixelRatio
    if (this.renderer.getPixelRatio() !== ratio) this.renderer.setPixelRatio(ratio)
    this.applyLineScale(ratio / this.basePixelRatio)
    for (const p of this.plots.values()) this.applyLod(p)
    for (const g of this.gpu3d.values()) this.applyRayBudget(g.ray)
    this.dirty = true
  }

  /** 光線の回数の上限: 視点を動かしている間は軽くする（止まったら元に戻して描き直す） */
  private applyRayBudget(ray: RaycastUniforms) {
    const b = this.camMoving ? RAY_MOVING : RAY_STILL
    ray.uMaxIter.value = b.maxIter
    ray.uWork.value = b.work
  }

  /** 格子線の太さの倍率（全部の曲面の材質で共有する。今の画素密度 ÷ ふつうのときの画素密度） */
  private pixelScale = { value: 1 }
  /** 表示範囲の箱（曲面の材質で共有する。箱の壁の格子線を、面の濃さと一緒に薄くするのに使う。2D では z の壁は無い） */
  private boxUniforms = { lo: { value: new THREE.Vector3(-1e30, -1e30, -1e30) }, hi: { value: new THREE.Vector3(1e30, 1e30, 1e30) } }

  /** 曲面の材質の箱を、今の表示範囲にする */
  private updateBoxUniforms() {
    const d = this.domain
    if (!d) return
    const far = 1e30
    this.boxUniforms.lo.value.set(d.xmin, d.ymin, this.flat ? -far : d.zmin)
    this.boxUniforms.hi.value.set(d.xmax, d.ymax, this.flat ? far : d.zmax)
  }

  /**
   * 画素密度を下げている間も、線が太って見えないようにする。
   * 曲面の格子線はシェーダーで細くする。枠・方眼・軸の線（WebGL の線は 1 画素より細くできない）は、
   * 1 画素のまま半透明にして、画面の上での濃さ（太さ × 濃さ）をそろえる
   * （背景の色に近づけるのでは、曲面の手前にある線が白く太って見える）
   */
  private applyLineScale(scale: number) {
    this.pixelScale.value = scale
    const opacity = Math.min(1, scale)
    this.frame.traverse((o) => {
      if (!(o instanceof THREE.LineSegments)) return
      const m = o.material as THREE.LineBasicMaterial
      const transparent = opacity < 1
      if (m.transparent !== transparent) {
        m.transparent = transparent
        m.needsUpdate = true
      }
      m.opacity = opacity
    })
  }

  /** 場面の三角形の合計と、粗い版があるかを quality に伝え、粗い版の段階で粗くする面を決める（重い面から順に） */
  private updateQualityScene() {
    let triangles = 0
    let hasLod = false
    const shown: Extract<PlotObject, { kind: 'surface' }>[] = []
    for (const p of this.plots.values()) {
      if (p.kind !== 'surface' || !p.group.visible) continue
      shown.push(p)
      triangles += (p.geo.index?.count ?? p.geo.getAttribute('position')?.count ?? 0) / 3
      if (p.geo.userData.lod) hasLod = true
    }
    const size = (p: (typeof shown)[number]) => (p.geo.index?.count ?? p.geo.getAttribute('position')?.count ?? 0) / 3
    const pick = pickLod(shown.map(size), shown.map((p) => !!p.geo.userData.lod))
    this.lodPlots = new Set(shown.filter((_, k) => pick[k]))
    this.quality.setScene(this.basePixelRatio, triangles, hasLod && this.lodPlots.size > 0)
  }

  /** 動かしている間の画質の設定（auto・sharp・full。motionQuality.ts の MotionMode） */
  setMotionMode(mode: MotionMode) {
    this.quality.setMode(mode)
    this.sceneChanged = true
  }

  /** 粗い版の段階で粗い版にする面（重い面だけ。ほかの面は動かしている間もそのままの版） */
  private lodPlots = new Set<PlotObject>()

  /** 曲面の形を、動かしている間は（段階が粗い版で、重い面なら）粗い版、それ以外はそのままの版にする */
  private applyLod(p: PlotObject) {
    if (p.kind !== 'surface') return
    const lod = p.geo.userData.lod as THREE.BufferGeometry | undefined
    // ベンチマークで描き方を決めているときは、粗い版のある面を全部粗くする（三角形と画素のどちらが重いかを見るため）
    const heavy = this.benchStage ? true : this.lodPlots.has(p)
    const geo = this.camMoving && this.motionStage().lod && lod && heavy ? lod : p.geo
    const main = p.group.children[0] as THREE.Mesh
    if (main.geometry !== geo) main.geometry = geo
  }

  private resize() {
    const w = this.container.clientWidth
    const h = this.container.clientHeight
    this.viewW = w
    this.viewH = h
    if (w === 0 || h === 0) return
    this.dirty = true
    this.camera.aspect = w / h
    this.renderer.setSize(w, h)
    for (const p of this.plots.values()) {
      if (p.kind === 'curve') (p.line.material as LineMaterial).resolution.set(w, h)
      else for (const l of p.outlines) (l.material as LineMaterial).resolution.set(w, h)
    }
    for (const { line } of this.limitSources.values()) line.material.resolution.set(w, h)
    this.updateProjection()
  }

  private verticalHalfAngleTan() {
    // スマホの 2D では式一覧を伸縮しても、横方向の表示範囲を保つ。
    return this.flat && this.hasResizableSheet() ? this.perspective / this.camera.aspect : this.perspective
  }

  private updateProjection() {
    this.camera.fov = THREE.MathUtils.radToDeg(2 * Math.atan(this.verticalHalfAngleTan()))
    this.camera.updateProjectionMatrix()
  }

  private loop = () => {
    if (this.stopped) return
    requestAnimationFrame(this.loop)
    // 前の回に描いたなら、そこから今回までの時間が、その1コマにかかった時間
    // （描くのが遅いと、次の回が呼ばれるのが遅れる。今回は描かないとしても数える）
    const now = performance.now()
    if (this.sceneChanged) {
      this.sceneChanged = false
      this.updateQualityScene()
      this.applyQuality()
    }
    // 視点が動いているコマだけ数える（スライダーだけのときは、計算待ちで描いていないだけのことが多い）。
    // 長すぎる間隔（タブが隠れていた など）は数えない
    const dt = now - this.lastTickAt
    // 形を差し替えたコマ（データの受け渡し・GPU への転送の時間）と、視点が跳んだコマは数えない。
    // 数えると、回転の再生中に視点を動かしたり、2D / 3D を切り替えたりしただけで「描くのが遅い」とみなして
    // 画素密度を一番下まで下げ、その段階を次の操作に持ち越してしまう
    // （差し替えの処理はこの回までの間に、転送はその次に描くコマにかかるので、両方を外す）
    const counted = this.renderedLastTick && !this.notMotionLastTick && !this.notMotion
    if (counted && this.camMoving && dt <= 2500 && this.quality.frame(dt)) this.applyQuality()
    this.lastTickAt = now
    this.stepWheelZoom()
    // カメラが動いたか（慣性で動き続けている間も true）
    if (this.controls.update()) this.dirty = true
    // 目に見えるほど動いたか。慣性の終わりのごくわずかな動きは「止まった」とみなす
    const dist = this.camera.position.distanceTo(this.controls.target)
    // OrbitControls の update() は、動いた距離が決まった値（0.001）を超えたときだけ true を返す。
    // 深く拡大すると 1 画素がそれよりずっと小さく、小さい動きが描かれずにたまって、何画素もまとめて飛んでいた。
    // 1 画素の 1/100 以上動いたら描き直す（遠近感が弱いとカメラはとても遠く、距離に比べた割合では粗すぎる）
    if (this.camera.position.distanceTo(this.lastCamPos) > this.pixelAt(dist) * 0.01 || this.camera.quaternion.angleTo(this.lastCamQuat) > 1e-6) this.dirty = true
    const visible =
      this.camera.position.distanceTo(this.lastCamPos) > dist * 3e-4 ||
      this.camera.quaternion.angleTo(this.lastCamQuat) > 3e-4
    this.lastCamPos.copy(this.camera.position)
    this.lastCamQuat.copy(this.camera.quaternion)
    const idle = !this.interacting && this.wheelPending === 0 && performance.now() > this.wheelHoldUntil
    if (visible) {
      this.stillFrames = 0
      this.setCamMoving(true)
    } else if (++this.stillFrames > 4 && idle) {
      // 視点が止まった（グラフの形は、まだ変わり続けているかもしれない）。
      // 何も動いていなければ、元の画素密度・そのままの版で描き直す
      this.setCamMoving(false)
    }
    // 何も変わっていなければ描かない（止まっているときに電池を使わない）
    if (!this.dirty) {
      this.renderedLastTick = false
      return
    }
    this.dirty = false
    this.updateClipRange()
    this.updateLabelSizes()
    this.updateStencilSides()
    const renderedBefore = this.renderedLastTick
    this.renderedLastTick = true
    this.notMotionLastTick = this.notMotion
    this.notMotion = false
    if (this.frameLog) this.renderMeasured(renderedBefore ? dt : null)
    else this.renderScene()
    this.placePickLabel()
  }
  private renderedLastTick = false
  /** 前に描いてから今までに、形の差し替え・視点の跳びがあったか（そのコマの時間は quality に数えない） */
  private notMotion = false
  /** 前の回に描いたコマが、形の差し替え・視点の跳びを含んでいたか */
  private notMotionLastTick = false
  private readonly transparency: SurfaceTransparency

  private renderScene() {
    if (this.flat) this.renderer.render(this.scene, this.camera)
    else this.transparency.render(this.scene, this.camera, this.camera.position.distanceTo(this.controls.target), this.sceneSize())
  }

  // ---- ベンチマーク（?bench=1）用。ふつうに開いたときは使わない ----

  /** 動かしている間の描き方（ベンチマークで決めていればそれ、なければ quality が選んだ段階） */
  private motionStage(): Stage {
    return this.benchStage ?? this.quality.stage
  }
  /** ベンチマークで決めた、動かしている間の描き方（null なら quality に任せる） */
  private benchStage: Stage | null = null
  /** 描いたコマごとの時間（frameStats 用）。null なら測らない */
  private frameLog: { interval: number; cpu: number }[] | null = null
  private gpuLog: number[] = []
  private timerExt: { TIME_ELAPSED_EXT: number; GPU_DISJOINT_EXT: number } | null = null
  private pendingQueries: WebGLQuery[] = []
  /** ベンチマークで回す前のカメラの位置（見る点からの差） */
  private orbitStart: THREE.Vector3 | null = null

  /** ふつうのときの画素密度 */
  get baseRatio(): number {
    return this.basePixelRatio
  }

  /**
   * コマの時間を測り始める（frameStats で読む）。stage を渡すと、動かしている間の描き方をそれに決める
   * （null なら quality が選ぶ。ふつうの動き）
   */
  benchMeasure(stage: Stage | null) {
    this.benchStage = stage
    this.frameLog = []
    this.gpuLog = []
    if (!this.timerExt) {
      const gl = this.renderer.getContext() as WebGL2RenderingContext
      this.timerExt = gl.getExtension('EXT_disjoint_timer_query_webgl2')
    }
    this.applyQuality()
  }

  /** 測るのをやめ、カメラを回す前の位置に戻す */
  benchEnd() {
    this.benchStage = null
    this.frameLog = null
    if (this.orbitStart) this.camera.position.copy(this.controls.target).add(this.orbitStart)
    this.orbitStart = null
    this.controls.update()
    this.applyQuality()
  }

  /** カメラを、見る点の周りに（縦の軸の周りで）回す。angle は回し始めからの角度 */
  benchOrbitTo(angle: number) {
    const target = this.controls.target
    if (!this.orbitStart) this.orbitStart = this.camera.position.clone().sub(target)
    const up = this.camera.up.clone().normalize()
    this.camera.position.copy(target).add(this.orbitStart.clone().applyAxisAngle(up, angle))
    this.camera.lookAt(target)
    this.dirty = true
  }

  /** 1 コマを描き、画面側の時間（render の呼び出し）と、使えれば GPU の時間を記録する */
  private renderMeasured(interval: number | null) {
    const gl = this.renderer.getContext() as WebGL2RenderingContext
    const ext = this.timerExt
    // 前に頼んだ GPU の時間で、結果が出たものを読む
    while (ext && this.pendingQueries.length > 0) {
      const q = this.pendingQueries[0]
      if (!gl.getQueryParameter(q, gl.QUERY_RESULT_AVAILABLE)) break
      this.pendingQueries.shift()
      if (!gl.getParameter(ext.GPU_DISJOINT_EXT)) this.gpuLog.push(gl.getQueryParameter(q, gl.QUERY_RESULT) / 1e6)
      gl.deleteQuery(q)
    }
    const q = ext && this.pendingQueries.length < 8 ? gl.createQuery() : null
    if (q) gl.beginQuery(ext!.TIME_ELAPSED_EXT, q)
    const t0 = performance.now()
    this.renderScene()
    const cpu = performance.now() - t0
    if (q) {
      gl.endQuery(ext!.TIME_ELAPSED_EXT)
      this.pendingQueries.push(q)
    }
    // 前の回も描いていたときだけ、間隔を 1 コマの時間として数える
    if (interval !== null) this.frameLog!.push({ interval, cpu })
    if (this.frameLog!.length > 120) this.frameLog!.shift()
    if (this.gpuLog.length > 120) this.gpuLog.shift()
  }

  /**
   * 直近 120 コマの時間（中央値と 95% 点。ms）、描いた三角形の数、今の画素密度、粗い版を描いているか。
   * interval はコマの間隔（画面の更新間隔で刻まれる）、cpu は render の呼び出しの時間、gpu は GPU の時間（測れなければ null）
   */
  frameStats() {
    const log = this.frameLog ?? []
    const q = (xs: number[], p: number) => {
      if (xs.length === 0) return null
      const s = [...xs].sort((a, b) => a - b)
      return s[Math.min(s.length - 1, Math.floor(p * s.length))]
    }
    const interval = log.map((f) => f.interval)
    const cpu = log.map((f) => f.cpu)
    let lod = false
    for (const p of this.plots.values()) {
      if (p.kind === 'surface' && (p.group.children[0] as THREE.Mesh).geometry !== p.geo) lod = true
    }
    return {
      frames: log.length,
      median: q(interval, 0.5),
      p95: q(interval, 0.95),
      cpuMedian: q(cpu, 0.5),
      cpuP95: q(cpu, 0.95),
      gpuMedian: this.timerExt ? q(this.gpuLog, 0.5) : null,
      gpuP95: this.timerExt ? q(this.gpuLog, 0.95) : null,
      triangles: this.renderer.info.render.triangles,
      pixelRatio: this.renderer.getPixelRatio(),
      lod,
    }
  }

  /**
   * 今の視点で 1 コマをすぐ描き、画面の絵を w × h（CSS ピクセル）に縮めて返す。
   * stage が null なら止まっているときの絵、そうでなければ動かしている間にその描き方で描いた絵
   */
  benchCapture(stage: Stage | null, w: number, h: number): Uint8ClampedArray {
    const saved = { stage: this.benchStage, moving: this.camMoving }
    this.benchStage = stage
    this.camMoving = stage !== null
    this.applyQuality()
    this.controls.update()
    this.updateClipRange()
    this.updateLabelSizes()
    this.updateStencilSides()
    this.renderScene()
    // 描いた直後（同じ処理の中）なら、preserveDrawingBuffer なしでも絵を読める
    const c = document.createElement('canvas')
    c.width = w
    c.height = h
    const ctx = c.getContext('2d', { willReadFrequently: true })!
    ctx.drawImage(this.renderer.domElement, 0, 0, w, h)
    const data = ctx.getImageData(0, 0, w, h).data
    this.benchStage = saved.stage
    this.camMoving = saved.moving
    this.applyQuality()
    return data
  }

  /**
   * グラフの一覧に出す小さな絵（JPEG の data URL）。今の視点で 1 コマをすぐ描いて、横 w の大きさに縮める（縦横比は画面のまま）。
   * 描けなければ null
   */
  thumbnail(w = 192): string | null {
    try {
      this.controls.update()
      this.updateClipRange()
      this.updateLabelSizes()
      this.updateStencilSides()
      this.renderScene()
      const src = this.renderer.domElement
      if (!src.width || !src.height) return null
      // 縦長の画面（スマホ）でも一覧で見やすいよう、横 : 縦 = 4 : 3 に中央を切り抜く
      const h = Math.round((w * 3) / 4)
      const k = Math.min(src.width / w, src.height / h)
      const sw = w * k
      const sh = h * k
      const c = document.createElement('canvas')
      c.width = w
      c.height = h
      const ctx = c.getContext('2d')
      if (!ctx) return null
      ctx.drawImage(src, (src.width - sw) / 2, (src.height - sh) / 2, sw, sh, 0, 0, w, h)
      return c.toDataURL('image/jpeg', 0.7)
    } catch {
      return null
    }
  }

  /**
   * 文字の画面上の大きさを、CSS ピクセルで一定にする。
   * sizeAttenuation: false のスプライトは、大きさ 1 が「画面の高さ × 射影行列の [1][1] / 2」になるので、その逆数をかける
   */
  private updateLabelSizes() {
    const { viewW: width, viewH: height, camera } = this
    if (!width || !height) return
    const scale = 2 / (height * camera.projectionMatrix.elements[5])
    const p = this.labelProjection
    const placed = this.positionedLabels
    camera.updateMatrixWorld()
    const view = this.labelView.multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse)
    if (this.labelLayoutWidth === width && this.labelLayoutHeight === height && this.lastLabelView.equals(view)) return
    this.lastLabelView.copy(view)
    this.labelLayoutWidth = width
    this.labelLayoutHeight = height
    placed.length = 0

    // 端の値を先に置く。狭い画面や斜めから見たときは、重なる途中の数字を間引く。
    for (const label of this.labels) {
      label.sprite.visible = false
      p.copy(label.anchor).applyMatrix4(view)
      if (!Number.isFinite(p.z) || p.z < -1 || p.z > 1) continue
      const x = (p.x + 1) * width / 2
      const y = (1 - p.y) * height / 2
      const halfW = label.w / 2 + 2
      const halfH = label.h / 2 + 2
      const margin = label.priority ? Math.max(halfW, halfH) : 0
      if (halfW * 2 > width || halfH * 2 > height || x < -margin || x > width + margin || y < -margin || y > height + margin) continue
      const baseX = Math.min(width - halfW, Math.max(halfW, x))
      const baseY = Math.min(height - halfH, Math.max(halfH, y))
      if (!label.priority && (baseX !== x || baseY !== y)) continue

      const attempts = label.priority ? 5 : 1
      for (let i = 0; i < attempts; i++) {
        let px = baseX
        let py = baseY
        // 角などで端の数値が近いときは、目盛りの方向と直交するほうへ先にずらす。
        if (i > 0) {
          const direction = i % 2 ? 1 : -1
          if ((i <= 2) === (label.nudgeX !== 0)) px += direction * (label.nudgeX || -1) * (label.w + 4)
          else py += direction * (label.h + 4)
        }
        if (px < halfW || px > width - halfW || py < halfH || py > height - halfH) continue
        if (placed.some((other) => Math.abs(other.x - px) < (other.w + label.w) / 2 + 4 && Math.abs(other.y - py) < (other.h + label.h) / 2 + 3)) continue

        label.x = px
        label.y = py
        label.sprite.position.copy(px === x && py === y ? label.anchor : p.set(px * 2 / width - 1, 1 - py * 2 / height, p.z).unproject(camera))
        label.sprite.scale.set(label.w * scale, label.h * scale, 1)
        label.sprite.visible = true
        placed.push(label)
        break
      }
    }
  }

  /** カメラから dist 離れた所での、1 画素（CSS の画素）の大きさ */
  private pixelAt(dist: number): number {
    return (2 * dist * Math.tan(THREE.MathUtils.degToRad(this.camera.fov) / 2)) / (this.viewH || 1)
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
    this.updateProjection()
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
    for (const h of this.holeMarks.values()) {
      const { uBg, uInk, uColor } = (h.material as THREE.ShaderMaterial).uniforms
      uBg.value.set(this.theme.background)
      uInk.value.set(this.theme.text)
      if (h.userData.limit) uColor.value.set(this.theme.text)
    }
    for (const { line, dots } of this.limitSources.values()) {
      line.material.color.set(this.theme.text)
      dots.material.color.set(this.theme.text)
    }
    if (this.domain) this.buildFrame(this.domain)
    for (const g of this.coordGrids.values()) (g.line.material as THREE.LineBasicMaterial).color.set(this.theme.box)
  }

  /** 表示範囲を設定し、軸・目盛り・枠を作り直す */
  setDomain(d: Domain, resetCamera = false) {
    this.dirty = true
    const first = this.domain === null
    this.domain = { ...d }

    // 範囲の外を切り落とす6枚の平面。ほんの少し（範囲の 1e-5 倍）外に置く: ちょうど端の高さにある平らな面
    // （階段 z = ⌊x⌋/2 + ⌊y⌋/4 の z = 6 の段など）が、切る面と重なって、残る所と消える所の砂嵐のような模様になるため
    const ex = (d.xmax - d.xmin) * 1e-5
    const ey = (d.ymax - d.ymin) * 1e-5
    const ez = (d.zmax - d.zmin) * 1e-5
    this.boxPlanes = [
      new THREE.Plane(new THREE.Vector3(1, 0, 0), -d.xmin + ex),
      new THREE.Plane(new THREE.Vector3(-1, 0, 0), d.xmax + ex),
      new THREE.Plane(new THREE.Vector3(0, 1, 0), -d.ymin + ey),
      new THREE.Plane(new THREE.Vector3(0, -1, 0), d.ymax + ey),
    ]
    // 2D 表示では、図形は z = 0 の平面にあるので z の範囲では切らない
    if (!this.flat) {
      this.boxPlanes.push(
        new THREE.Plane(new THREE.Vector3(0, 0, 1), -d.zmin + ez),
        new THREE.Plane(new THREE.Vector3(0, 0, -1), d.zmax + ez),
      )
    }
    this.applyClipping()
    this.updateBoxUniforms()
    for (const p of this.plots.values()) {
      if (p.kind === 'surface') p.grid.uHeatRange.value.set(d.zmin, d.zmax)
      else this.placeDots(p.dots)
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
    this.updateProjection()
    this.updateBoxUniforms()
    this.controls.enableRotate = !on
    this.applyButtons()
    this.controls.touches.ONE = on ? THREE.TOUCH.PAN : THREE.TOUCH.ROTATE
    this.camera.up.set(0, on ? 1 : 0, on ? 0 : 1)
    for (const p of this.plots.values()) if (p.kind === 'surface') p.grid.uNoBackShade.value = on ? 1 : 0
    // 3D に戻すときは、画素ごとの 2D の平面を捨てて、隠していた線・面を戻す（3D の結果が届くまでの間も見えるように）
    // 2D に切り替えるときは光線の箱を捨てる（2D の結果が届くまで、隠していた面を見せる）
    if (on) {
      for (const id of [...this.gpu3d.keys()]) this.dropPixelLayers(id)
    }
    if (!on) {
      for (const id of [...this.gpu2d.keys()]) {
        this.removeGpu2d(id)
        const st = this.styles.get(id)
        if (st) this.styleGpu2d(id, st)
      }
    }
    if (this.domain) this.setDomain(this.domain, true)
  }

  /** 範囲を直接編集したとき、枠が画面からはみ出す場合だけ画角を合わせる。 */
  fitDomainIfNeeded() {
    const d = this.domain
    if (!d) return
    this.camera.updateMatrixWorld()
    const corner = this.labelProjection
    for (const x of [d.xmin, d.xmax]) {
      for (const y of [d.ymin, d.ymax]) {
        for (const z of this.flat ? [0] : [d.zmin, d.zmax]) {
          corner.set(x, y, z).project(this.camera)
          if (Math.abs(corner.x) > 0.9 || Math.abs(corner.y) > 0.9 || Math.abs(corner.z) > 1) {
            this.resetCamera(true)
            return
          }
        }
      }
    }
  }

  /** カメラを斜め上からの見やすい位置に戻す（2D 表示なら真上から） */
  resetCamera(keepDirection = false) {
    this.wheelPending = 0
    this.dirty = true
    this.notMotion = true // 視点が跳ぶだけ（2D / 3D の切り替えなど）
    const d = this.domain
    if (!d) return
    if (this.flat) {
      // 表示範囲の長方形が画面に収まる距離（縦長の画面では横幅に合わせる）
      const c = new THREE.Vector3((d.xmin + d.xmax) / 2, (d.ymin + d.ymax) / 2, 0)
      const half = Math.max((d.ymax - d.ymin) / 2, (d.xmax - d.xmin) / 2 / this.camera.aspect) * 1.12
      this.controls.target.copy(c)
      this.camera.position.copy(c).add(new THREE.Vector3(0, 0, half / this.verticalHalfAngleTan()))
      this.camera.lookAt(c)
      this.updateClipRange()
      return
    }
    const center = new THREE.Vector3((d.xmin + d.xmax) / 2, (d.ymin + d.ymax) / 2, (d.zmin + d.zmax) / 2)
    const size = this.sceneSize()
    // 縦長の画面（スマホ）では横幅に収まるよう、その分だけ遠くから見る
    const dist = (size * 0.56) / this.perspective / Math.min(1, this.camera.aspect)
    const direction = keepDirection ? this.camera.position.clone().sub(this.controls.target) : new THREE.Vector3(1.1, -1.5, 0.9)
    this.controls.target.copy(center)
    this.camera.position.copy(center).add(direction.setLength(dist))
    this.updateClipRange()
  }

  private buildFrame(d: Domain) {
    this.buildFrameLines(d)
    this.labels.sort((a, b) => b.priority - a.priority)
    this.labelLayoutWidth = 0
    this.applyLineScale(this.pixelScale.value) // 画素密度を下げている最中に作り直したときも、線の濃さを合わせる
  }

  private buildFrameLines(d: Domain) {
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
    const label = (text: string, kind: 'tick' | 'edge' | 'axis-name', pos: THREE.Vector3, nudgeX = 0) => {
      const font = kind === 'tick' ? TICK_FONT : kind === 'edge' ? EDGE_FONT : AXIS_FONT
      const l = makeLabel(text, font, kind === 'tick' ? this.theme.tick : this.theme.text, pos)
      l.priority = kind === 'tick' ? 0 : kind === 'axis-name' ? 1 : 2
      l.nudgeX = nudgeX
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
    const xTicks = axisTicks(d.xmin, d.xmax)
    const yTicks = axisTicks(d.ymin, d.ymax)
    const zTicks = axisTicks(d.zmin, d.zmax)
    const steps = [xTicks.step, yTicks.step, zTicks.step]
    const pts: number[] = []
    for (const x of xTicks.grid) {
      pts.push(x, d.ymin, zFloor, x, d.ymax, zFloor)
    }
    for (const y of yTicks.grid) {
      pts.push(d.xmin, y, zFloor, d.xmax, y, zFloor)
    }
    const gridGeo = new THREE.BufferGeometry()
    gridGeo.setAttribute('position', new THREE.Float32BufferAttribute(pts, 3))
    const floorGrid = new THREE.LineSegments(gridGeo, new THREE.LineBasicMaterial({ color: t.grid }))
    if (!this.flat) {
      // 3D: 床の方眼は、図形よりいつも奥に見せる（最初に描き、深さを書かない）。下から見上げると、床（z = 0）は上の図形より手前にあるので、
      // 深さのとおりに描くと方眼が図形の上に重なって、図形にひびが入ったように見えた（floor(x) + floor(y) + floor(z) = 0 の立方体）。
      // 軸・枠は深さのとおりに描く（面を貫く軸の見え方を残す）
      floorGrid.material.depthWrite = false
      floorGrid.renderOrder = -10000
    }
    this.frame.add(floorGrid)

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
    for (const [i, [a, b, color, name]] of axes.entries()) {
      const g = new THREE.BufferGeometry().setFromPoints([a, b])
      this.frame.add(new THREE.LineSegments(g, new THREE.LineBasicMaterial({ color })))
      const end = b.clone().add(b.clone().sub(a).normalize().multiplyScalar(steps[i] * 0.4))
      this.frame.add(label(name, 'axis-name', end))
    }

    // 目盛りの数字（x, y は床の手前の辺、z は左奥の縦の辺）
    const padX = xTicks.step * 0.35
    const padY = yTicks.step * 0.35
    for (const { value, edge } of xTicks.labels) {
      this.frame.add(label(axisTickText(value, xTicks.step), edge ? 'edge' : 'tick', new THREE.Vector3(value, d.ymin - padY, zFloor)))
    }
    for (const { value, edge } of yTicks.labels) {
      // 2D 表示では左の辺に並べる
      const x = this.flat ? d.xmin - padX * 1.3 : d.xmax + padX
      this.frame.add(label(axisTickText(value, yTicks.step), edge ? 'edge' : 'tick', new THREE.Vector3(x, value, zFloor), this.flat ? -1 : 1))
    }
    if (!this.flat) {
      // 3D: 床の方眼・軸・枠の線を、深さだけ少し奥へずらす。面とちょうど同じ所にある線（floor(x) + floor(y) + floor(z) = 0 の
      // z = 0 の上面・平面 z = 0）は、深さが同じになって、線がぶつぶつに見える（ちらつく）。面の上に描く境目の線・≠ の点線は
      // ずらさないので、面の上にそのまま見える
      this.frame.traverse((o) => {
        if (o instanceof THREE.LineSegments) {
          const m = o.material as THREE.Material
          m.onBeforeCompile = (shader) => {
            shader.vertexShader = shader.vertexShader.replace(
              '#include <project_vertex>',
              '#include <project_vertex>\n  gl_Position.z += 3e-4 * gl_Position.w;',
            )
          }
          m.customProgramCacheKey = () => 'frame-behind'
        }
      })
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
    for (const { value, edge } of zTicks.labels) {
      this.frame.add(label(axisTickText(value, zTicks.step), edge ? 'edge' : 'tick', new THREE.Vector3(d.xmin - padX, d.ymin - padY, value), -1))
    }
  }

  /** 式 id の曲面・曲線を設定（作り直し） */
  /**
   * 式の形を描く。座標系を使う式（mesh.target がある）は、計算エンジンが返した形を覚えておき、
   * 移行のスライダーの値（[`setMorph`]）で位置を混ぜた形を描く
   */
  setPlot(id: number, mesh: MeshData, style: PlotStyle) {
    const source = (mesh.target?.length === mesh.positions.length || mesh.limitSourceTarget)
      ? { mesh, weld: null }
      : null
    if (source) mesh = this.morphed(id, source)
    this.applyPlot(id, mesh, style)
    if (source) this.morphSources.set(id, source)
    else this.morphSources.delete(id)
    this.notePickData(id, mesh)
  }

  /** 回転中の形を、表示座標で記録する。 */
  recordRotationTrail(id: number, angles: number[], center: number[]) {
    const plot = this.plots.get(id)
    const style = this.styles.get(id)
    if (!plot || !style?.visible) return
    const now = performance.now()
    let trail = this.rotationTrails.get(id)
    if (trail) {
      const previousAngles = trail.angles
      const delta = Math.max(0, ...angles.map((a, i) => {
        const d = Math.abs((a - (previousAngles[i] ?? 0)) % 360)
        return Math.min(d, 360 - d)
      }))
      const previousCenter = trail.center
      const centerMoved = center.length !== previousCenter.length || center.some((v, i) => v !== previousCenter[i])
      if ((delta < 0.01 && !centerMoved) || (now - trail.at < (plot.kind === 'surface' ? 240 : 160) && delta < 8)) return
    }
    if (plot.kind === 'surface') {
      const surface = trail?.kind === 'surface' ? trail.surface : new RotationSurfaceTrail()
      const mesh = plot.group.children[0] as THREE.Mesh
      const geo = (plot.geo.userData.lod as THREE.BufferGeometry | undefined) ?? plot.geo
      const volume = plot.volume?.u
      const region = !geo.index?.count && volume?.uRegion.value
        ? regionTrailTriangles(volume)
        : undefined
      if (!surface.record(mesh, geo, plot.cap, region, style.color, this.clipPlanes, this.transparency)) return
      if (!surface.group.parent) this.scene.add(surface.group)
      this.rotationTrails.set(id, { kind: 'surface', surface, angles: [...angles], center: [...center], at: now })
      this.dirty = true
      return
    }

    const lines: number[] = []
    let dots: number[] = []
    const pts = this.curvePts.get(id)
    if (!pts) return
    const step = Math.max(1, Math.ceil(pts.length / (3 * 256)))
    let previous = -1
    for (let i = 0; i < pts.length; i += 3) {
      if (!Number.isFinite(pts[i]) || !Number.isFinite(pts[i + 1]) || !Number.isFinite(pts[i + 2])) {
        previous = -1
      } else if (previous < 0 || i - previous >= 3 * step || !Number.isFinite(pts[i + 3])) {
        if (previous >= 0 && lines.length < 6 * 384 &&
            (pts[previous] !== pts[i] || pts[previous + 1] !== pts[i + 1] || pts[previous + 2] !== pts[i + 2])) {
          lines.push(pts[previous], pts[previous + 1], pts[previous + 2], pts[i], pts[i + 1], pts[i + 2])
        }
        previous = i
      }
    }
    const points = plot.dots.userData.pos as number[]
    const stride = Math.max(1, Math.ceil(points.length / (3 * 128))) * 3
    for (let i = 0; i < points.length; i += stride) dots.push(points[i], points[i + 1], points[i + 2])
    const lonePoint = lines.length === 0 && dots.length === 3 ? dots : undefined
    if (lonePoint && trail?.kind === 'curve' && trail.lastPoint) {
      lines.push(...trail.lastPoint, ...lonePoint)
      dots = []
    }
    if (!lines.length && !dots.length) return
    if (trail?.kind !== 'curve') {
      const appearance = { color: style.color, transparent: true, opacity: 0.26, depthWrite: false, clippingPlanes: this.clipPlanes }
      const strokes = new THREE.LineSegments(new THREE.BufferGeometry(), new THREE.LineBasicMaterial(appearance))
      const points = new THREE.Points(new THREE.BufferGeometry(), new THREE.PointsMaterial({ ...appearance, size: 3, sizeAttenuation: false }))
      strokes.raycast = points.raycast = () => {}
      this.scene.add(strokes, points)
      trail = { kind: 'curve', lines: strokes, dots: points, frames: [], angles, center, at: now }
      this.rotationTrails.set(id, trail)
    }
    trail.angles = [...angles]
    trail.center = [...center]
    trail.at = now
    trail.lastPoint = lonePoint
    trail.frames.push({ lines: new Float32Array(lines), dots: new Float32Array(dots) })
    if (trail.frames.length > TRAIL_FRAMES) trail.frames.shift()
    for (const kind of ['lines', 'dots'] as const) {
      const positions = new Float32Array(trail.frames.reduce((n, frame) => n + frame[kind].length, 0))
      let offset = 0
      for (const frame of trail.frames) {
        positions.set(frame[kind], offset)
        offset += frame[kind].length
      }
      const geometry = new THREE.BufferGeometry()
      geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3))
      trail[kind].geometry.dispose()
      trail[kind].geometry = geometry
    }
    this.dirty = true
  }

  clearRotationTrail(id: number) {
    const trail = this.rotationTrails.get(id)
    if (!trail) return
    if (trail.kind === 'surface') {
      this.scene.remove(trail.surface.group)
      trail.surface.dispose()
    } else {
      for (const object of [trail.lines, trail.dots]) {
        this.scene.remove(object)
        object.geometry.dispose()
        object.material.dispose()
      }
    }
    this.rotationTrails.delete(id)
    this.dirty = true
  }

  clearRotationTrails() {
    for (const id of this.rotationTrails.keys()) this.clearRotationTrail(id)
  }

  /**
   * 座標系を使う式の、移行のスライダーの値 s（0〜1）を変える。位置を (1 − s) q + s T(q) に書き換えて、形だけを差し替える
   * （式は計算し直さない）。座標系を使わない式では、値を覚えるだけ
   */
  setMorph(id: number, s: number) {
    const v = Math.min(1, Math.max(0, s))
    if (this.morphValues.get(id) === v) return
    this.morphValues.set(id, v)
    const m = this.morphSources.get(id)
    const style = this.styles.get(id)
    if (m && style) {
      const mm = this.morphed(id, m)
      this.applyPlot(id, mm, style)
      this.notePickData(id, mm)
    }
  }

  /**
   * 座標系の格子（座標曲線。計画 S39）を描く・消す（`q` が null なら消す）。`q` は元の位置（座標の値）、`target` は移した先
   * （点の列。線の間は NaN）。移行の値 `s` で混ぜて描く。枠・方眼より少し濃い色の細い線で、表示範囲・切り口で切る
   */
  setCoordGrid(cid: number, q: Float32Array | null, target: Float32Array | null, s: number) {
    const old = this.coordGrids.get(cid)
    if (old) {
      old.line.geometry.dispose()
      ;(old.line.material as THREE.Material).dispose()
      this.scene.remove(old.line)
      this.coordGrids.delete(cid)
    }
    this.dirty = true
    if (!q || !target || q.length !== target.length) return
    const mat = new THREE.LineBasicMaterial({ color: this.theme.box, transparent: true, opacity: 0.55, clippingPlanes: this.clipPlanes })
    const line = new THREE.LineSegments(new THREE.BufferGeometry(), mat)
    line.raycast = () => {} // 視点の操作の「カーソルの下の図形」には使わない
    line.frustumCulled = false
    this.scene.add(line)
    const g = { line, q, target, s: -1 }
    this.coordGrids.set(cid, g)
    this.setCoordGridMorph(cid, s)
  }

  /** 座標系の格子の移行の値を変える（形だけを差し替える） */
  setCoordGridMorph(cid: number, s: number) {
    const g = this.coordGrids.get(cid)
    if (!g || g.s === s) return
    g.s = s
    const pts = mixPositions(g.q, g.target, Math.min(1, Math.max(0, s)))
    // 点の列を、隣り合う 2 点ずつの線分にする（NaN の所は線を切る）
    const ok = (k: number) => Number.isFinite(pts[k]) && Number.isFinite(pts[k + 1]) && Number.isFinite(pts[k + 2])
    const seg: number[] = []
    for (let k = 0; k + 5 < pts.length; k += 3) {
      if (ok(k) && ok(k + 3)) for (let j = 0; j < 6; j++) seg.push(pts[k + j])
    }
    g.line.geometry.dispose()
    const geo = new THREE.BufferGeometry()
    geo.setAttribute('position', new THREE.Float32BufferAttribute(seg, 3))
    g.line.geometry = geo
    this.dirty = true
  }

  /** 移行のスライダーの値で位置を混ぜた形。s = 0 なら、計算エンジンが返した位置と法線をそのまま使う */
  private morphed(id: number, m: { mesh: MeshData; weld: Int32Array | null }): MeshData {
    const { mesh } = m
    const s = this.morphValues.get(id) ?? 0
    const target = mesh.target?.length === mesh.positions.length ? mesh.target : undefined
    const positions = target ? mixPositions(mesh.positions, target, s) : mesh.positions
    const surface = !mesh.curve && mesh.indices.length > 0
    let normals = mesh.normals
    if (target && surface && s > 0) {
      normals = vertexNormals(positions, mesh.indices)
      // s = 1 では、周期のある座標の継ぎ目（φ = 0 と 2π）で同じ点になる頂点の法線をそろえる
      if (s >= 1) weldNormals(normals, (m.weld ??= weldGroups(target, mesh.indices)))
    }
    const lod =
      mesh.lod && mesh.lod.target && mesh.lod.target.length === mesh.lod.positions.length && s > 0
        ? (() => {
            const lp = mixPositions(mesh.lod.positions, mesh.lod.target, s)
            return { ...mesh.lod, positions: lp, normals: vertexNormals(lp, mesh.lod.indices) }
          })()
        : mesh.lod
    const mix = (q?: Float32Array, t?: Float32Array) => (q && t ? mixPositions(q, t, s) : q)
    return {
      ...mesh,
      positions,
      normals,
      // 格子線は座標の値に沿って引く（座標系の格子に沿った線になる）
      gridCoords: mesh.gridCoords.length > 0 || !surface ? mesh.gridCoords : mesh.positions,
      lod,
      outline: mix(mesh.outline, mesh.outlineTarget),
      outlineDashed: mix(mesh.outlineDashed, mesh.outlineDashedTarget),
      holes: mix(mesh.holes, mesh.limitHolesTarget),
      limitPoint: mix(mesh.limitPoint, mesh.limitPointTarget),
      limitSource: mix(mesh.limitSource, mesh.limitSourceTarget),
    }
  }

  private applyPlot(id: number, mesh: MeshData, style: PlotStyle) {
    this.sceneChanged = true
    this.notMotion = true
    const old = this.plots.get(id)
    const kind = mesh.curve ? 'curve' : 'surface'
    // 同じ種類なら、材質（シェーダー）はそのまま使い、形のデータだけ差し替える。
    // 回転を再生しているときなど、毎フレーム形が変わるときに軽くなる
    if (old && old.kind === kind) {
      if (old.kind === 'curve') {
        old.line.geometry.dispose()
        old.line.geometry = this.curveGeometry(mesh.positions)
        this.setCurveValues(old.line, mesh)
        // 点の数が変わるので、点の球は作り直す（材質はそのまま使う）
        const mat = old.dots.material as THREE.MeshStandardMaterial
        const scale = old.dots.userData.scale
        old.line.remove(old.dots)
        old.dots.dispose()
        old.dots = this.makeDots(mesh.positions, mat, scale)
        old.line.add(old.dots)
      } else {
        disposeGeometry(old.geo)
        old.geo = this.surfaceGeometry(mesh)
        this.applyLod(old)
        old.uvSpan = mesh.params.length >= 2 ? mesh.paramSpan : null
        old.solid = mesh.solid
        old.vivid = mesh.vivid === true
        this.applyValueRange(old.grid, mesh)
        old.grid.uHeat.value = mesh.heat ? 1 : 0
        if (old.stencil) old.stencil.inc.geometry = old.stencil.dec.geometry = old.geo
        this.setOutlines(old, mesh)
        this.setVolume(old, mesh)
      }
    } else {
      this.removePlot(id)
      if (mesh.curve) {
        this.setCurve(id, mesh.positions)
        const p = this.plots.get(id)
        if (p?.kind === 'curve') this.setCurveValues(p.line, mesh)
      } else this.setSurface(id, mesh)
    }
    this.updateStencil(id)
    this.setLimitSource(id, mesh.limitSource)
    this.setHoles(id, mesh)
    // 描画の方法（式の ⚙。自動なら端末で決める）で、GPU の描き方を使うか
    this.syncGpu2d(id, mesh, wantGpu(style.render, '2d'))
    this.syncGpu3d(id, mesh, wantGpu(style.render, '3d'), style.render === 'gpu')
    this.setStyle(id, style)
  }

  /** 画素ごとの 2D の描き方の平面を、届いた結果に合わせて作る・作り直す・捨てる（GLSL が同じなら材質をそのまま使う） */
  private syncGpu2d(id: number, mesh: MeshData, want: boolean) {
    const info = this.flat && want ? mesh.gpu2d : undefined
    const old = this.gpu2d.get(id)
    const d = this.domain
    if (!info || !d) {
      if (old) this.removeGpu2d(id)
      return
    }
    const key = `${d.xmin},${d.xmax},${d.ymin},${d.ymax}`
    if (old && old.glsl === info.glsl) {
      if (old.key !== key) {
        if (old.ready) this.scene.remove(old.mesh)
        old.mesh.geometry.dispose()
        old.mesh = gpu2dMesh(old.mat, d)
        old.key = key
        if (old.ready) this.scene.add(old.mesh)
      }
      // 回転・断面・パラメータは uniform（GLSL は同じ）
      setGpu2dUniforms(old.mat.uniforms as Gpu2dUniforms, info)
      old.params = info.params
      this.dirty = true
      return
    }
    if (old) this.removeGpu2d(id)
    const mat = gpu2dMaterial(info)
    setGpu2dUniforms(mat.uniforms as Gpu2dUniforms, info)
    const m = gpu2dMesh(mat, d)
    const layer = { mesh: m, mat, glsl: info.glsl, key, eq: info.eq, ready: false, params: info.params }
    this.gpu2d.set(id, layer)
    // GLSL が変わったとき（式を書き換えた・パラメータを畳み込んだ式でスライダーを動かした）は、コンパイルを裏で行い
    // （KHR_parallel_shader_compile。無い環境ではその場で）、終わるまでは今までのメッシュの線・面を見せる
    const show = () => {
      if (this.gpu2d.get(id) !== layer) return // 待っている間に、次の結果が来た・式が消えた
      layer.ready = true
      this.scene.add(layer.mesh)
      const st = this.styles.get(id)
      if (st) this.styleGpu2d(id, st)
      this.dirty = true
    }
    this.renderer.compileAsync(m, this.camera, this.scene).then(show, show)
  }

  private removeGpu2d(id: number) {
    const g = this.gpu2d.get(id)
    if (!g) return
    if (g.ready) this.scene.remove(g.mesh)
    g.mesh.geometry.dispose()
    g.mat.dispose()
    this.gpu2d.delete(id)
    this.dirty = true
  }

  /** 光線で描く箱を、届いた結果に合わせて作る・作り直す・捨てる（GLSL が同じなら材質をそのまま使う） */
  private syncGpu3d(id: number, mesh: MeshData, want: boolean, forced: boolean) {
    let info = !this.flat && want ? mesh.gpu3d : undefined
    // 打ち切りが多くてメッシュに回した式（自動のときだけ。画素ごとを選んでいれば、そのまま光線で描く）
    if (info && !forced && this.gpuRejected.get(id)?.glsl === info.glsl) info = undefined
    const glsl = info?.glsl
    const old = this.gpu3d.get(id)
    const d = this.domain
    const p = this.plots.get(id)
    if (!info || !glsl || !d || !p || p.kind !== 'surface') {
      if (old) this.removeGpu3d(id)
      return
    }
    // 表示中の座標から式の座標への 1 次式（回転・断面）・パラメータの値（GLSL が同じなら、これだけを変える）
    const setUniforms = (ray: RaycastUniforms) => {
      setRayAffine(ray, info.affine)
      ray.uParam.value = info.values.slice()
    }
    const key = `,,,,,`
    if (old && old.glsl === glsl) {
      if (old.key !== key) {
        if (old.ready) this.scene.remove(old.mesh)
        old.mesh.geometry.dispose()
        old.mesh = raycastMesh(old.mat, old.ray, d)
        old.key = key
        if (old.ready) this.scene.add(old.mesh)
      }
      setUniforms(old.ray)
      old.axes = info.axes
      this.dirty = true
      return
    }
    if (old) this.removeGpu3d(id)
    const ray = createRaycastUniforms()
    setUniforms(ray)
    this.applyRayBudget(ray)
    // 格子線・色付けの値は、その式の曲面と同じもの（式の ⚙ の設定がそのまま効く）
    const mat = createRaycastMaterial(glsl, Math.floor(info.affine.length / 4), p.grid, ray)
    this.transparency.register(mat, true)
    // 不透明・半透明は、コンパイルの前に面の材質に合わせる（three.js は別のシェーダーにするので、後で変えると
    // 画面のスレッドを止めてコンパイルし直す。光線のシェーダーは重く、Windows の Chrome で 2 秒ほど固まった）
    const src = (p.group.children[0] as THREE.Mesh).material as THREE.MeshStandardMaterial
    mat.transparent = src.transparent
    mat.depthWrite = src.depthWrite
    mat.alphaToCoverage = !src.transparent
    const m = raycastMesh(mat, ray, d)
    const layer = { mesh: m, mat, ray, glsl, key, ready: false, axes: info.axes, params: info.params }
    this.gpu3d.set(id, layer)
    this.applyRayCut(layer.ray)
    // コンパイルは裏で行い、終わるまでは今までのメッシュの面を見せる
    const show = () => {
      if (this.gpu3d.get(id) !== layer) return
      layer.ready = true
      this.scene.add(layer.mesh)
      this.quality.forget()
      // 自動のときは、打ち切りが多ければメッシュに回す（描いた後の次のコマで、一度だけ数える）
      if (!forced) requestAnimationFrame(() => this.checkAborts(id, layer))
      const st = this.styles.get(id)
      if (st) this.setStyle(id, st)
      this.dirty = true
    }
    this.renderer.compileAsync(m, this.camera, this.scene).then(show, show)
  }

  /**
   * 光線で描くと打ち切りが多い式（自動のとき、メッシュに回した）。式の番号 → その GLSL と理由。
   * 同じ GLSL の結果が届いても、光線の箱を作り直さない（計画 12.6 の危ない所 1 の (c)）
   */
  private gpuRejected = new Map<number, { glsl: string; reason: string }>()
  /** 光線の描き方からメッシュに回したとき（App が式の ⚙ に理由を出す） */
  onGpuFallback: ((id: number, reason: string) => void) | null = null

  /** 光線の描き方からメッシュに回した理由（回していなければ空） */
  gpuFallbackReason(id: number, glsl: string | undefined): string {
    const r = this.gpuRejected.get(id)
    return r && r.glsl === glsl ? r.reason : ''
  }

  /**
   * 光線の描き方で、打ち切った画素の割合を数える。小さな描画先（横 192 画素）に一度だけ描き、光を当てずに
   * (打ち切ったか, 当たったか) を出させて読む。1 画素の大きさは画面のものを使う（同じ深さまで詰めたときの割合）。
   * mod(x y, z) = 0 のように、値の飛ぶ所が 1 画素より細かく並ぶ式は、画面の大きさではほとんどの画素が打ち切りになる
   */
  private abortRatio(layer: { mesh: THREE.Mesh; ray: RaycastUniforms }): number {
    // 描く先は画面そのもの（左下の小さな範囲）、シーンもふだんと同じ（光線の箱のほかは隠す）にする。
    // three.js は、描く先（画面 / 描画先のテクスチャ）で色空間が、シーンの光源で光の数が変わると、別のシェーダーとして
    // コンパイルし直す。前は光源の無いシーンを描画先のテクスチャに描いていたので、重い光線のシェーダーをもう一度、
    // 画面のスレッドを止めてコンパイルしていた（Windows の Chrome で 2〜4 秒固まった）。同じ設定で描けば、
    // 裏でコンパイルし終えたシェーダーをそのまま使える
    const ratio = this.renderer.getPixelRatio()
    const cw = this.viewW || 1
    const ch = this.viewH || 1
    const w = Math.min(192, Math.floor(cw * ratio))
    const h = Math.max(1, Math.round((w * ch) / cw))
    const hidden: THREE.Object3D[] = []
    // 光源（カメラに付けた光源も）を含むものは隠さない（光の数が変わると、別のシェーダーになる）
    const hasLight = (o: THREE.Object3D) => {
      let found = false
      o.traverse((c) => {
        if ((c as THREE.Light).isLight) found = true
      })
      return found
    }
    for (const o of this.scene.children) {
      if (o !== layer.mesh && o.visible && !hasLight(o)) {
        o.visible = false
        hidden.push(o)
      }
    }
    const background = this.scene.background
    this.scene.background = null
    layer.ray.uProbe.value = 1
    layer.mesh.userData.pixelH = this.renderer.domElement.height
    const prevColor = this.renderer.getClearColor(new THREE.Color())
    const prevAlpha = this.renderer.getClearAlpha()
    const px = new Uint8Array(w * h * 4)
    const gl = this.renderer.getContext()
    const prevViewport = this.renderer.getViewport(new THREE.Vector4())
    const prevScissor = this.renderer.getScissor(new THREE.Vector4())
    const prevScissorTest = this.renderer.getScissorTest()
    try {
      this.renderer.setRenderTarget(null)
      this.renderer.setViewport(0, 0, w / ratio, h / ratio)
      this.renderer.setScissor(0, 0, w / ratio, h / ratio)
      this.renderer.setScissorTest(true)
      this.renderer.setClearColor(0x000000, 0)
      this.renderer.clear()
      this.renderer.render(this.scene, this.camera)
      gl.readPixels(0, 0, w, h, gl.RGBA, gl.UNSIGNED_BYTE, px)
    } finally {
      this.renderer.setScissorTest(prevScissorTest)
      this.renderer.setScissor(prevScissor)
      this.renderer.setViewport(prevViewport)
      this.renderer.setClearColor(prevColor, prevAlpha)
      this.scene.background = background
      for (const o of hidden) o.visible = true
      layer.ray.uProbe.value = 0
      delete layer.mesh.userData.pixelH
      // 左下を確かめのために塗ったので、画面に出る前に、ふつうの絵で描き直す
      this.renderScene()
      this.dirty = true
    }
    let drawn = 0
    let aborted = 0
    for (let i = 0; i < px.length; i += 4) {
      if (px[i + 1] > 127) {
        drawn++
        if (px[i] > 127) aborted++
      }
    }
    return drawn >= 20 ? aborted / drawn : 0
  }

  /** 打ち切りが 15 % を超えたら、メッシュに回す（自動のときだけ） */
  private checkAborts(id: number, layer: { mesh: THREE.Mesh; ray: RaycastUniforms; glsl: string }) {
    if (this.gpu3d.get(id) !== layer) return
    const ratio = this.abortRatio(layer)
    if (ratio <= 0.15) return
    const reason = `値の飛ぶ所などが細かすぎて、画素ごとの描き方では詰めきれません（打ち切り ${Math.round(ratio * 100)} %）`
    this.gpuRejected.set(id, { glsl: layer.glsl, reason })
    this.removeGpu3d(id)
    const st = this.styles.get(id)
    if (st) this.setStyle(id, st)
    this.onGpuFallback?.(id, reason)
  }

  private removeGpu3d(id: number) {
    const g = this.gpu3d.get(id)
    if (!g) return
    this.quality.forget()
    if (g.ready) this.scene.remove(g.mesh)
    g.mesh.geometry.dispose()
    g.mat.dispose()
    this.gpu3d.delete(id)
    this.dirty = true
  }

  /**
   * 回転だけが変わった結果（形は同じで、向きだけが届く）を受け取ったとき。光線の式は回転を入れる前の元の座標の式なので、
   * 回転は setPlotTransform で uniform を変えるだけで、捨てなくてよい（計画 G7。前は回した後の式で、ここでメッシュに戻していた）
   */
  dropPixelLayers(_id: number) {}

  /**
   * スライダーのパラメータの値（名前 → 値）を、光線で描く式の uniform に入れる。計算し直しの結果を待たずに描き直せる
   * （その式が使っていない名前は無視する）
   */
  setGpuParams(values: Record<string, number>) {
    for (const g of this.gpu3d.values()) {
      if (g.params.length === 0) continue
      g.ray.uParam.value = g.params.map((n, k) => values[n] ?? g.ray.uParam.value[k] ?? 0)
      this.dirty = true
    }
    for (const g of this.gpu2d.values()) {
      if (g.params.length === 0) continue
      const u = g.mat.uniforms as Gpu2dUniforms
      u.uParam.value = g.params.map((n, k) => values[n] ?? u.uParam.value[k] ?? 0)
      this.dirty = true
    }
  }

  /** 光線の箱の切り口（three.js の Plane と同じく、n · p + c ≥ 0 の側を残す） */
  private applyRayCut(ray: RaycastUniforms) {
    const c = this.cutPlane
    ray.uCutOn.value = c ? 1 : 0
    if (c) ray.uCut.value.set(c.normal.x, c.normal.y, c.normal.z, c.constant)
  }

  /** 光線の描き方の見た目: 面の色・不透明度は、隠したメッシュの面の材質と同じにする */
  private styleGpu3d(id: number, style: PlotStyle) {
    const p = this.plots.get(id)
    const layer = this.gpu3d.get(id)
    if (!p || p.kind !== 'surface' || !layer) return
    const main = p.group.children[0] as THREE.Mesh
    if (!layer.ready) {
      main.visible = true
      return
    }
    main.visible = false
    layer.mesh.visible = style.visible
    const src = main.material as THREE.MeshStandardMaterial
    const m = layer.mat
    m.color.copy(src.color)
    m.emissive.copy(src.emissive)
    m.depthWrite = src.depthWrite
    if (m.transparent !== src.transparent) {
      // 不透明・半透明が変わると three.js は別のシェーダーにするので、裏でコンパイルし直し、その間はメッシュの面を見せる
      m.transparent = src.transparent
      m.alphaToCoverage = !src.transparent
      m.needsUpdate = true
      layer.ready = false
      this.scene.remove(layer.mesh)
      main.visible = true
      const show = () => {
        if (this.gpu3d.get(id) !== layer || layer.ready) return
        layer.ready = true
        this.scene.add(layer.mesh)
        const st = this.styles.get(id)
        if (st) this.styleGpu3d(id, st)
        this.dirty = true
      }
      this.renderer.compileAsync(layer.mesh, this.camera, this.scene).then(show, show)
    }
  }

  /** 画素ごとの描き方の見た目。メッシュの線・面（と実線の境目）は隠し、点線の境目だけ残す */
  private styleGpu2d(id: number, style: PlotStyle) {
    const p = this.plots.get(id)
    const layer = this.gpu2d.get(id)
    const g = layer?.ready ? layer : undefined
    if (!p) return
    if (p.kind === 'curve') {
      p.line.visible = style.visible && !g
    } else {
      p.group.children[0].visible = !g
      for (const l of p.outlines) l.visible = !g || l.userData.dashed === true
    }
    if (!layer) return
    layer.mesh.visible = style.visible
    const u = layer.mat.uniforms as Gpu2dUniforms
    u.uColor.value.set(style.color)
    // 曲線は式の線の太さ、領域の境目はメッシュの道と同じ 2.5
    u.uWidth.value = layer.eq === 1 ? style.lineWidth : 2.5
    // 面の材質は光が当たって明るくなる分、同じ不透明度でも濃く見える。それに近い濃さにする
    const opacity = Math.min(1, Math.max(0, style.opacity))
    u.uFill.value = opacity * 0.88
    const gs = style.grid
    const sp = (v: number) => (gs.enabled && Number.isFinite(v) && v > 0 ? v : 0)
    u.uGridSpacing.value.set(sp(gs.x), sp(gs.y))
    u.uGridColor.value.set(style.color).multiplyScalar(0.3 + 0.7 * (1 - opacity))
    u.uGridStrength.value = 0.75 + 0.25 * (1 - opacity)
    u.uGridWidth.value = gs.width
  }

  /**
   * 曲面の 3D オブジェクトに 1 次式 q = M p + t を掛ける（[M の 1 行目, t₀, 2 行目, t₁, 3 行目, t₂] の 12 個。null なら掛けない）。
   * 回す前の形で作った陰関数は、回転だけが変わったら、形を作り直さずにこれだけを変えて描く（GPU で回す）。
   * 格子線は表示中の座標で引く（uGridWorld）。表示範囲・切り口で切り落とす平面は、もともと表示中の座標
   */
  setPlotTransform(id: number, t: ArrayLike<number> | null) {
    // 回す前の式で描いている光線は、同じ 1 次式を uniform に入れる（GLSL は作り直さない）
    const layer = this.gpu3d.get(id)
    if (layer && t && t.length === 12) {
      applyShownRotation(layer.ray, layer.axes, t)
      this.dirty = true
    }
    const p = this.plots.get(id)
    if (!p || p.kind !== 'surface') return
    const main = p.group.children[0] as THREE.Mesh
    if (t && t.length === 12) {
      main.matrixAutoUpdate = false
      main.matrix.set(t[0], t[1], t[2], t[3], t[4], t[5], t[6], t[7], t[8], t[9], t[10], t[11], 0, 0, 0, 1)
      main.matrixWorldNeedsUpdate = true
      p.grid.uGridWorld.value = 1
    } else if (!main.matrixAutoUpdate) {
      main.matrix.identity()
      main.matrixAutoUpdate = true
      main.matrixWorldNeedsUpdate = true
      p.grid.uGridWorld.value = 0
    }
    this.dirty = true
  }

  // ---- 切り口 ----

  /** 曲面・曲線・ふたの材質に、切り落とす平面を設定し直す */
  private applyClipping() {
    this.clipPlanes = this.cutPlane ? [...this.boxPlanes, this.cutPlane] : this.boxPlanes
    for (const g of this.gpu3d.values()) this.applyRayCut(g.ray)
    for (const p of this.plots.values()) {
      if (p.kind === 'curve') {
        ;(p.line.material as THREE.Material).clippingPlanes = this.clipPlanes
        ;(p.dots.material as THREE.Material).clippingPlanes = this.clipPlanes
        continue
      }
      ;((p.group.children[0] as THREE.Mesh).material as THREE.Material).clippingPlanes = this.clipPlanes
      // ふたは切り口の平面の上にあるので、切り口の平面では切らない
      if (p.cap) (p.cap.material as THREE.Material).clippingPlanes = this.boxPlanes
      for (const l of p.outlines) (l.material as THREE.Material).clippingPlanes = this.clipPlanes
      this.applyVolumeCut(p)
      if (p.stencil) {
        ;(p.stencil.inc.material as THREE.Material).clippingPlanes = this.clipPlanes
        ;(p.stencil.dec.material as THREE.Material).clippingPlanes = this.clipPlanes
        ;(p.stencil.quad.material as THREE.Material).clippingPlanes = this.boxPlanes
      }
    }
    for (const id of this.plots.keys()) this.updateStencil(id)
    for (const g of this.coordGrids.values()) (g.line.material as THREE.Material).clippingPlanes = this.clipPlanes
    for (const { line, dots } of this.limitSources.values()) {
      line.material.clippingPlanes = this.clipPlanes
      dots.material.clippingPlanes = this.clipPlanes
    }
    for (const trail of this.rotationTrails.values()) {
      if (trail.kind === 'surface') trail.surface.setClipping(this.clipPlanes)
      else trail.lines.material.clippingPlanes = trail.dots.material.clippingPlanes = this.clipPlanes
    }
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
    this.notMotion = true
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
      p.cap = new THREE.Mesh(geo, createSurfaceMaterial(p.grid, this.boxPlanes, true))
      this.transparency.register(p.cap.material as THREE.MeshStandardMaterial)
      p.cap.frustumCulled = false // 切り口は表示範囲の中（外接球を求めない。下の曲面と同じ）
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
      const inc = new THREE.Mesh(p.geo, pass(THREE.IncrementWrapStencilOp))
      const dec = new THREE.Mesh(p.geo, pass(THREE.DecrementStencilOp))
      this.transparency.register(inc.material)
      this.transparency.register(dec.material)
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
      this.transparency.register(quadMat)
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
    p.stencil.inc.geometry = p.stencil.dec.geometry = p.geo
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
    // 粗い版（陰関数）: 自分の頂点を持つ。格子線用の座標は位置で代用する（陰関数はもともとそう）。色分けの値があれば使う
    // （等値面を重ねたもの・複素数の陰関数の色分け。無いと、動かしている間だけ位置で色が付いてしまう）
    if (mesh.lod && mesh.lod.indices.length > 0) {
      const lod = new THREE.BufferGeometry()
      const lp = new THREE.BufferAttribute(mesh.lod.positions, 3)
      lod.setAttribute('position', lp)
      lod.setAttribute('normal', new THREE.BufferAttribute(mesh.lod.normals, 3))
      lod.setAttribute('gridCoord', lp)
      const cv = mesh.lod.complexValues
      lod.setAttribute('complexValue', cv && cv.length === mesh.lod.positions.length ? new THREE.BufferAttribute(cv, 3) : lp)
      lod.setIndex(new THREE.BufferAttribute(mesh.lod.indices, 1))
      geo.userData.lod = lod
    }
    // 粗い版: 頂点のデータは同じものを使い、三角形の頂点番号だけが違う
    else if (mesh.lodIndices.length > 0) {
      const lod = new THREE.BufferGeometry()
      for (const name of Object.keys(geo.attributes)) lod.setAttribute(name, geo.attributes[name])
      lod.setIndex(new THREE.BufferAttribute(mesh.lodIndices, 1))
      geo.userData.lod = lod
    }
    return geo
  }

  private setSurface(id: number, mesh: MeshData) {
    // 表と裏は1つの材質で描く（裏面はシェーダーで暗くする）
    const grid = createGridUniforms(this.pixelScale, this.boxUniforms)
    const group = new THREE.Group()
    const geo = this.surfaceGeometry(mesh)
    const surface = new THREE.Mesh(geo, createSurfaceMaterial(grid, this.clipPlanes))
    this.transparency.register(surface.material)
    // 曲面はいつも表示範囲の箱の中にあるので、視錐台の判定はしない。判定すると、three.js が新しい形ごとに外接球を
    // 点を 1 つずつ読んで求める（再生中は返事のたびに。数十万点の形で数十 ms）。カーソルの下の点を探すときは、
    // three.js が必要になったときに求める
    surface.frustumCulled = false
    group.add(surface)
    const uvSpan = mesh.params.length >= 2 ? mesh.paramSpan : null
    this.applyValueRange(grid, mesh)
    grid.uHeat.value = mesh.heat ? 1 : 0
    grid.uNoBackShade.value = this.flat ? 1 : 0
    if (this.domain) grid.uHeatRange.value.set(this.domain.zmin, this.domain.zmax)
    this.plots.set(id, {
      kind: 'surface',
      group,
      geo,
      grid,
      uvSpan,
      solid: mesh.solid,
      cap: null,
      stencil: null,
      order: this.nextOrder++,
      outlines: [],
      vivid: mesh.vivid === true,
      volume: null,
    })
    this.applyLod(this.plots.get(id)!)
    this.setOutlines(this.plots.get(id)!, mesh)
    this.setVolume(this.plots.get(id)!, mesh)
    this.scene.add(group)
  }

  /** 頂点の値の色付け: 表示していない座標の値（valueRange があるとき）か、複素数の偏角 */
  private applyValueRange(grid: GridUniforms, mesh: MeshData) {
    const vr = mesh.valueRange
    grid.uValue.value = vr ? 1 : 0
    if (vr) {
      grid.uValueRange.value.set(vr[0], vr[1])
      const lr = logRange(vr, mesh.complexValues)
      grid.uValueLogRange.value.set(lr[0], lr[1])
    }
    grid.uComplex.value = !vr && mesh.complexValues.length > 0 ? 1 : 0
  }

  /** 伝達関数の箱を作り直す（格子の値が無ければ消す）。箱は計算したときの表示範囲 */
  private setVolume(p: PlotObject, mesh: MeshData) {
    if (p.kind !== 'surface') return
    // 前の箱が見えていたか（見えていたなら、そのシェーダーはもうコンパイルしてある）
    const wasShown = p.volume !== null && p.volume.box.parent === p.group
    if (p.volume) {
      p.volume.u.uTex.value.dispose()
      p.volume.box.geometry.dispose()
      ;(p.volume.box.material as THREE.Material).dispose()
      p.group.remove(p.volume.box)
      p.volume = null
    }
    const v = mesh.volume
    const d = this.domain
    if (!v || !d) return
    const u = createVolumeUniforms(volumeTexture(v.values, v.n))
    u.uBoxMin.value.set(d.xmin, d.ymin, d.zmin)
    u.uBoxMax.value.set(d.xmax, d.ymax, d.zmax)
    if (mesh.valueRange) u.uRange.value.set(mesh.valueRange[0], mesh.valueRange[1])
    u.uSteps.value = Math.min(192, Math.max(64, Math.round(1.5 * v.n)))
    u.uN.value = v.n
    u.uComplex.value = v.values.length === 2 * v.n ** 3 ? 1 : 0
    // 値の範囲が無い格子は、不等式の領域の霧。境目の薄い所を踏み外さないよう、歩数を多めにする
    u.uRegion.value = mesh.valueRange ? 0 : 1
    u.uRayMin.value.copy(u.uBoxMin.value)
    u.uRayMax.value.copy(u.uBoxMax.value)
    if (!mesh.valueRange) {
      u.uSteps.value = Math.min(256, Math.max(96, Math.round(2.5 * v.n)))
      // 光線は、領域（境目から 3 マスまで）を囲む箱の中だけ進める（何も無い所を歩くのが、霧の重さのほとんど）
      const b = regionBounds(v.values, v.n, [d.xmin, d.ymin, d.zmin], [d.xmax, d.ymax, d.zmax])
      if (!b) {
        u.uTex.value.dispose()
        return
      }
      u.uRayMin.value.set(...b[0])
      u.uRayMax.value.set(...b[1])
    }
    const lo = u.uRayMin.value
    const hi = u.uRayMax.value
    const geo = new THREE.BoxGeometry(hi.x - lo.x, hi.y - lo.y, hi.z - lo.z)
    geo.translate((lo.x + hi.x) / 2, (lo.y + hi.y) / 2, (lo.z + hi.z) / 2)
    const box = new THREE.Mesh(geo, createVolumeMaterial(u))
    box.raycast = () => {} // 視点の操作の「カーソルの下の図形」には使わない
    box.renderOrder = 1 // 半透明の面より後に重ねる
    // ふだんは箱の表の面を描く（中の不透明な図形より手前に霧を重ねる）。視点が箱の中（か、表の面が近くの切り捨ての面で
    // 切られるほど近く）に入ると表の面が画面に無くなり、霧が消えるので、そのときは裏の面を描く（光線は視点から進める）
    const eye = new THREE.Vector3()
    box.onBeforeRender = (_r, _s, camera) => {
      eye.setFromMatrixPosition(camera.matrixWorld)
      box.worldToLocal(eye)
      const margin = 2 * ((camera as THREE.PerspectiveCamera).near ?? 0)
      const inside = eye.x > lo.x - margin && eye.x < hi.x + margin && eye.y > lo.y - margin && eye.y < hi.y + margin && eye.z > lo.z - margin && eye.z < hi.z + margin
      const side = inside ? THREE.BackSide : THREE.FrontSide
      const mat = box.material as THREE.ShaderMaterial
      if (mat.side !== side) {
        mat.side = side
        mat.needsUpdate = true
      }
    }
    p.volume = { box, u }
    this.applyVolumeCut(p)
    const style = this.styles.get(this.idOf(p))
    if (style) applyVolumeStyle(u, style.hidden, new THREE.Color(style.color), style.opacity)
    // シェーダーのコンパイルは裏で行い（KHR_parallel_shader_compile。無い環境ではその場で）、終わってから箱を見せる。
    // 最初に描くときにコンパイルすると、その間（Windows の Chrome で 1 秒近く）画面が固まる。
    // 前の箱が見えていた（再生中などで、次の結果に差し替える）ときは、すぐに入れ替える。拡張の無い環境（Firefox・Safari）の
    // compileAsync は、コンパイル済みでも 10 ms 待ってから返事をするので、その間の 1 コマ、箱が消えてちらつく
    if (wasShown) {
      p.group.add(box)
      return
    }
    const show = () => {
      if (p.volume?.box !== box) return // 待っている間に、次の結果が来た・式が消えた
      p.group.add(box)
      this.dirty = true
    }
    this.renderer.compileAsync(box, this.camera, this.scene).then(show, show)
  }

  /** 伝達関数の箱に、切り口の平面を渡す */
  private applyVolumeCut(p: PlotObject) {
    if (p.kind !== 'surface' || !p.volume) return
    const c = this.cutPlane
    p.volume.u.uCutOn.value = c ? 1 : 0
    if (c) p.volume.u.uCut.value.set(c.normal.x, c.normal.y, c.normal.z, c.constant)
  }

  /** 描画物の式の番号 */
  private idOf(p: PlotObject): number {
    for (const [id, q] of this.plots) if (q === p) return id
    return -1
  }

  /** 面の式の境目の線（2D 表示の不等式の領域）を作り直す。`<`・`>` の分は点線 */
  private setOutlines(p: PlotObject, mesh: MeshData) {
    if (p.kind !== 'surface') return
    for (const l of p.outlines) {
      l.geometry.dispose()
      ;(l.material as THREE.Material).dispose()
      p.group.remove(l)
    }
    p.outlines = []
    const d = this.domain
    // 点線の長さは表示範囲の大きさに合わせる（LineMaterial の点線は空間の長さで決まる）
    const span = d ? Math.max(d.xmax - d.xmin, d.ymax - d.ymin) : 20
    for (const [pts, dashed] of [
      [mesh.outline, false],
      [mesh.outlineDashed, true],
    ] as const) {
      if (!pts || pts.length === 0) continue
      const mat = new LineMaterial({
        linewidth: mesh.vectorArrows ? 1.75 : 2.5,
        clippingPlanes: this.clipPlanes,
        dashed,
        dashSize: span * 0.02,
        gapSize: span * 0.012,
      })
      mat.resolution.set(this.viewW, this.viewH)
      const line = new LineSegments2(this.curveGeometry(pts), mat)
      line.frustumCulled = false // 表示範囲の中の線（視錐台の判定のための外接球を求めない）
      if (dashed) line.computeLineDistances()
      line.userData.dashed = dashed
      line.userData.vectorShaft = mesh.vectorArrows === true
      // ベクトル場の流線: 点ごとの大きさで色を付ける（色は setStyle で塗る）
      const vr = mesh.valueRange
      const v = mesh.outlineValues
      // （colorCurve は、曲線の頂点の値と同じく、点ごとに 3 つの並びの 1 つ目を読む）
      let vals: Float32Array | null = null
      if (!dashed && vr && v && v.length * 3 === pts.length) {
        vals = new Float32Array(pts.length)
        for (let i = 0; i < v.length; i++) vals[3 * i] = v[i]
      }
      line.userData.values = vals ? { pts, v: vals, range: vr } : null
      line.raycast = () => {} // 視点の操作の「カーソルの下の図形」には使わない
      p.outlines.push(line)
      p.group.add(line)
    }
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

  /**
   * 点（曲線の中で、1 点に縮んだ所）を、小さな球として置く。球の半径は表示範囲のいちばん長い辺の 0.8%（× 線の太さの倍率）。
   * 表示範囲・太さが変わったら [`placeDots`] で置き直す
   */
  private makeDots(pts: Float32Array, mat: THREE.MeshStandardMaterial, scale = 1) {
    const pos = dotPositions(pts)
    const count = pos.length / 3
    const dots = new THREE.InstancedMesh(count >= 1000 ? denseDotSphere() : dotSphere(), mat, count)
    dots.frustumCulled = false
    dots.raycast = () => {} // 視点の操作の「カーソルの下の図形」には使わない
    dots.userData.pos = pos
    dots.userData.scale = scale
    this.placeDots(dots)
    // 視点が動いたら、画面の上の大きさの上限に合わせて置き直す（点が少ないときだけ。多い点の集まりは空間の大きさのまま）
    if (count <= 4096) {
      dots.onBeforeRender = (_r, _s, camera) => {
        const c = camera as THREE.PerspectiveCamera
        const key = `${c.position.x},${c.position.y},${c.position.z},${c.fov},${this.viewH}`
        if (dots.userData.camKey !== key) {
          dots.userData.camKey = key
          this.placeDots(dots)
        }
      }
    }
    return dots
  }

  /** 点の球の位置と大きさを、今の表示範囲と太さに合わせる */
  private placeDots(dots: THREE.InstancedMesh) {
    const d = this.domain
    const span = d ? Math.max(d.xmax - d.xmin, d.ymax - d.ymin, this.flat ? 0 : d.zmax - d.zmin) : 10
    const r = span * (dots.count >= 1000 ? 0.0025 : 0.006) * dots.userData.scale
    const pos = dots.userData.pos as number[]
    const m = new THREE.Matrix4()
    // 空間の大きさ（表示範囲の 0.6 %）のまま描くが、画面の上で半径 4 画素（太さの倍率をかける）を超えないようにする。
    // 拡大して近づくと、点が画面を覆うほど大きな球になっていた（線積分の「今の点」）
    const cam = this.camera
    const perPixel = (2 * Math.tan(THREE.MathUtils.degToRad(cam.fov) / 2)) / Math.max(1, this.viewH)
    const cap = 4 * dots.userData.scale * perPixel
    for (let i = 0; i < dots.count; i++) {
      const x = pos[3 * i], y = pos[3 * i + 1], z = pos[3 * i + 2]
      const dist = Math.hypot(x - cam.position.x, y - cam.position.y, z - cam.position.z)
      const ri = Math.min(r, cap * dist)
      m.makeScale(ri, ri, ri).setPosition(x, y, z)
      dots.setMatrixAt(i, m)
    }
    dots.instanceMatrix.needsUpdate = true
    this.dirty = true
  }

  private setCurve(id: number, pts: Float32Array) {
    const mat = new LineMaterial({ linewidth: 3, clippingPlanes: this.clipPlanes })
    mat.resolution.set(this.viewW, this.viewH)
    const line = new LineSegments2(this.curveGeometry(pts), mat)
    line.frustumCulled = false // 表示範囲の中の線（視錐台の判定のための外接球を求めない）
    // 点は、光の当たった小さな球として空間に置く（画面に貼った円ではなく、遠近で大きさが変わり、面の陰に隠れる）
    const dotMat = new THREE.MeshStandardMaterial({ roughness: 0.35, metalness: 0.05, clippingPlanes: this.clipPlanes })
    const dots = this.makeDots(pts, dotMat)
    line.add(dots)
    this.plots.set(id, { kind: 'curve', line, dots })
    this.scene.add(line)
  }

  /**
   * 曲線の値の色付け（表示していない座標の値。等値面を何枚も重ねた曲線）。点ごとの値と範囲を覚えておき、色は [`colorCurve`] で塗る。
   * 値が無ければ、式の色 1 色
   */
  private setCurveValues(line: LineSegments2, mesh: MeshData) {
    const vr = mesh.valueRange
    line.userData.values =
      vr && mesh.complexValues.length === mesh.positions.length ? { pts: mesh.positions, v: mesh.complexValues, range: vr, logRange: logRange(vr, mesh.complexValues) } : null
  }

  /** 値で色を付ける曲線なら、線分の両端の色を塗る（色の対応表・式の色が変わったときも塗り直す） */
  private colorCurve(line: LineSegments2, style: PlotStyle, which: keyof typeof COLORMAP_INDEX = style.hidden?.colormap ?? 'viridis'): boolean {
    const d = line.userData.values as { pts: Float32Array; v: Float32Array; range: [number, number]; logRange?: [number, number] } | null
    const m = line.material as LineMaterial
    if (!d) {
      m.vertexColors = false
      return false
    }
    const log = style.hidden?.log === true && d.logRange !== undefined
    const range = log ? d.logRange! : d.range
    const { pts, v } = d
    const ok = (k: number) => Number.isFinite(pts[k]) && Number.isFinite(pts[k + 1]) && Number.isFinite(pts[k + 2])
    const base = new THREE.Color(style.color)
    const tmp = new THREE.Color()
    const cols: number[] = []
    // curveGeometry と同じ順に、点 k と k + 1 を結ぶ線分ごとに両端の色を並べる
    for (let k = 0; k + 5 < pts.length; k += 3) {
      if (!(ok(k) && ok(k + 3))) continue
      for (const q of [k, k + 3]) {
        colormapColor(valueT(v[q], range, log), which, base, tmp)
        cols.push(tmp.r, tmp.g, tmp.b)
      }
    }
    ;(line.geometry as LineSegmentsGeometry).setColors(cols)
    m.vertexColors = true
    m.needsUpdate = true
    return true
  }

  /** 式ごとの最後の見た目（後から作ったふたにも同じ色を付けるため） */
  private styles = new Map<number, PlotStyle>()
  private limitSources = new Map<number, {
    line: LineSegments2
    dots: THREE.InstancedMesh<THREE.BufferGeometry, THREE.MeshStandardMaterial>
  }>()

  private setLimitSource(id: number, positions?: Float32Array) {
    const old = this.limitSources.get(id)
    if (old) {
      this.scene.remove(old.line)
      old.line.geometry.dispose()
      old.line.material.dispose()
      old.dots.dispose()
      old.dots.material.dispose()
      this.limitSources.delete(id)
    }
    if (!positions?.length) return
    const d = this.domain
    const span = d ? Math.max(d.xmax - d.xmin, d.ymax - d.ymin) : 20
    const mat = new LineMaterial({
      color: this.theme.text, linewidth: 2, dashed: true, dashSize: span * 0.011, gapSize: span * 0.008,
      clippingPlanes: this.clipPlanes, transparent: true, depthWrite: false,
      polygonOffset: true, polygonOffsetFactor: -1, polygonOffsetUnits: -1,
    })
    mat.resolution.set(this.viewW, this.viewH)
    const line = new LineSegments2(this.curveGeometry(positions), mat)
    if (line.geometry.getAttribute('instanceStart')) line.computeLineDistances()
    else line.geometry.instanceCount = 0
    line.frustumCulled = false
    line.raycast = () => {}
    line.renderOrder = 5
    const dots = this.makeDots(positions, new THREE.MeshStandardMaterial({
      color: this.theme.text, roughness: 0.35, clippingPlanes: this.clipPlanes,
    }))
    line.add(dots)
    this.limitSources.set(id, { line, dots })
    this.scene.add(line)
  }

  /** 穴と定義値の丸印（式ごと）。曲線を GPU で描くときも見えるよう、図形とは別に置く */
  private holeMarks = new Map<number, THREE.Points>()

  /** 穴には白丸、極限の近づく点での定義値には塗りつぶした丸を置く（画面の上の大きさは一定） */
  private setHoles(id: number, mesh: MeshData) {
    const old = this.holeMarks.get(id)
    if (old) {
      this.scene.remove(old)
      old.geometry.dispose()
      ;(old.material as THREE.Material).dispose()
      this.holeMarks.delete(id)
    }
    const open = mesh.holes?.length ?? 0
    const filled = mesh.limitPoint?.length ?? 0
    if (open + filled === 0) return
    const positions = filled ? new Float32Array(open + filled) : mesh.holes!
    const closed = new Float32Array(positions.length / 3)
    if (filled) {
      if (mesh.holes) positions.set(mesh.holes)
      positions.set(mesh.limitPoint!, open)
      closed.fill(1, open / 3)
    }
    const geo = new THREE.BufferGeometry()
    geo.setAttribute('position', new THREE.BufferAttribute(positions, 3))
    geo.setAttribute('aFilled', new THREE.BufferAttribute(closed, 1))
    const mat = new THREE.ShaderMaterial({
      uniforms: {
        uColor: { value: new THREE.Color(this.styles.get(id)?.color ?? '#2b6cb0') },
        uBg: { value: new THREE.Color(this.theme.background) },
        uInk: { value: new THREE.Color(this.theme.text) },
        uSize: { value: 11 * this.renderer.getPixelRatio() },
      },
      vertexShader: `
        uniform float uSize;
        attribute float aFilled;
        varying float vFilled;
        void main() {
          vFilled = aFilled;
          gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
          // 線・面と同じ深さで重なってちらつかないよう、少し手前に出す
          gl_Position.z -= 2e-3 * gl_Position.w;
          gl_PointSize = uSize;
        }`,
      fragmentShader: `
        uniform vec3 uColor;
        uniform vec3 uBg;
        uniform vec3 uInk;
        varying float vFilled;
        void main() {
          float r = length(gl_PointCoord - 0.5) * 2.0;
          if (r > 1.0) discard;
          float ring = smoothstep(0.5, 0.62, r);
          vec3 color = mix(mix(uBg, uColor, ring), uInk, vFilled);
          gl_FragColor = vec4(color, 1.0 - smoothstep(0.88, 1.0, r));
          // 色は線形の値で持っているので、画面の色空間（sRGB）に直す（線・面の材質と同じ色に見えるように）
          #include <colorspace_fragment>
        }`,
      transparent: true,
    })
    const pts = new THREE.Points(geo, mat)
    pts.frustumCulled = false
    pts.renderOrder = 10
    pts.userData.limit = mesh.limitSource !== undefined
    pts.raycast = () => {}
    pts.visible = this.styles.get(id)?.visible ?? true
    this.holeMarks.set(id, pts)
    this.scene.add(pts)
  }

  /** 色・表示/非表示・格子線などの見た目だけを変える（計算し直さない） */
  setStyle(id: number, style: PlotStyle) {
    this.setStyleInner(id, style)
    this.styleGpu2d(id, style)
    this.styleGpu3d(id, style)
  }

  private setStyleInner(id: number, style: PlotStyle) {
    const p = this.plots.get(id)
    if (!p) return
    this.styles.set(id, style)
    this.dirty = true
    const c = new THREE.Color(style.color)
    const trail = this.rotationTrails.get(id)
    if (trail?.kind === 'surface') trail.surface.setStyle(c, style.visible)
    else if (trail) {
      trail.lines.visible = trail.dots.visible = style.visible
      if (!style.visible) trail.lastPoint = undefined
      trail.lines.material.color.copy(c)
      trail.dots.material.color.copy(c)
    }
    const holes = this.holeMarks.get(id)
    const showLimitSource = style.visible && !style.limitSourceHidden
    const source = this.limitSources.get(id)
    if (source) source.line.visible = showLimitSource
    if (holes) {
      holes.visible = holes.userData.limit ? showLimitSource : style.visible
      ;(holes.material as THREE.ShaderMaterial).uniforms.uColor.value.set(holes.userData.limit ? this.theme.text : c)
    }

    if (p.kind === 'curve') {
      p.line.visible = style.visible
      const m = p.line.material as LineMaterial
      // 値で色を付ける曲線は、頂点の色をそのまま出す（材質の色は白にして掛け合わせない）
      m.color.copy(this.colorCurve(p.line, style) ? new THREE.Color(1, 1, 1) : c)
      m.linewidth = style.lineWidth
      ;(p.dots.material as THREE.MeshStandardMaterial).color.copy(c)
      // 線の太さ 3 で 1 倍。細くしても小さくなりすぎないように
      p.dots.userData.scale = Math.max(0.7, style.lineWidth / 3)
      this.placeDots(p.dots)
      return
    }

    if (p.group.visible !== style.visible) {
      this.sceneChanged = true
      this.quality.forget() // 重い式を隠したら、下げた画質を覚えたままにしない
    }
    p.group.visible = style.visible
    const [mesh0] = p.group.children as THREE.Mesh[]
    ;(mesh0.material as THREE.MeshStandardMaterial).color.copy(c)
    p.grid.uColormap.value = COLORMAP_INDEX[p.vivid ? 'vivid' : (style.hidden?.colormap ?? 'viridis')]
    p.grid.uValueLog.value = style.hidden?.log ? 1 : 0
    if (p.volume) applyVolumeStyle(p.volume.u, style.hidden, c, style.opacity)
    const opacity = Math.min(1, Math.max(0, style.opacity))
    // 境目の線・流線を塗る。列ベクトルと固有ベクトルの軸は矢じりと同じ不透明度にする。
    for (const l of p.outlines) {
      const material = l.material as LineMaterial
      material.color.copy(this.colorCurve(l, style, p.vivid ? 'vivid' : undefined) ? new THREE.Color(1, 1, 1) : c)
      if (l.userData.vectorShaft) {
        material.opacity = opacity
        material.depthWrite = opacity === 1
        if (material.transparent !== (opacity < 1)) {
          material.transparent = opacity < 1
          material.needsUpdate = true
        }
      }
    }
    // 切り口のふたは、少し明るい色で塗る（曲面の裏側と見分けやすいように）
    const capColor = c.clone().lerp(new THREE.Color(0xffffff), 0.3)
    if (p.cap) (p.cap.material as THREE.MeshStandardMaterial).color.copy(capColor)
    if (p.stencil) (p.stencil.quad.material as THREE.MeshStandardMaterial).color.copy(capColor)

    // 面の不透明度。1 未満なら半透明として描き、奥の面や線も透けて見えるようにする
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
      // 面の濃さ 0 なら、ふたも見えなくする（透明でも描いて、ステンシルは 0 に戻す）
      q.opacity = opacity
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
    // ベクトル場の矢印には引かない（細い矢印の上の筋になって、色が見えにくくなる）
    const arrows = p.vivid || p.outlines.some((line) => line.userData.vectorShaft)
    const sp = (v: number) => (g.enabled && !arrows && Number.isFinite(v) && v > 0 ? v : 0)
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
    this.clearRotationTrail(id)
    this.setLimitSource(id)
    this.setHoles(id, { holes: undefined } as MeshData)
    this.setPoi(id, undefined)
    this.curvePts.delete(id)
    if (this.pickFor === id) this.showPick(null)
    this.removeGpu2d(id)
    this.removeGpu3d(id)
    this.gpuRejected.delete(id)
    const p = this.plots.get(id)
    if (!p) return
    this.dirty = true
    this.sceneChanged = true
    if (p.kind === 'curve') {
      p.line.geometry.dispose()
      ;(p.line.material as THREE.Material).dispose()
      p.dots.dispose()
      ;(p.dots.material as THREE.Material).dispose()
      this.scene.remove(p.line)
    } else {
      for (const o of p.group.children as THREE.Mesh[]) {
        // ステンシルの inc / dec は曲面と形のデータを共有しているので、材質だけを捨てる
        if (!p.stencil || (o !== p.stencil.inc && o !== p.stencil.dec)) o.geometry.dispose()
        ;(o.material as THREE.Material).dispose()
      }
      p.volume?.u.uTex.value.dispose()
      disposeGeometry(p.geo) // 粗い版を描いている最中でも、両方の版を捨てる
      this.scene.remove(p.group)
    }
    this.plots.delete(id)
    this.styles.delete(id)
    this.morphSources.delete(id)
  }
}

/**
 * 曲線の点の列のうち、長さの無い部分（NaN で区切られた一続きの点がすべて同じ位置）を、点として描く位置の列 [x, y, z, …] にする。
 * 複素平面の点（1 + 2i など）や組 (1, 2, 3) は、同じ位置の点の列として届く
 */
function dotPositions(pts: Float32Array): number[] {
  const out: number[] = []
  const ok = (k: number) => Number.isFinite(pts[k]) && Number.isFinite(pts[k + 1]) && Number.isFinite(pts[k + 2])
  let k = 0
  while (k + 2 < pts.length) {
    if (!ok(k)) {
      k += 3
      continue
    }
    // 一続きの点 [k, end) が、最初の点からどれだけ離れているか
    const [x, y, z] = [pts[k], pts[k + 1], pts[k + 2]]
    let end = k
    let same = true
    while (end + 2 < pts.length && ok(end)) {
      const d = Math.abs(pts[end] - x) + Math.abs(pts[end + 1] - y) + Math.abs(pts[end + 2] - z)
      if (d > 1e-9 * (1 + Math.abs(x) + Math.abs(y) + Math.abs(z))) same = false
      end += 3
    }
    if (same) out.push(x, y, z)
    k = end
  }
  return out
}

/** 点を描く球の形（半径 1。1つだけ作って使い回す） */
let dotSphereGeo: THREE.BufferGeometry | null = null
function dotSphere(): THREE.BufferGeometry {
  if (!dotSphereGeo) dotSphereGeo = new THREE.SphereGeometry(1, 24, 16)
  return dotSphereGeo
}

/** 点群でも他の点と同じ材質を使い、球の面数を抑える。 */
let denseDotSphereGeo: THREE.BufferGeometry | null = null
function denseDotSphere(): THREE.BufferGeometry {
  return denseDotSphereGeo ??= new THREE.SphereGeometry(1, 6, 4)
}
