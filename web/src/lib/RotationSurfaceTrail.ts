import * as THREE from 'three'
import type { SurfaceTransparency } from './SurfaceTransparency'
import type { VolumeUniforms } from './volumeMaterial'

const FRAME_LIMIT = 16
const TRIANGLE_LIMIT = 5000
const CLUSTER_CELLS = 14

/** 軌跡用に細かい面をまとめる。点を間引く場合も、元の三角形のつながりから面を作る。 */
function surfaceTriangles(geometry: THREE.BufferGeometry): Float32Array {
  const position = geometry.getAttribute('position')
  const indices = geometry.getIndex()
  if (!position || !indices?.count) return new Float32Array()
  const xyz = (i: number) => [position.getX(i), position.getY(i), position.getZ(i)]
  const triangles: number[] = []
  if (indices.count <= TRIANGLE_LIMIT * 3) {
    for (let i = 0; i < indices.count; i += 3) {
      const points = [...xyz(indices.getX(i)), ...xyz(indices.getX(i + 1)), ...xyz(indices.getX(i + 2))]
      if (points.every(Number.isFinite)) triangles.push(...points)
    }
    return new Float32Array(triangles)
  }

  const bounds = new THREE.Box3()
  const point = new THREE.Vector3()
  for (let i = 0; i < position.count; i++) {
    point.fromBufferAttribute(position, i)
    if (Number.isFinite(point.x) && Number.isFinite(point.y) && Number.isFinite(point.z)) bounds.expandByPoint(point)
  }
  const { x: width, y: height, z: depth } = bounds.getSize(point)
  const span = Math.max(width, height, depth)
  if (!(span > 0)) return new Float32Array()

  const cells = CLUSTER_CELLS
  const cellCount = cells ** 3
  const cellOf = new Int32Array(position.count).fill(-1)
  const representatives: number[][] = []
  const coordinate = (v: number, min: number) => Math.min(cells - 1, Math.floor((v - min) / span * cells))
  for (let i = 0; i < position.count; i++) {
    point.fromBufferAttribute(position, i)
    if (!Number.isFinite(point.x) || !Number.isFinite(point.y) || !Number.isFinite(point.z)) continue
    const x = coordinate(point.x, bounds.min.x), y = coordinate(point.y, bounds.min.y), z = coordinate(point.z, bounds.min.z)
    const cell = x + cells * (y + cells * z)
    cellOf[i] = cell
    representatives[cell] ??= [point.x, point.y, point.z]
  }

  const faces = new Set<number>()
  for (let i = 0; i < indices.count; i += 3) {
    const a = cellOf[indices.getX(i)], b = cellOf[indices.getX(i + 1)], c = cellOf[indices.getX(i + 2)]
    if (a < 0 || b < 0 || c < 0 || a === b || a === c || b === c) continue
    const lo = Math.min(a, b, c), hi = Math.max(a, b, c)
    const key = lo + cellCount * (a + b + c - lo - hi + cellCount * hi)
    if (faces.has(key)) continue
    faces.add(key)
    triangles.push(...representatives[a], ...representatives[b], ...representatives[c])
  }
  if (triangles.length <= TRIANGLE_LIMIT * 9) return new Float32Array(triangles)
  const stride = triangles.length / (TRIANGLE_LIMIT * 9)
  const reduced = new Float32Array(TRIANGLE_LIMIT * 9)
  for (let i = 0; i < TRIANGLE_LIMIT; i++) {
    const start = 9 * Math.floor(i * stride)
    reduced.set(triangles.slice(start, start + 9), i * 9)
  }
  return reduced
}

/** 霧で描く不等式にはメッシュが無いので、格子から領域の境界を作る。 */
export function regionTrailTriangles(volume: VolumeUniforms): Float32Array {
  const { data, width: n } = volume.uTex.value.image
  if (!data) return new Float32Array()
  const min = volume.uRayMin.value
  const span = volume.uRayMax.value.clone().sub(min)
  const boxMin = volume.uBoxMin.value
  const boxSpan = volume.uBoxMax.value.clone().sub(boxMin)
  const cells = Math.min(12, n)
  const samples = (axis: number) => Array.from({ length: cells }, (_, i) => {
    const coord = min.getComponent(axis) + span.getComponent(axis) * (i + 0.5) / cells
    const index = (coord - boxMin.getComponent(axis)) / boxSpan.getComponent(axis) * (n - 1)
    return Math.max(0, Math.min(n - 1, Math.round(index)))
  })
  const sx = samples(0), sy = samples(1), sz = samples(2)
  const inside = (x: number, y: number, z: number) => {
    if (x < 0 || x >= cells || y < 0 || y >= cells || z < 0 || z >= cells) return false
    const k = 2 * (sx[x] + n * (sy[y] + n * sz[z]))
    return data[k + 1] !== 0 && THREE.DataUtils.fromHalfFloat(data[k]) <= 0
  }
  const out: number[] = []
  for (let z = 0; z < cells; z++) for (let y = 0; y < cells; y++) for (let x = 0; x < cells; x++) {
    if (!inside(x, y, z)) continue
    for (let axis = 0; axis < 3; axis++) {
      for (const side of [-1, 1]) {
        const neighbor = [x, y, z]
        neighbor[axis] += side
        if (inside(neighbor[0], neighbor[1], neighbor[2])) continue
        const u = (axis + 1) % 3, v = (axis + 2) % 3
        for (const [du, dv] of [[0, 0], [1, 0], [1, 1], [0, 0], [1, 1], [0, 1]]) {
          const point = [x, y, z]
          point[axis] += Number(side > 0)
          point[u] += du
          point[v] += dv
          out.push(min.x + span.x * point[0] / cells, min.y + span.y * point[1] / cells, min.z + span.z * point[2] / cells)
        }
      }
    }
  }
  return new Float32Array(out)
}

/** 現在の曲面と重なる最後のサンプルは、次の姿勢を記録してから表示する。 */
export class RotationSurfaceTrail {
  readonly group = new THREE.Group()
  private readonly frames: THREE.Mesh<THREE.BufferGeometry, THREE.MeshBasicMaterial>[] = []
  private readonly triangles = new WeakMap<THREE.BufferGeometry, Float32Array>()

  record(surface: THREE.Mesh, geometry: THREE.BufferGeometry, cap: THREE.Mesh | null, region: Float32Array | undefined,
    color: string, clippingPlanes: THREE.Plane[], transparency: SurfaceTransparency): boolean {
    const sources = cap ? [[surface, geometry], [cap, cap.geometry]] as const : [[surface, geometry]] as const
    const positions: number[] = []
    const point = new THREE.Vector3()
    for (const [mesh, source] of sources) {
      let triangles = this.triangles.get(source)
      if (!triangles) {
        triangles = surfaceTriangles(source)
        this.triangles.set(source, triangles)
      }
      mesh.updateWorldMatrix(true, false)
      for (let i = 0; i < triangles.length; i += 3) {
        point.fromArray(triangles, i).applyMatrix4(mesh.matrixWorld)
        positions.push(point.x, point.y, point.z)
      }
    }
    const regionSize = region?.length ?? 0
    if (!positions.length && !regionSize) return false
    const vertices = new Float32Array(positions.length + regionSize)
    vertices.set(positions)
    if (region) vertices.set(region, positions.length)
    const snapshot = new THREE.BufferGeometry()
    snapshot.setAttribute('position', new THREE.BufferAttribute(vertices, 3))
    const material = new THREE.MeshBasicMaterial({
      color, transparent: true, depthWrite: false, side: THREE.DoubleSide, clippingPlanes,
    })
    transparency.register(material)
    const mesh = new THREE.Mesh(snapshot, material)
    mesh.raycast = () => {}
    this.group.add(mesh)
    this.frames.push(mesh)
    if (this.frames.length > FRAME_LIMIT) this.remove(this.frames.shift()!)
    const newest = this.frames.length - 1
    for (const [i, frame] of this.frames.entries()) {
      frame.visible = i < newest
      frame.material.opacity = 0.025 + 0.115 * (1 - (newest - 1 - i) / (FRAME_LIMIT - 1))
    }
    return true
  }

  setClipping(planes: THREE.Plane[]) {
    for (const frame of this.frames) frame.material.clippingPlanes = planes
  }

  setStyle(color: THREE.Color, visible: boolean) {
    this.group.visible = visible
    for (const frame of this.frames) frame.material.color.copy(color)
  }

  private remove(frame: (typeof this.frames)[number]) {
    this.group.remove(frame)
    frame.geometry.dispose()
    frame.material.dispose()
  }

  dispose() {
    for (const frame of this.frames) this.remove(frame)
    this.frames.length = 0
  }
}
