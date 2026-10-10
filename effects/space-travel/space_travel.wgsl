struct DeckEffectGlobals {
    time_seconds: f32,
    mix_amount: f32,
    parameter_count: u32,
    _padding: u32,
    parameters: array<vec4<f32>, 8>,
}

@group(0) @binding(0) var effect_sampler: sampler;
@group(0) @binding(1) var original_texture: texture_2d<f32>;
@group(0) @binding(2) var<uniform> globals: DeckEffectGlobals;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let positions = array(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    let uvs = array(vec2(0.0, 1.0), vec2(2.0, 1.0), vec2(0.0, -1.0));
    var output: VertexOutput;
    output.position = vec4(positions[index], 0.0, 1.0);
    output.uv = uvs[index];
    return output;
}

fn hash21(p: vec2<f32>) -> f32 {
    let h = vec3(p.xyx) * 0.1031;
    let q = h + dot(h, h.yzx + vec3(33.33));
    return fract((q.x + q.y) * q.z);
}

fn hue_rotate(color: vec3<f32>, angle: f32) -> vec3<f32> {
    let axis = normalize(vec3(1.0, 1.0, 1.0));
    return color * cos(angle) + cross(axis, color) * sin(angle)
        + axis * dot(axis, color) * (1.0 - cos(angle));
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let p0 = globals.parameters[0];
    let p1 = globals.parameters[1];
    let p2 = globals.parameters[2];
    let speed = p0.x;
    let animate = p0.y;
    let stretch = p0.z;
    let density = p0.w;
    let origin = p1.xy;
    let tint = p1.z;
    let brightness = p1.w;
    let source_mix = p2.x;
    let pattern = u32(round(p2.y));
    let time = globals.time_seconds * animate;
    let source = textureSample(original_texture, effect_sampler, input.uv);

    let aspect = vec2(1.0, 1.0);
    let delta = (input.uv - origin) * aspect;
    let radius = length(delta);
    let angle = atan2(delta.y, delta.x);
    let safe_radius = max(radius, 0.012);
    let depth = fract(1.0 - 0.38 / safe_radius - time * speed * 0.32);
    let layer_density = select(1.0, 1.65, pattern == 1u);
    let layer = floor(layer_density / safe_radius + time * speed * 1.7);
    let angle_density = select(28.0, select(42.0, 18.0, pattern == 2u), pattern == 1u);
    let angle_cell = floor((angle + 3.14159265) * angle_density);
    let seed = vec2(layer, angle_cell);
    let random = hash21(seed);
    let star_on = smoothstep(1.0 - density * 0.42, 1.0, random);
    let phase = fract(depth + random * 0.31);
    let core = exp(-phase * phase * (90.0 - stretch * 76.0));
    let trail = exp(-phase * (5.0 + (1.0 - stretch) * 34.0));
    let angular_falloff = smoothstep(1.0, 0.08, radius);
    let star = star_on * max(core, trail * stretch) * angular_falloff;
    let flicker = 0.72 + 0.28 * sin(time * (2.0 + random * 7.0) + random * 91.0);
    let rays = pow(max(0.0, 0.5 + 0.5 * cos(angle * 5.0 + time * 0.3)), 18.0)
        * (0.04 + 0.16 * stretch) * angular_falloff;

    var color = vec3(0.64, 0.82, 1.0) * star * flicker * brightness;
    color += vec3(0.15, 0.34, 0.7) * rays * brightness;
    color = hue_rotate(color, tint * 6.2831853);
    let starfield = vec4(source.rgb * (0.25 + 0.22 * (1.0 - source_mix)) + color, source.a);
    let composed = mix(starfield, source, source_mix);
    return mix(source, composed, clamp(globals.mix_amount, 0.0, 1.0));
}
