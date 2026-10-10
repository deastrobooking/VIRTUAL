struct MasterEffectGlobals {
    direction: vec2<f32>,
    texel_size: vec2<f32>,
    radius: f32,
    mix_amount: f32,
    mode: u32,
    feedback: f32,
    time_seconds: f32,
    parameter_count: u32,
    pass_index: u32,
    pass_count: u32,
    parameters: array<vec4<f32>, 8>,
    history_valid: u32,
}

@group(0) @binding(0) var effect_sampler: sampler;
@group(0) @binding(1) var original_texture: texture_2d<f32>;
@group(0) @binding(2) var effect_texture: texture_2d<f32>;
@group(0) @binding(3) var<uniform> globals: MasterEffectGlobals;
@group(0) @binding(4) var history_texture: texture_2d<f32>;
@group(0) @binding(5) var custom_history_texture: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    let positions = array(
        vec2(-1.0, -1.0),
        vec2( 3.0, -1.0),
        vec2(-1.0,  3.0),
    );
    let uvs = array(
        vec2(0.0, 1.0),
        vec2(2.0, 1.0),
        vec2(0.0, -1.0),
    );
    var output: VertexOutput;
    output.position = vec4(positions[index], 0.0, 1.0);
    output.uv = uvs[index];
    return output;
}

fn param(index: u32) -> f32 {
    return globals.parameters[index / 4u][index % 4u];
}

fn lifted_luma(uv: vec2<f32>) -> f32 {
    let c = textureSampleLevel(original_texture, effect_sampler, uv, 0.0);
    let luma = dot(c.rgb, vec3(0.2126, 0.7152, 0.0722));
    let exposed = max(luma - param(2u), 0.0) * param(1u);
    return pow(clamp(exposed, 0.0, 1.0), param(3u));
}

fn outline_color(palette: u32) -> vec3<f32> {
    if palette == 1u { return vec3(0.12, 0.92, 1.0); }
    if palette == 2u { return vec3(0.18, 1.0, 0.28); }
    if palette == 3u { return vec3(1.0, 0.62, 0.12); }
    return vec3(1.0);
}

fn phosphor_color(value: f32, palette: u32) -> vec3<f32> {
    if palette == 3u { return value * vec3(1.0, 0.55, 0.12); }
    if palette == 1u { return value * vec3(0.08, 0.72, 1.0); }
    return value * vec3(0.12, 1.0, 0.2);
}

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let original = textureSampleLevel(original_texture, effect_sampler, input.uv, 0.0);
    if original.a <= 0.0 {
        return vec4(0.0);
    }

    let step_uv = globals.texel_size * param(7u);
    let tl = lifted_luma(input.uv + vec2(-step_uv.x, -step_uv.y));
    let tc = lifted_luma(input.uv + vec2(0.0, -step_uv.y));
    let tr = lifted_luma(input.uv + vec2(step_uv.x, -step_uv.y));
    let ml = lifted_luma(input.uv + vec2(-step_uv.x, 0.0));
    let mr = lifted_luma(input.uv + vec2(step_uv.x, 0.0));
    let bl = lifted_luma(input.uv + vec2(-step_uv.x, step_uv.y));
    let bc = lifted_luma(input.uv + vec2(0.0, step_uv.y));
    let br = lifted_luma(input.uv + vec2(step_uv.x, step_uv.y));
    let center = lifted_luma(input.uv);

    // Sobel gradients find both vertical and horizontal crowd contours.
    let gx = -tl + tr - 2.0 * ml + 2.0 * mr - bl + br;
    let gy = -tl - 2.0 * tc - tr + bl + 2.0 * bc + br;
    let gradient = length(vec2(gx, gy)) * 0.25 * param(6u);
    let edge = smoothstep(param(4u), param(4u) + param(5u), gradient);
    let luma = smoothstep(param(4u), param(4u) + param(5u), center);
    let look = u32(round(param(0u)));
    let palette = u32(round(param(8u)));
    let ink = outline_color(palette);
    let mutepulse = select(1.0, 0.9 + 0.1 * sin(globals.time_seconds * 2.3), param(12u) > 0.5);
    let grain_tick = select(0.0, floor(globals.time_seconds * 24.0), param(12u) > 0.5);
    let grain = (hash(floor(input.uv / globals.texel_size) + vec2(grain_tick, 3.1)) - 0.5) * param(11u);

    var color = vec3(0.0);
    if look == 0u {
        // A clean outline against black; source detail can be mixed back in.
        color = ink * edge * (1.0 + param(10u)) * mutepulse;
    } else if look == 1u {
        // A brighter neon contour with a lifted antialiased edge.
        color = ink * edge * (1.25 + param(10u)) * mutepulse;
    } else if look == 2u {
        color = phosphor_color(center, palette);
        color += ink * edge * param(10u) * mutepulse;
    } else {
        color = vec3(luma);
        color += ink * edge * param(10u);
    }

    color = mix(color, original.rgb, clamp(param(9u), 0.0, 1.0));
    color = max(color + vec3(grain), vec3(0.0));
    return mix(original, vec4(color, original.a), clamp(globals.mix_amount, 0.0, 1.0));
}
