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


fn param(i: u32) -> f32 { return globals.parameters[i / 4u][i % 4u]; }
fn hash(p: vec2<f32>) -> f32 { return fract(sin(dot(p,vec2(127.1,311.7)))*43758.5453); }
fn mirror_uv(uv: vec2<f32>) -> vec2<f32> { return 1.0-abs(fract(uv*0.5)*2.0-1.0); }
fn turn(p: vec2<f32>,a: f32) -> vec2<f32> { return vec2(cos(a)*p.x-sin(a)*p.y,sin(a)*p.x+cos(a)*p.y); }
fn source(uv: vec2<f32>) -> vec4<f32> { return textureSampleLevel(original_texture,effect_sampler,mirror_uv(uv),0.0); }
fn luma(c: vec4<f32>) -> f32 {return dot(c.rgb,vec3(0.2126,0.7152,0.0722))*c.a;}
fn noise(p: vec2<f32>) -> f32 {
    let i=floor(p); let f=fract(p); let u=f*f*(3.0-2.0*f);
    return mix(mix(hash(i),hash(i+vec2(1.0,0.0)),u.x),mix(hash(i+vec2(0.0,1.0)),hash(i+vec2(1.0)),u.x),u.y);
}

@fragment fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let original=source(input.uv); let uv=input.uv; let time=globals.time_seconds*param(3u);
    let tick=floor(time); let mode=u32(round(param(1u))); let scale=param(2u);
    let block=floor(uv*scale); let row=floor(uv.y*scale); let seed=param(6u);
    var offset=vec2(hash(vec2(row,tick+seed))-.5,0.0)*param(0u);
    if mode==1u {offset=(vec2(hash(block+tick+seed),hash(block.yx+tick+31.0))-.5)*param(0u);}
    if mode==2u {offset=vec2(noise(vec2(uv.y*scale,time*.1+seed))-.5,sin(time*.1+uv.x*8.0)*.1)*param(0u);}
    if mode==3u {offset=(uv-.5)*(hash(vec2(tick,seed))-.5)*param(0u)*3.0;}
    var warped=source(uv+offset);
    if mode==4u {
        let a=source(uv+offset); let b=source(uv-offset);
        warped=vec4(a.r,original.g,b.b,max(a.a,max(original.a,b.a)));
    }
    var coord=floor(uv/globals.texel_size);
    if param(5u)>.5 {coord=block;}
    if param(5u)>1.5 {coord=vec2(row,0.0);}
    let grain=(hash(coord+vec2(tick,seed))-.5)*param(4u);
    let result=vec4(clamp(warped.rgb+grain,vec3(0.0),vec3(1.0)),warped.a);
    return mix(original,result,globals.mix_amount);
}
