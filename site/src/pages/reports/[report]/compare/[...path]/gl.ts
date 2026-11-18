// The Scan view's solid on the GPU with WebGL2: lit faces, edges over them, and ghosts behind. A
// ghost's fill shows only where the subject is not (by the stencil), and its edges only where the
// subject does not hide them (by the subject's depth). The page ships no 3D library.
import type { Vec3 } from './vec'

export interface Mesh {
  positions: Float32Array
  // A unit normal per vertex, or zeros where the triangle's own normal is to be used.
  normals: Float32Array
  triangles: Uint32Array
  // Two xyz ends per segment.
  edges: Float32Array
}

export type Rgb = [number, number, number]

export interface View {
  center: Vec3
  right: Vec3
  up: Vec3
  toward: Vec3
  // Device pixels per model unit, and the view's size in device pixels.
  scale: number
  w: number
  h: number
  // Half the depth the frame spans, so every depth maps inside the clip volume.
  reach: number
  // Device pixels per CSS pixel, for the point markers' size.
  dpr: number
}

export interface Look {
  face: Rgb
  // The second subject's faces: a part's neighbour in an assembly, a mirror image, a piece.
  faceAlt: Rgb
  edge: Rgb
  ghost: [Rgb, Rgb]
  // The overlay's plane and box, and its points.
  guide: Rgb
  dot: Rgb
}

// The share of a face's light that does not depend on its turn to the light: low for the subject,
// so its shading shows its shape; high for a ghost, so a face seen edge on does not thin it out.
const AMBIENT = 0.3
const GHOST_AMBIENT = 0.8
const GHOST_ALPHA = 0.22
const GHOST_EDGE_ALPHA = 0.55
// The subject's edges darken the face they cross rather than paint over it, so a one-pixel line
// split across two pixels still shows against a face in light or in shade.
const EDGE_ALPHA = 0.6

// Invariant, so a ghost's colour pass lands on exactly the depth its depth pass wrote.
const VERTEX = `#version 300 es
in vec3 a_pos;
in vec3 a_nrm;
uniform vec3 u_center, u_right, u_up, u_toward, u_scale;
out vec3 v_nrm;
invariant gl_Position;
void main() {
  vec3 d = a_pos - u_center;
  gl_Position = vec4(dot(d, u_right) * u_scale.x, dot(d, u_up) * u_scale.y,
                     -dot(d, u_toward) * u_scale.z, 1.0);
  v_nrm = a_nrm;
}`

// The whole view at the far depth, to clear the depth a ghost wrote where the subject is not.
const FAR_VERTEX = `#version 300 es
void main() {
  vec2 p = vec2(float((gl_VertexID & 1) << 2) - 1.0, float((gl_VertexID & 2) << 1) - 1.0);
  gl_Position = vec4(p, 1.0, 1.0);
}`
const FAR = `#version 300 es
precision highp float;
out vec4 o;
void main() { o = vec4(0.0); }`

// Both sides of a face are lit alike, so a mesh wound either way, or an open one, reads as a
// surface. The colour is written premultiplied, as the canvas composites it.
const FACES = `#version 300 es
precision highp float;
in vec3 v_nrm;
uniform vec3 u_light;
uniform vec4 u_color;
uniform float u_ambient;
out vec4 o;
void main() {
  float l = u_ambient + (1.0 - u_ambient) * abs(dot(normalize(v_nrm), u_light));
  o = vec4(u_color.rgb * l * u_color.a, u_color.a);
}`

// Edges are WebGL lines one device pixel wide, drawn after the faces with the ordinary depth
// test. A line is drawn only along its own centre, where it and the faces it bounds share their
// vertices and so their depth. A wider line would reach over the faces beside it, which a slant
// puts nearer than the edge. The overlay's plane and box are drawn with this flat colour too.
const LINES = `#version 300 es
precision highp float;
uniform vec4 u_color;
out vec4 o;
void main() { o = vec4(u_color.rgb * u_color.a, u_color.a); }`

// The overlay's points as round markers of a fixed size on screen, depth-tested like the rest,
// so a point on the far side of the solid is hidden by it.
const POINT_VERTEX = `#version 300 es
in vec3 a_pos;
uniform vec3 u_center, u_right, u_up, u_toward, u_scale;
uniform float u_size;
void main() {
  vec3 d = a_pos - u_center;
  gl_Position = vec4(dot(d, u_right) * u_scale.x, dot(d, u_up) * u_scale.y,
                     -dot(d, u_toward) * u_scale.z, 1.0);
  gl_PointSize = u_size;
}`
// A disc of the colour with a thin dark rim, so it reads on a face in light or shade.
const POINTS = `#version 300 es
precision highp float;
uniform vec4 u_color;
out vec4 o;
void main() {
  float r = length(gl_PointCoord * 2.0 - 1.0);
  if (r > 1.0) discard;
  vec3 c = r > 0.75 ? u_color.rgb * 0.35 : u_color.rgb;
  o = vec4(c * u_color.a, u_color.a);
}`

// The overlay as the GPU draws it, in model units: a plane as two triangles and its outline, a box
// as its twelve edges, the points as points.
export interface OverlayGeometry {
  plane: Float32Array
  planeEdges: Float32Array
  box: Float32Array
  points: Float32Array
}

const PLANE_ALPHA = 0.16
const GUIDE_ALPHA = 0.85
// A point marker's diameter in CSS pixels.
const POINT_SIZE = 9

interface Uploaded {
  faces: WebGLVertexArrayObject | null
  count: number
  indexed: boolean
  lines: WebGLVertexArrayObject | null
  segments: number
  buffers: WebGLBuffer[]
}

function compile(gl: WebGL2RenderingContext, vs: string, fs: string): WebGLProgram {
  const p = gl.createProgram()!
  for (const [type, src] of [
    [gl.VERTEX_SHADER, vs],
    [gl.FRAGMENT_SHADER, fs],
  ] as const) {
    const s = gl.createShader(type)!
    gl.shaderSource(s, src)
    gl.compileShader(s)
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s) ?? '')
    gl.attachShader(p, s)
  }
  gl.bindAttribLocation(p, 0, 'a_pos')
  gl.bindAttribLocation(p, 1, 'a_nrm')
  gl.linkProgram(p)
  if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p) ?? '')
  return p
}

// The mesh as the GPU draws it: indexed where every vertex has a normal, else one vertex per
// corner with its triangle's normal.
export function prepare(m: Mesh): {
  pos: Float32Array
  nrm: Float32Array
  idx: Uint32Array | null
} {
  let given = true
  for (let i = 0; i < m.normals.length && given; i += 3)
    if (!m.normals[i] && !m.normals[i + 1] && !m.normals[i + 2]) given = false
  if (given && m.normals.length === m.positions.length)
    return { pos: m.positions, nrm: m.normals, idx: m.triangles }
  const T = m.triangles
  const P = m.positions
  const pos = new Float32Array(T.length * 3)
  const nrm = new Float32Array(T.length * 3)
  for (let t = 0; t + 2 < T.length; t += 3) {
    const [a, b, c] = [T[t]! * 3, T[t + 1]! * 3, T[t + 2]! * 3]
    const u = [P[b]! - P[a]!, P[b + 1]! - P[a + 1]!, P[b + 2]! - P[a + 2]!]
    const v = [P[c]! - P[a]!, P[c + 1]! - P[a + 1]!, P[c + 2]! - P[a + 2]!]
    const n = [
      u[1]! * v[2]! - u[2]! * v[1]!,
      u[2]! * v[0]! - u[0]! * v[2]!,
      u[0]! * v[1]! - u[1]! * v[0]!,
    ]
    const len = Math.hypot(n[0]!, n[1]!, n[2]!) || 1
    for (let k = 0; k < 3; k++) {
      const at = T[t + k]! * 3
      pos.set([P[at]!, P[at + 1]!, P[at + 2]!], (t + k) * 3)
      nrm.set([n[0]! / len, n[1]! / len, n[2]! / len], (t + k) * 3)
    }
  }
  return { pos, nrm, idx: null }
}

export class Renderer {
  private gl: WebGL2RenderingContext
  private faces: WebGLProgram
  private lines: WebGLProgram
  private far: WebGLProgram
  private points: WebGLProgram
  private subject: Uploaded | null = null
  private alt: Uploaded | null = null
  private ghosts: { mesh: Uploaded; tone: 0 | 1 }[] = []
  // The overlay, each part as positions only: its array and how many vertices it holds.
  private overlay: { vao: WebGLVertexArrayObject | null; buffer: WebGLBuffer; count: number }[] = []

  // Throws where the browser has no WebGL2.
  constructor(canvas: HTMLCanvasElement) {
    const gl = canvas.getContext('webgl2', { antialias: true, stencil: true, alpha: true })
    if (!gl) throw new Error('no WebGL2')
    this.gl = gl
    this.faces = compile(gl, VERTEX, FACES)
    this.lines = compile(gl, VERTEX, LINES)
    this.far = compile(gl, FAR_VERTEX, FAR)
    this.points = compile(gl, POINT_VERTEX, POINTS)
  }

  // The overlay's plane, plane outline, box and points, in that order.
  setOverlay(o: OverlayGeometry) {
    const gl = this.gl
    for (const part of this.overlay) {
      gl.deleteVertexArray(part.vao)
      gl.deleteBuffer(part.buffer)
    }
    this.overlay = [o.plane, o.planeEdges, o.box, o.points].map(data => {
      const vao = gl.createVertexArray()
      gl.bindVertexArray(vao)
      const buffer = gl.createBuffer()!
      gl.bindBuffer(gl.ARRAY_BUFFER, buffer)
      gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW)
      gl.enableVertexAttribArray(0)
      gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0)
      gl.vertexAttrib3f(1, 0, 0, 1)
      gl.bindVertexArray(null)
      return { vao, buffer, count: data.length / 3 }
    })
  }

  private upload(m: Mesh): Uploaded {
    const gl = this.gl
    const buffers: WebGLBuffer[] = []
    const buffer = (target: number, data: ArrayBufferView) => {
      const b = gl.createBuffer()!
      gl.bindBuffer(target, b)
      gl.bufferData(target, data, gl.STATIC_DRAW)
      buffers.push(b)
      return b
    }
    const { pos, nrm, idx } = prepare(m)
    const faces = gl.createVertexArray()
    gl.bindVertexArray(faces)
    buffer(gl.ARRAY_BUFFER, pos)
    gl.enableVertexAttribArray(0)
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0)
    buffer(gl.ARRAY_BUFFER, nrm)
    gl.enableVertexAttribArray(1)
    gl.vertexAttribPointer(1, 3, gl.FLOAT, false, 0, 0)
    if (idx) buffer(gl.ELEMENT_ARRAY_BUFFER, idx)
    const lines = gl.createVertexArray()
    gl.bindVertexArray(lines)
    buffer(gl.ARRAY_BUFFER, m.edges)
    gl.enableVertexAttribArray(0)
    gl.vertexAttribPointer(0, 3, gl.FLOAT, false, 0, 0)
    gl.vertexAttrib3f(1, 0, 0, 1)
    gl.bindVertexArray(null)
    return {
      faces,
      count: idx ? idx.length : pos.length / 3,
      indexed: !!idx,
      lines,
      segments: m.edges.length / 6,
      buffers,
    }
  }

  private free(u: Uploaded | null) {
    if (!u) return
    const gl = this.gl
    gl.deleteVertexArray(u.faces)
    gl.deleteVertexArray(u.lines)
    for (const b of u.buffers) gl.deleteBuffer(b)
  }

  set(subject: Mesh, alt: Mesh, ghosts: (Mesh & { tone: 0 | 1 })[]) {
    this.free(this.subject)
    this.free(this.alt)
    for (const g of this.ghosts) this.free(g.mesh)
    this.subject = this.upload(subject)
    this.alt = this.upload(alt)
    this.ghosts = ghosts.map(g => ({ mesh: this.upload(g), tone: g.tone }))
  }

  draw(v: View, look: Look) {
    const gl = this.gl
    gl.viewport(0, 0, v.w, v.h)
    gl.clearColor(0, 0, 0, 0)
    gl.clearStencil(0)
    gl.clearDepth(1)
    gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT | gl.STENCIL_BUFFER_BIT)
    gl.enable(gl.DEPTH_TEST)
    gl.depthFunc(gl.LEQUAL)
    gl.enable(gl.BLEND)
    gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA)

    const light = [0, 1, 2].map(i => v.toward[i]! + 0.35 * v.up[i]! - 0.25 * v.right[i]!)
    const len = Math.hypot(...light)
    const uniforms = (p: WebGLProgram) => {
      gl.useProgram(p)
      const u = (n: string) => gl.getUniformLocation(p, n)
      gl.uniform3fv(u('u_center'), v.center)
      gl.uniform3fv(u('u_right'), v.right)
      gl.uniform3fv(u('u_up'), v.up)
      gl.uniform3fv(u('u_toward'), v.toward)
      gl.uniform3f(u('u_scale'), (2 * v.scale) / v.w, (2 * v.scale) / v.h, 1 / (2 * v.reach))
      if (p === this.faces) {
        gl.uniform3f(u('u_light'), light[0]! / len, light[1]! / len, light[2]! / len)
        gl.uniform1f(u('u_ambient'), AMBIENT)
      }
      return u('u_color')
    }
    const rgba = (c: Rgb, a: number) => [c[0] / 255, c[1] / 255, c[2] / 255, a] as const
    const fill = (m: Uploaded) => {
      gl.bindVertexArray(m.faces)
      if (m.indexed) gl.drawElements(gl.TRIANGLES, m.count, gl.UNSIGNED_INT, 0)
      else gl.drawArrays(gl.TRIANGLES, 0, m.count)
    }
    const stroke = (m: Uploaded) => {
      gl.bindVertexArray(m.lines)
      gl.drawArrays(gl.LINES, 0, m.segments * 2)
    }

    // The subject's faces turned toward the viewer, marking the stencil where they cover. A
    // solid's triangles are counter-clockwise seen from outside (display's contract), so the back
    // faces are culled and never meet the front ones near an edge.
    let color = uniforms(this.faces)
    gl.enable(gl.STENCIL_TEST)
    gl.enable(gl.CULL_FACE)
    gl.cullFace(gl.BACK)
    // Along an edge the line and its faces have one depth, a tie that rounding would settle a
    // sample at a time; the least offset settles it for the line.
    gl.enable(gl.POLYGON_OFFSET_FILL)
    gl.polygonOffset(1, 1)
    gl.depthMask(true)
    gl.stencilFunc(gl.ALWAYS, 1, 0xff)
    gl.stencilOp(gl.KEEP, gl.KEEP, gl.REPLACE)
    gl.uniform4f(color, ...rgba(look.face, 1))
    if (this.subject) fill(this.subject)
    gl.uniform4f(color, ...rgba(look.faceAlt, 1))
    if (this.alt) fill(this.alt)

    // Each ghost's fill where the subject is not, its nearest surface only: blending every layer
    // of it, front and back, in triangle order would band it by the order they came in. So its
    // depth first, then its colour where that depth is, then that depth cleared away again, so the
    // next ghost and the edges meet only the subject's.
    gl.disable(gl.CULL_FACE)
    gl.disable(gl.POLYGON_OFFSET_FILL)
    gl.stencilFunc(gl.EQUAL, 0, 0xff)
    gl.stencilOp(gl.KEEP, gl.KEEP, gl.KEEP)
    for (const g of this.ghosts) {
      color = uniforms(this.faces)
      gl.colorMask(false, false, false, false)
      gl.depthMask(true)
      gl.depthFunc(gl.LESS)
      fill(g.mesh)
      gl.colorMask(true, true, true, true)
      gl.depthMask(false)
      gl.depthFunc(gl.EQUAL)
      gl.uniform4f(color, ...rgba(look.ghost[g.tone], GHOST_ALPHA))
      gl.uniform1f(gl.getUniformLocation(this.faces, 'u_ambient'), GHOST_AMBIENT)
      fill(g.mesh)
      gl.useProgram(this.far)
      gl.colorMask(false, false, false, false)
      gl.depthMask(true)
      gl.depthFunc(gl.ALWAYS)
      gl.bindVertexArray(null)
      gl.drawArrays(gl.TRIANGLES, 0, 3)
      gl.colorMask(true, true, true, true)
    }
    gl.depthFunc(gl.LEQUAL)
    gl.depthMask(false)
    gl.disable(gl.STENCIL_TEST)

    // The ghosts' edges where the subject does not hide them, then the subject's over them.
    color = uniforms(this.lines)
    for (const g of this.ghosts) {
      gl.uniform4f(color, ...rgba(look.ghost[g.tone], GHOST_EDGE_ALPHA))
      stroke(g.mesh)
    }
    gl.uniform4f(color, ...rgba(look.edge, EDGE_ALPHA))
    if (this.subject) stroke(this.subject)
    if (this.alt) stroke(this.alt)

    // The overlay, depth-tested and writing no depth: the plane faint where nothing of the solid
    // is in front of it, then its outline and the box, then the points.
    const [plane, planeEdges, box, points] = this.overlay
    const draw = (part: (typeof this.overlay)[number] | undefined, mode: number) => {
      if (!part?.count) return
      gl.bindVertexArray(part.vao)
      gl.drawArrays(mode, 0, part.count)
    }
    gl.uniform4f(color, ...rgba(look.guide, PLANE_ALPHA))
    draw(plane, gl.TRIANGLES)
    gl.uniform4f(color, ...rgba(look.guide, GUIDE_ALPHA))
    draw(planeEdges, gl.LINES)
    draw(box, gl.LINES)
    if (points?.count) {
      const p = uniforms(this.points)
      gl.uniform1f(gl.getUniformLocation(this.points, 'u_size'), POINT_SIZE * v.dpr)
      gl.uniform4f(p, ...rgba(look.dot, 1))
      draw(points, gl.POINTS)
    }
    gl.depthMask(true)
    gl.bindVertexArray(null)
  }

  destroy() {
    this.free(this.subject)
    this.free(this.alt)
    for (const g of this.ghosts) this.free(g.mesh)
    this.gl.deleteProgram(this.faces)
    this.gl.deleteProgram(this.lines)
    this.gl.deleteProgram(this.far)
    this.gl.deleteProgram(this.points)
    for (const part of this.overlay) {
      this.gl.deleteVertexArray(part.vao)
      this.gl.deleteBuffer(part.buffer)
    }
  }
}
