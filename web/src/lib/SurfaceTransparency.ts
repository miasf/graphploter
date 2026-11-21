import * as THREE from 'three'

type DrawObject = THREE.Object3D & { material: THREE.Material }

/**
 * Weighted blended transparency (McGuire & Bavoil, 2013).
 * Keep the opaque depth buffer and accumulate translucent fragments in any order.
 */
export class SurfaceTransparency {
  private readonly materials = new WeakSet<THREE.Material>()
  private readonly pass = { value: false }
  private readonly depth = { value: null as THREE.DepthTexture | null }
  private readonly depthScale = { value: new THREE.Vector4() }
  private readonly size = new THREE.Vector2()
  private readonly clearColor = new THREE.Color()
  private buffers: { base: THREE.WebGLRenderTarget; layers: THREE.WebGLRenderTarget } | null = null
  private readonly screen = new THREE.Scene()
  private readonly camera = new THREE.Camera()
  private readonly quad = new THREE.Mesh(new THREE.PlaneGeometry(2, 2))
  private readonly composite = new THREE.ShaderMaterial({
    transparent: true,
    depthTest: false,
    depthWrite: false,
    uniforms: { uColors: { value: null }, uDensity: { value: null } },
    vertexShader: 'void main() { gl_Position = vec4(position.xy, 0.0, 1.0); }',
    fragmentShader: `
      uniform sampler2D uColors;
      uniform sampler2D uDensity;
      void main() {
        ivec2 pixel = ivec2(gl_FragCoord.xy);
        vec4 sum = texelFetch(uColors, pixel, 0);
        float density = texelFetch(uDensity, pixel, 0).r;
        gl_FragColor = vec4(sum.rgb / max(sum.a, 1e-5), 1.0 - exp(-density));
      }
    `,
  })
  private readonly present = new THREE.ShaderMaterial({
    depthTest: false,
    depthWrite: false,
    uniforms: { uBase: { value: null } },
    vertexShader: this.composite.vertexShader,
    fragmentShader: `
      uniform sampler2D uBase;
      void main() {
        gl_FragColor = texelFetch(uBase, ivec2(gl_FragCoord.xy), 0);
        #include <colorspace_fragment>
      }
    `,
  })

  constructor(private renderer: THREE.WebGLRenderer) {
    this.quad.frustumCulled = false
    this.screen.add(this.quad)
  }

  register(material: THREE.Material, raycast = false) {
    this.materials.add(material)
    material.blendSrc = THREE.OneFactor
    material.blendDst = THREE.OneFactor
    const compile = material.onBeforeCompile
    // The default key reads onBeforeCompile dynamically, so capture it before wrapping.
    const standardKey = material.customProgramCacheKey()
    material.customProgramCacheKey = () => `${standardKey}:surface-transparency:${raycast}`
    material.onBeforeCompile = (shader, renderer) => {
      compile.call(material, shader, renderer)
      Object.assign(shader.uniforms, { uCollectSurface: this.pass, uOpaqueDepth: this.depth, uSurfaceDepthScale: this.depthScale })
      shader.fragmentShader = shader.fragmentShader
        .replace('#include <common>', `#include <common>
          uniform bool uCollectSurface;
          uniform sampler2D uOpaqueDepth;
          uniform vec4 uSurfaceDepthScale;
          layout(location = 1) out vec4 surfaceDensity;
        `)
        .replace(/}\s*$/, `
          surfaceDensity = vec4(0.0);
          if (uCollectSurface && ${material.colorWrite ? 'true' : 'false'}) {
            float depth = ${raycast ? 'gl_FragDepth' : 'gl_FragCoord.z'};
            if (depth > texelFetch(uOpaqueDepth, ivec2(gl_FragCoord.xy), 0).r) discard;
            float alpha = clamp(gl_FragColor.a, 0.0, 1.0);
            float viewDistance = uSurfaceDepthScale.x / (uSurfaceDepthScale.y - depth * uSurfaceDepthScale.z);
            float relativeDepth = viewDistance - uSurfaceDepthScale.w;
            // Normalize to the graph's size so orbiting and near-parallel projection use the same weights.
            float weight = exp2(-8.0 * clamp(relativeDepth, -0.5, 0.5));
            float contribution = alpha * alpha * weight;
            gl_FragColor = vec4(gl_FragColor.rgb * contribution, contribution);
            // Adding optical densities multiplies transmittance with the same blending mode as the colors.
            surfaceDensity.r = -log(max(1.0 - alpha, 1e-4));
          }
        }`)
    }
  }

  private resize() {
    const { x: width, y: height } = this.renderer.getDrawingBufferSize(this.size)
    if (!this.buffers) {
      const depth = new THREE.DepthTexture(width, height, THREE.UnsignedInt248Type)
      depth.format = THREE.DepthStencilFormat
      const base = new THREE.WebGLRenderTarget(width, height, {
        colorSpace: THREE.SRGBColorSpace,
        depthTexture: depth,
        stencilBuffer: true,
        samples: Math.min(4, this.renderer.capabilities.maxSamples),
        resolveStencilBuffer: false,
      })
      const layers = new THREE.WebGLRenderTarget(width, height, {
        type: THREE.HalfFloatType, count: 2, stencilBuffer: true,
      })
      if (this.renderer.extensions.has('EXT_color_buffer_float')) layers.textures[1].format = THREE.RedFormat
      this.composite.uniforms.uColors.value = layers.textures[0]
      this.composite.uniforms.uDensity.value = layers.textures[1]
      this.present.uniforms.uBase.value = base.texture
      this.buffers = { base, layers }
    }
    this.buffers.base.setSize(width, height)
    this.buffers.layers.setSize(width, height)
    return this.buffers
  }

  render(scene: THREE.Scene, camera: THREE.PerspectiveCamera, distance: number, span: number) {
    const renderer = this.renderer
    if (!renderer.extensions.has('EXT_color_buffer_float') && !renderer.extensions.has('EXT_color_buffer_half_float')) {
      renderer.render(scene, camera)
      return
    }
    const baseObjects: DrawObject[] = []
    const surfaceObjects: DrawObject[] = []
    const stencilObjects: DrawObject[] = []
    const overlays: DrawObject[] = []
    scene.traverseVisible((object) => {
      const draw = object as DrawObject
      const m = draw.material
      if (!m) return
      if (m.transparent && this.materials.has(m)) {
        if (m.colorWrite) surfaceObjects.push(draw)
        else stencilObjects.push(draw)
      } else if (m.transparent && (draw.renderOrder > 0 || !m.depthTest)) overlays.push(draw)
      else baseObjects.push(draw)
    })
    if (!surfaceObjects.length) {
      renderer.render(scene, camera)
      return
    }

    const { base, layers } = this.resize()
    this.depthScale.value.set(camera.near * camera.far / span, camera.far, camera.far - camera.near, distance / span)
    const background = scene.background
    const autoClear = renderer.autoClear
    const autoReset = renderer.info.autoReset
    const sortObjects = renderer.sortObjects
    const setVisible = (objects: DrawObject[], visible: boolean) => {
      for (const object of objects) object.visible = visible
    }
    const surfacePass = [...surfaceObjects, ...stencilObjects]
    renderer.info.reset()
    renderer.info.autoReset = false
    try {
      // Draw the opaque scene first. Its depth hides translucent fragments that fall behind it.
      setVisible(surfacePass, false)
      setVisible(overlays, false)
      renderer.setRenderTarget(base)
      renderer.render(scene, camera)
      base.resolveDepthBuffer = false

      setVisible(baseObjects, false)
      setVisible(surfacePass, true)
      scene.background = null
      renderer.autoClear = false
      renderer.setRenderTarget(layers)
      renderer.getClearColor(this.clearColor)
      const clearAlpha = renderer.getClearAlpha()
      renderer.setClearColor(0, 0)
      renderer.clear()
      renderer.setClearColor(this.clearColor, clearAlpha)
      this.depth.value = base.depthTexture
      this.pass.value = true
      for (const { material } of surfaceObjects) {
        material.blending = THREE.CustomBlending
        material.forceSinglePass = true
      }
      // Only stencil caps depend on ordering. Skipping the sort also avoids recomputing the bounds of moving meshes.
      renderer.sortObjects = sortObjects && stencilObjects.length > 0
      renderer.render(scene, camera)
      renderer.sortObjects = sortObjects
      this.pass.value = false
      this.depth.value = null

      // Composite into the base target so labels and overlays still use the opaque depth buffer.
      renderer.setRenderTarget(base)
      this.quad.material = this.composite
      renderer.render(this.screen, this.camera)
      if (overlays.length) {
        setVisible(surfacePass, false)
        setVisible(overlays, true)
        renderer.render(scene, camera)
      }
      renderer.setRenderTarget(null)
      this.quad.material = this.present
      renderer.render(this.screen, this.camera)
    } finally {
      this.pass.value = false
      this.depth.value = null
      for (const { material } of surfaceObjects) {
        material.blending = THREE.NormalBlending
        material.forceSinglePass = false
      }
      setVisible(surfacePass, true)
      setVisible(baseObjects, true)
      setVisible(overlays, true)
      scene.background = background
      base.resolveDepthBuffer = true
      renderer.setRenderTarget(null)
      renderer.autoClear = autoClear
      renderer.info.autoReset = autoReset
      renderer.sortObjects = sortObjects
    }
  }

  dispose() {
    this.buffers?.base.dispose()
    this.buffers?.layers.dispose()
    this.quad.geometry.dispose()
    this.composite.dispose()
    this.present.dispose()
  }
}
