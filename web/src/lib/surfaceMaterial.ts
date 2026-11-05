// 曲面用の材質。Three.js の標準の材質に「格子線」を描く処理を足したもの。
//
// 格子線はメッシュの三角形とは関係なく、画素ごとに
// 「格子用の座標 gridCoord の各成分が、間隔のちょうど整数倍に近いか」を調べて線の色を混ぜている。
// そのため、どんな形の曲面にも同じように線が引ける。
// - 陽関数・陰関数: gridCoord = (x, y, z)。z 方向の線は z = f(x, y) では等高線になる
// - 媒介変数表示の曲面: gridCoord = (u − u0, v − v0, z)。曲面に沿った u, v の格子になる

import * as THREE from 'three'

/** シェーダーに渡す値。表面用・裏面用の2つの材質で共有する。 */
export interface GridUniforms {
  /** x, y, z 方向の線の間隔（0 なら描かない） */
  uGridSpacing: { value: THREE.Vector3 }
  uGridColor: { value: THREE.Color }
  /** 線の濃さ（0〜1） */
  uGridStrength: { value: number }
  /** 線の太さ（画素） */
  uGridWidth: { value: number }
  /** 面の不透明度（0 なら格子線だけが見える） */
  uFillOpacity: { value: number }
}

export function createGridUniforms(): GridUniforms {
  return {
    uGridSpacing: { value: new THREE.Vector3() },
    uGridColor: { value: new THREE.Color() },
    uGridStrength: { value: 0.75 },
    uGridWidth: { value: 1 },
    uFillOpacity: { value: 1 },
  }
}

const FRAGMENT_HEADER = /* glsl */ `
varying vec3 vGridPos;
uniform vec3 uGridSpacing;
uniform vec3 uGridColor;
uniform float uGridStrength;
uniform float uGridWidth;
uniform float uFillOpacity;

// 座標 coord が spacing の整数倍の近くなら 1、離れていれば 0（太さ uGridWidth 画素）
float gridLine(float coord, float spacing) {
  if (spacing <= 0.0) return 0.0;
  float c = coord / spacing;
  float w = fwidth(c);                      // 1画素で c がどれだけ変わるか
  float d = abs(fract(c - 0.5) - 0.5) / w;  // 一番近い線までの距離（画素）
  float hw = 0.5 * uGridWidth;               // 太さの半分
  float line = (1.0 - smoothstep(hw - 0.5, hw + 0.5, d)) * min(uGridWidth, 1.0);
  return line * (1.0 - smoothstep(0.2, 0.5, w)); // 線が細かすぎて潰れるときは薄くする
}
`

const FRAGMENT_APPLY = /* glsl */ `
{
  float g = max(max(gridLine(vGridPos.x, uGridSpacing.x),
                    gridLine(vGridPos.y, uGridSpacing.y)),
                gridLine(vGridPos.z, uGridSpacing.z));
  diffuseColor.rgb = mix(diffuseColor.rgb, uGridColor, g * uGridStrength);
  // 面は uFillOpacity の濃さ、線の上は不透明にする。ほぼ透明な所は描かない
  diffuseColor.a = max(uFillOpacity, g);
  if (diffuseColor.a < 0.01) discard;
}
`

export function createSurfaceMaterial(
  side: THREE.Side,
  grid: GridUniforms,
  clippingPlanes: THREE.Plane[],
): THREE.MeshStandardMaterial {
  const m = new THREE.MeshStandardMaterial({
    side,
    roughness: side === THREE.BackSide ? 0.7 : 0.55,
    metalness: side === THREE.BackSide ? 0.0 : 0.05,
    clippingPlanes,
  })
  // Three.js が作るシェーダーのコードに、格子線の処理を差し込む
  m.onBeforeCompile = (shader) => {
    Object.assign(shader.uniforms, grid)
    shader.vertexShader = shader.vertexShader
      .replace('#include <common>', '#include <common>\nattribute vec3 gridCoord;\nvarying vec3 vGridPos;')
      .replace(
        '#include <begin_vertex>',
        '#include <begin_vertex>\nvGridPos = gridCoord;',
      )
    shader.fragmentShader = shader.fragmentShader
      .replace('#include <common>', '#include <common>\n' + FRAGMENT_HEADER)
      .replace('#include <color_fragment>', '#include <color_fragment>\n' + FRAGMENT_APPLY)
  }
  m.customProgramCacheKey = () => 'surface-with-grid'
  return m
}
