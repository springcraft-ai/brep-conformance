// The shape's faces, shaded as metal under a studio light by WebGL2, from scan.ts's orthographic
// camera. The lights are fixed to the view, so the shape keeps one look whichever way it is turned.
// Plain WebGL2, so the page ships no 3D library.
import type { Camera } from './scan'

export interface Mesh {
  // xyz per vertex.
  positions: Float32Array
  // A unit normal per vertex, or zeros where the triangle's own normal is to be used.
  normals: Float32Array
  triangles: Uint32Array
}

export interface MeshLayer {
  set(mesh: Mesh | null): void
  draw(c: Camera, dpr: number): void
  destroy(): void
}

const VERTEX = `#version 300 es
in vec3 aPos;
in vec3 aNormal;
uniform vec3 uCenter;
uniform vec3 uRight;
uniform vec3 uUp;
uniform vec3 uToward;
uniform vec3 uScale;
out vec3 vNormal;
out vec3 vView;
void main() {
  vec3 d = aPos - uCenter;
  vView = vec3(dot(d, uRight), dot(d, uUp), dot(d, uToward));
  gl_Position = vec4(vView.x * uScale.x, vView.y * uScale.y, -vView.z * uScale.z, 1.0);
  vNormal = vec3(dot(aNormal, uRight), dot(aNormal, uUp), dot(aNormal, uToward));
}`

// View space: x right, y up, z toward the viewer. A vertex with no normal takes its triangle's own,
// from the screen-space slope of its position. A face seen from behind is lit as its front, so a
// mesh wound either way, or an open one, still reads as a surface.
const FRAGMENT = `#version 300 es
precision highp float;
in vec3 vNormal;
in vec3 vView;
uniform vec3 uBase;
out vec4 color;
const vec3 KEY = vec3(-0.45, 0.75, 0.55);
const vec3 FILL = vec3(0.7, -0.25, 0.45);
void main() {
  vec3 n = dot(vNormal, vNormal) > 1e-6 ? normalize(vNormal) : normalize(cross(dFdx(vView), dFdy(vView)));
  if (n.z < 0.0) n = -n;
  vec3 key = normalize(KEY);
  float light = 0.08 + 0.22 * (n.y * 0.5 + 0.5)
    + 0.8 * max(dot(n, key), 0.0) + 0.22 * max(dot(n, normalize(FILL)), 0.0);
  float shine = 0.55 * pow(max(dot(n, normalize(key + vec3(0.0, 0.0, 1.0))), 0.0), 48.0);
  float rim = pow(1.0 - max(n.z, 0.0), 3.0);
  vec3 c = uBase * light + shine + rim * vec3(0.1, 0.13, 0.18);
  color = vec4(pow(min(c, vec3(1.0)), vec3(1.0 / 1.15)), 1.0);
}`

// Null where the browser has no WebGL2. `onRestore` asks for a redraw once a lost context is back.
export function createMeshLayer(
  canvas: HTMLCanvasElement,
  base: [number, number, number],
  onRestore: () => void,
): MeshLayer | null {
  const gl = canvas.getContext('webgl2', { antialias: true, premultipliedAlpha: true })
  if (!gl) return null
  let mesh: Mesh | null = null
  let state = build(gl)
  if (!state) return null

  const lost = (e: Event) => {
    e.preventDefault()
    state = null
  }
  const restored = () => {
    state = build(gl)
    if (state && mesh) upload(gl, state, mesh)
    onRestore()
  }
  canvas.addEventListener('webglcontextlost', lost)
  canvas.addEventListener('webglcontextrestored', restored)

  return {
    set(next) {
      mesh = next
      if (state) upload(gl, state, next)
    },
    draw(c, dpr) {
      const w = Math.round(c.w * dpr)
      const h = Math.round(c.h * dpr)
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w
        canvas.height = h
      }
      gl.viewport(0, 0, w, h)
      gl.clearColor(0, 0, 0, 0)
      gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT)
      if (!state || !state.count || !c.w || !c.h) return
      // The frame's bounding sphere, which camera() fits to 0.44 of the shorter side.
      const radius = (0.44 * Math.min(c.w, c.h)) / c.scale
      const { program, u, vao } = state
      gl.enable(gl.DEPTH_TEST)
      gl.useProgram(program)
      gl.uniform3fv(u.center, c.center)
      gl.uniform3fv(u.right, c.right)
      gl.uniform3fv(u.up, c.up)
      gl.uniform3fv(u.toward, c.toward)
      gl.uniform3f(u.scale, (2 * c.scale) / c.w, (2 * c.scale) / c.h, 1 / (radius * 1.01))
      gl.uniform3f(u.base, base[0] / 255, base[1] / 255, base[2] / 255)
      gl.bindVertexArray(vao)
      gl.drawElements(gl.TRIANGLES, state.count, gl.UNSIGNED_INT, 0)
      gl.bindVertexArray(null)
    },
    destroy() {
      canvas.removeEventListener('webglcontextlost', lost)
      canvas.removeEventListener('webglcontextrestored', restored)
      if (state) {
        for (const b of state.buffers) gl.deleteBuffer(b)
        gl.deleteVertexArray(state.vao)
        gl.deleteProgram(state.program)
      }
      state = null
    },
  }
}

interface State {
  program: WebGLProgram
  u: Record<'center' | 'right' | 'up' | 'toward' | 'scale' | 'base', WebGLUniformLocation | null>
  vao: WebGLVertexArrayObject
  buffers: [WebGLBuffer, WebGLBuffer, WebGLBuffer]
  count: number
}

function compile(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader {
  const s = gl.createShader(type)!
  gl.shaderSource(s, source)
  gl.compileShader(s)
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS) && !gl.isContextLost())
    throw new Error(`scan shader: ${gl.getShaderInfoLog(s)}`)
  return s
}

// Null only while the context is lost; a shader that does not build is a bug, and throws.
function build(gl: WebGL2RenderingContext): State | null {
  if (gl.isContextLost()) return null
  const program = gl.createProgram()
  gl.attachShader(program, compile(gl, gl.VERTEX_SHADER, VERTEX))
  gl.attachShader(program, compile(gl, gl.FRAGMENT_SHADER, FRAGMENT))
  gl.linkProgram(program)
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    if (gl.isContextLost()) return null
    throw new Error(`scan shader: ${gl.getProgramInfoLog(program)}`)
  }
  const at = (name: string) => gl.getUniformLocation(program, name)
  return {
    program,
    u: {
      center: at('uCenter'),
      right: at('uRight'),
      up: at('uUp'),
      toward: at('uToward'),
      scale: at('uScale'),
      base: at('uBase'),
    },
    vao: gl.createVertexArray(),
    buffers: [gl.createBuffer(), gl.createBuffer(), gl.createBuffer()],
    count: 0,
  }
}

function upload(gl: WebGL2RenderingContext, s: State, mesh: Mesh | null) {
  s.count = 0
  if (!mesh || !mesh.triangles.length) return
  // Zeros, which the shader reads as "use the face's own", where the normals do not match.
  const normals =
    mesh.normals.length === mesh.positions.length
      ? mesh.normals
      : new Float32Array(mesh.positions.length)
  gl.bindVertexArray(s.vao)
  for (const [i, data] of [mesh.positions, normals].entries()) {
    gl.bindBuffer(gl.ARRAY_BUFFER, s.buffers[i]!)
    gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW)
    const loc = gl.getAttribLocation(s.program, i ? 'aNormal' : 'aPos')
    gl.enableVertexAttribArray(loc)
    gl.vertexAttribPointer(loc, 3, gl.FLOAT, false, 0, 0)
  }
  gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, s.buffers[2])
  gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, mesh.triangles, gl.STATIC_DRAW)
  gl.bindVertexArray(null)
  s.count = mesh.triangles.length
}
