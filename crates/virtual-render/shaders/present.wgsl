@group(0) @binding(0) var program_sampler: sampler;
@group(0) @binding(1) var program_texture: texture_2d<f32>;

struct PresentGlobals {
    content_scale: vec2<f32>,
    test_card: u32,
    identify: u32,
    mapping_rows: array<vec4<f32>, 3>,
    source_rect: vec4<f32>,
    masks: array<vec4<f32>, 4>,
    mapping_enabled: vec4<u32>,
    edge_blend: vec4<f32>,
    color_calibration: vec4<f32>,
}

@group(0) @binding(2) var<uniform> globals: PresentGlobals;

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

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    var source_uv = (input.uv - vec2(0.5)) / globals.content_scale + vec2(0.5);
    if globals.mapping_enabled.x != 0u {
        let p = vec3(input.uv, 1.0);
        let w = dot(globals.mapping_rows[2].xyz, p);
        if abs(w) < 0.00001 { return vec4(0.0, 0.0, 0.0, 1.0); }
        source_uv = vec2(dot(globals.mapping_rows[0].xyz, p), dot(globals.mapping_rows[1].xyz, p)) / w;
        for (var i = 0u; i < 4u; i += 1u) {
            let mask = globals.masks[i];
            if all(mask.zw > mask.xy) && all(input.uv >= mask.xy) && all(input.uv <= mask.zw) {
                return vec4(0.0, 0.0, 0.0, 1.0);
            }
        }
    }
    if any(source_uv < vec2(0.0)) || any(source_uv > vec2(1.0)) {
        return vec4(0.0, 0.0, 0.0, 1.0);
    }
    var sample_uv = source_uv;
    if globals.mapping_enabled.x != 0u {
        sample_uv = mix(globals.source_rect.xy, globals.source_rect.zw, source_uv);
    }
    var color = textureSampleLevel(program_texture, program_sampler, sample_uv, 0.0).rgb;
    if globals.test_card != 0u {
        let bars = array(
            vec3(0.75, 0.75, 0.75),
            vec3(0.75, 0.75, 0.0),
            vec3(0.0, 0.75, 0.75),
            vec3(0.0, 0.75, 0.0),
            vec3(0.75, 0.0, 0.75),
            vec3(0.75, 0.0, 0.0),
            vec3(0.0, 0.0, 0.75),
            vec3(0.04, 0.04, 0.04),
        );
        let bar = min(u32(source_uv.x * 8.0), 7u);
        color = bars[bar];
        let grid = min(
            abs(fract(source_uv.x * 16.0) - 0.5),
            abs(fract(source_uv.y * 9.0) - 0.5),
        );
        if grid > 0.485 {
            color *= 0.45;
        }
        if source_uv.y > 0.78 {
            color = vec3(source_uv.x);
        }
    }
    if globals.identify != 0u {
        let border = min(
            min(source_uv.x, 1.0 - source_uv.x),
            min(source_uv.y, 1.0 - source_uv.y),
        );
        let cross = min(abs(source_uv.x - 0.5), abs(source_uv.y - 0.5));
        if border < 0.012 || cross < 0.002 {
            color = vec3(1.0, 0.0, 0.7);
        }
    }
    if globals.mapping_enabled.x != 0u {
        let edge = globals.edge_blend;
        var weight = 1.0;
        if edge.x > 0.0 { weight *= smoothstep(0.0, edge.x, source_uv.x); }
        if edge.y > 0.0 { weight *= smoothstep(0.0, edge.y, source_uv.y); }
        if edge.z > 0.0 { weight *= smoothstep(0.0, edge.z, 1.0 - source_uv.x); }
        if edge.w > 0.0 { weight *= smoothstep(0.0, edge.w, 1.0 - source_uv.y); }
        let gamma = max(globals.color_calibration.x, 0.01);
        color = max(
            pow(max(color, vec3(0.0)), vec3(1.0 / gamma)),
            vec3(globals.color_calibration.y),
        ) * weight;
    }
    return vec4(color, 1.0);
}
