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
  // The overlay's plane, box and lines; its points; and those outside or with no answer.
  guide: Rgb
  dot: Rgb
  quiet: Rgb
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

// The overlay's marks, its arrows and balls, as solids.
const MARK_VERTEX = `#version 300 es
in vec3 a_pos;
in vec3 a_nrm;
uniform vec3 u_center, u_right, u_up, u_toward, u_scale;
out vec3 v_nrm;
void main() {
  vec3 d = a_pos - u_center;
  gl_Position = vec4(dot(d, u_right) * u_scale.x, dot(d, u_up) * u_scale.y,
                     -dot(d, u_toward) * u_scale.z, 1.0);
  v_nrm = a_nrm;
}`
// Lit softly, mostly by ambient as a ghost is, so a ball reads as a mark and not as a part.
const MARK = `#version 300 es
precision highp float;
in vec3 v_nrm;
uniform vec3 u_light;
uniform vec4 u_color;
out vec4 o;
void main() {
  float d = max(dot(normalize(v_nrm), u_light), 0.0);
  o = vec4(u_color.rgb * (0.75 + 0.25 * d) * u_color.a, u_color.a);
}`

// A flat arrow, a shaft and a head in one plane through its axis, turned about the axis each frame
// to face the viewer as squarely as it can. Being flat in space, it foreshortens as it turns, its
// head shortening as it points toward or away from the viewer, as a drawn arrow would.
const RIBBON_VERTEX = `#version 300 es
in vec3 a_pos;
in vec3 a_b;
in vec2 a_k;
uniform vec3 u_center, u_right, u_up, u_toward, u_scale;
uniform vec3 u_shape;
void main() {
  vec3 d = a_b - a_pos;
  float len = length(d);
  vec3 t = len > 0.0 ? d / len : vec3(0.0, 0.0, 1.0);
  vec3 w = cross(t, u_toward);
  w = length(w) > 1e-3 ? normalize(w) : u_right;
  float head = u_shape.y * len;
  vec3 base = a_pos + t * (len - head);
  vec3 p;
  if (a_k.x < 1.5) p = mix(a_pos, base, a_k.x) + w * a_k.y * u_shape.x * len;
  else if (a_k.x < 2.5) p = a_b;
  else p = base + w * a_k.y * u_shape.z * len;
  vec3 q = p - u_center;
  gl_Position = vec4(dot(q, u_right) * u_scale.x, dot(q, u_up) * u_scale.y,
                     -dot(q, u_toward) * u_scale.z, 1.0);
}`

// A guide line as a band of fixed width on screen, which a GL line, one pixel wide, cannot be.
const WIDE_VERTEX = `#version 300 es
in vec3 a_pos;
in vec3 a_b;
in vec2 a_k;
uniform vec3 u_center, u_right, u_up, u_toward, u_scale;
uniform vec2 u_px;
uniform float u_width;
vec3 clip(vec3 p) {
  vec3 d = p - u_center;
  return vec3(dot(d, u_right) * u_scale.x, dot(d, u_up) * u_scale.y, -dot(d, u_toward) * u_scale.z);
}
void main() {
  vec3 a = clip(a_pos);
  vec3 b = clip(a_b);
  vec2 sa = a.xy * u_px;
  vec2 sb = b.xy * u_px;
  float len = length(sb - sa);
  vec2 t = len > 1e-4 ? (sb - sa) / len : vec2(1.0, 0.0);
  vec2 s = mix(sa, sb, a_k.x) + vec2(-t.y, t.x) * a_k.y * 0.5 * u_width;
  gl_Position = vec4(s / u_px, mix(a.z, b.z, a_k.x), 1.0);
}`
// The overlay as the GPU draws it, in model units: a plane as two triangles and its outline, a box
// as its twelve edges, guide lines and the dashes of what a drawing hides, arrows, and points as
// balls in three colours.
export interface OverlayGeometry {
  plane: Float32Array
  planeEdges: Float32Array
  box: Float32Array
  points: Float32Array
  lines: Float32Array
  hiddenLines: Float32Array
  // The sticks of classify probes: part of a mark, so faint where hidden, never dashed.
  sticks: Float32Array
  // A drawing's seen stretches, drawn solid where visible and never dashed.
  seen: Float32Array
  // Each arrow as one segment, from its point to its tip.
  arrows: Float32Array
  dashes: Float32Array
  inside: Float32Array
  quiet: Float32Array
  // A point's ball, its radius in model units.
  ball: number
}

const PLANE_ALPHA = 0.16
const GUIDE_ALPHA = 0.85
const HIDDEN_ALPHA = 0.35
// A guide line's width in CSS pixels.
const LINE_WIDTH = 2
// A mark's opacity where nothing hides it; an arrow's shaft half width, head length and head half
// width, each as a share of its length.
const MARK_ALPHA = 0.8
const ARROW_SHAPE: [number, number, number] = [0.03, 0.3, 0.1]

type OverlayPart =
  | 'plane'
  | 'planeEdges'
  | 'box'
  | 'points'
  | 'inside'
  | 'quiet'
  | 'lines'
  | 'hiddenLines'
  | 'dashes'
  | 'arrows'
  | 'sticks'
  | 'seen'

interface Part {
  vao: WebGLVertexArrayObject | null
  buffers: WebGLBuffer[]
  count: number
}

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
  gl.bindAttribLocation(p, 2, 'a_b')
  gl.bindAttribLocation(p, 3, 'a_k')
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

// Segments as the wide-line shader takes them: each vertex holds both ends of its segment and its
// place on it, the start (0) or end (1) and its side (−1 or 1); with `head`, also a head's tip (2)
// and its base either side (3). The shader widens them.
export function wideOf(ends: Float32Array, head = false): Float32Array {
  const shaft = [
    [0, -1],
    [1, -1],
    [1, 1],
    [0, -1],
    [1, 1],
    [0, 1],
  ]
  const tip = [
    [2, 0],
    [3, -1],
    [3, 1],
  ]
  const places = head ? [...shaft, ...tip] : shaft
  const out = new Float32Array((ends.length / 6) * places.length * 8)
  let o = 0
  for (let i = 0; i + 5 < ends.length; i += 6)
    for (const [along, side] of places) {
      out.set(ends.subarray(i, i + 6), o)
      out[o + 6] = along!
      out[o + 7] = side!
      o += 8
    }
  return out
}

// A ball of radius `r` round each centre, as triangles winding counter-clockwise from outside.
export function sphereMesh(
  centres: Float32Array,
  r: number,
): { pos: Float32Array; nrm: Float32Array } {
  const [RINGS, SIDES] = [10, 20]
  const pos: number[] = []
  const nrm: number[] = []
  const at = (i: number, j: number) => {
    const phi = (Math.PI * i) / RINGS
    const th = (2 * Math.PI * j) / SIDES
    return [Math.sin(phi) * Math.cos(th), Math.sin(phi) * Math.sin(th), Math.cos(phi)]
  }
  for (let c = 0; c + 2 < centres.length; c += 3) {
    const o = [centres[c]!, centres[c + 1]!, centres[c + 2]!]
    const put = (n: number[]) => {
      pos.push(...n.map((x, k) => o[k]! + r * x))
      nrm.push(...n)
    }
    for (let i = 0; i < RINGS; i++)
      for (let j = 0; j < SIDES; j++) {
        const [a, b, cc, d] = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]
        if (i > 0) for (const n of [a, b, d]) put(n)
        if (i < RINGS - 1) for (const n of [b, cc, d]) put(n)
      }
  }
  return { pos: Float32Array.from(pos), nrm: Float32Array.from(nrm) }
}

export class Renderer {
  private gl: WebGL2RenderingContext
  private faces: WebGLProgram
  private lines: WebGLProgram
  private far: WebGLProgram
  private wide: WebGLProgram
  private marks: WebGLProgram
  private ribbon: WebGLProgram
  private subject: Uploaded | null = null
  private alt: Uploaded | null = null
  private ghosts: { mesh: Uploaded; tone: 0 | 1 }[] = []
  // The overlay's parts, each laid out for the program that draws it.
  private overlay: Partial<Record<OverlayPart, Part>> = {}

  // Throws where the browser has no WebGL2.
  constructor(canvas: HTMLCanvasElement) {
    const gl = canvas.getContext('webgl2', { antialias: true, stencil: true, alpha: true })
    if (!gl) throw new Error('no WebGL2')
    this.gl = gl
    this.faces = compile(gl, VERTEX, FACES)
    this.lines = compile(gl, VERTEX, LINES)
    this.far = compile(gl, FAR_VERTEX, FAR)
    this.wide = compile(gl, WIDE_VERTEX, LINES)
    this.marks = compile(gl, MARK_VERTEX, MARK)
    this.ribbon = compile(gl, RIBBON_VERTEX, LINES)
  }

  setOverlay(o: OverlayGeometry) {
    const gl = this.gl
    this.freeOverlay()
    // One buffer per attribute list: [data, [location, size, stride, offset] for each attribute].
    const part = (lists: [Float32Array, number[][]][], vertices: number): Part => {
      const vao = gl.createVertexArray()
      gl.bindVertexArray(vao)
      const buffers = lists.map(([data, attributes]) => {
        const b = gl.createBuffer()!
        gl.bindBuffer(gl.ARRAY_BUFFER, b)
        gl.bufferData(gl.ARRAY_BUFFER, data, gl.STATIC_DRAW)
        for (const [at, size, stride, offset] of attributes) {
          gl.enableVertexAttribArray(at!)
          gl.vertexAttribPointer(at!, size!, gl.FLOAT, false, stride! * 4, offset! * 4)
        }
        return b
      })
      gl.vertexAttrib3f(1, 0, 0, 1)
      gl.bindVertexArray(null)
      return { vao, buffers, count: vertices }
    }
    const plain = (data: Float32Array) => part([[data, [[0, 3, 0, 0]]]], data.length / 3)
    const wide = (data: Float32Array) =>
      part(
        [
          [
            data,
            [
              [0, 3, 8, 0],
              [2, 3, 8, 3],
              [3, 2, 8, 6],
            ],
          ],
        ],
        data.length / 8,
      )
    const solid = (...meshes: { pos: Float32Array; nrm: Float32Array }[]) => {
      const join = (k: 'pos' | 'nrm') => {
        const out = new Float32Array(meshes.reduce((n, m) => n + m[k].length, 0))
        let at = 0
        for (const m of meshes) {
          out.set(m[k], at)
          at += m[k].length
        }
        return out
      }
      const [pos, nrm] = [join('pos'), join('nrm')]
      return part(
        [
          [pos, [[0, 3, 0, 0]]],
          [nrm, [[1, 3, 0, 0]]],
        ],
        pos.length / 3,
      )
    }
    this.overlay = {
      plane: plain(o.plane),
      planeEdges: plain(o.planeEdges),
      box: plain(o.box),
      points: solid(sphereMesh(o.points, o.ball)),
      inside: solid(sphereMesh(o.inside, o.ball)),
      quiet: solid(sphereMesh(o.quiet, o.ball)),
      lines: wide(wideOf(o.lines)),
      hiddenLines: wide(wideOf(o.hiddenLines)),
      dashes: wide(wideOf(o.dashes)),
      arrows: wide(wideOf(o.arrows, true)),
      sticks: wide(wideOf(o.sticks)),
      seen: wide(wideOf(o.seen)),
    }
  }

  private freeOverlay() {
    for (const p of Object.values(this.overlay)) {
      this.gl.deleteVertexArray(p.vao)
      for (const b of p.buffers) this.gl.deleteBuffer(b)
    }
    this.overlay = {}
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
      gl.uniform2f(u('u_px'), v.w / 2, v.h / 2)
      if (p === this.faces || p === this.marks) {
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
    // is in front of it, then its outline and the box. A drawing's hidden stretches are behind the
    // solid by definition, so their dashes skip the test.
    const P = this.overlay
    const draw = (part: Part | undefined, mode: number) => {
      if (!part?.count) return
      gl.bindVertexArray(part.vao)
      gl.drawArrays(mode, 0, part.count)
    }
    const band = (width: number) => {
      const c = uniforms(this.wide)
      gl.uniform1f(gl.getUniformLocation(this.wide, 'u_width'), width * v.dpr)
      return c
    }
    gl.uniform4f(color, ...rgba(look.guide, PLANE_ALPHA))
    draw(P.plane, gl.TRIANGLES)
    gl.uniform4f(color, ...rgba(look.guide, GUIDE_ALPHA))
    draw(P.planeEdges, gl.LINES)
    draw(P.box, gl.LINES)
    if (P.dashes?.count) {
      gl.disable(gl.DEPTH_TEST)
      gl.uniform4f(band(LINE_WIDTH), ...rgba(look.guide, GUIDE_ALPHA))
      draw(P.dashes, gl.TRIANGLES)
      gl.enable(gl.DEPTH_TEST)
    }
    // The guide lines and marks, twice: where the solid hides them, as CAD draws hidden geometry,
    // since a query's points sit inside the solid or on its faces, the lines dashed and the marks
    // faint; then where nothing does. Marks are drawn light and a little clear, as a drawing's
    // marks are, not as parts of the solid.
    // Where balls overlap the first drawn shows, so the points asked about and those on the
    // boundary come first, then those inside, then the rest.
    const balls: [Part | undefined, Rgb][] = [
      [P.points, look.dot],
      [P.inside, look.guide],
      [P.quiet, look.quiet],
    ]
    for (const hidden of [true, false]) {
      gl.depthFunc(hidden ? gl.GREATER : gl.LEQUAL)
      gl.uniform4f(band(LINE_WIDTH), ...rgba(look.guide, GUIDE_ALPHA))
      draw(hidden ? P.hiddenLines : P.lines, gl.TRIANGLES)
      if (!hidden) draw(P.seen, gl.TRIANGLES)
      // Each pixel takes one mark only, so where marks overlap they do not blend over each other.
      gl.clear(gl.STENCIL_BUFFER_BIT)
      gl.enable(gl.STENCIL_TEST)
      gl.stencilFunc(gl.EQUAL, 0, 0xff)
      gl.stencilOp(gl.KEEP, gl.KEEP, gl.INCR)
      const k = hidden ? HIDDEN_ALPHA : MARK_ALPHA
      const c = uniforms(this.marks)
      gl.enable(gl.CULL_FACE)
      gl.cullFace(gl.BACK)
      for (const [part, rgb] of balls) {
        gl.uniform4f(c, ...rgba(rgb, k))
        draw(part, gl.TRIANGLES)
      }
      gl.disable(gl.CULL_FACE)
      // After the balls, so a probe's stick runs behind its balls and does not cut through them.
      gl.uniform4f(band(LINE_WIDTH), ...rgba(look.guide, k))
      draw(P.sticks, gl.TRIANGLES)
      const r = uniforms(this.ribbon)
      gl.uniform3f(gl.getUniformLocation(this.ribbon, 'u_shape'), ...ARROW_SHAPE)
      gl.uniform4f(r, ...rgba(look.guide, k))
      draw(P.arrows, gl.TRIANGLES)
      gl.disable(gl.STENCIL_TEST)
    }
    gl.depthFunc(gl.LEQUAL)
    gl.depthMask(true)
    gl.bindVertexArray(null)
  }

  destroy() {
    this.free(this.subject)
    this.free(this.alt)
    for (const g of this.ghosts) this.free(g.mesh)
    for (const p of [this.faces, this.lines, this.far, this.wide, this.marks, this.ribbon])
      this.gl.deleteProgram(p)
    this.freeOverlay()
  }
}
