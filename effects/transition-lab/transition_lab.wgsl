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
    let original=source(input.uv); let progress=param(0u); let style=u32(round(param(1u)));
    if progress<=0.0 {return original;}
    if progress>=1.0 {return mix(original,vec4(0.0),globals.mix_amount);}
    let aspect=vec2(globals.texel_size.y/max(globals.texel_size.x,0.000001),1.0);
    let p=(input.uv-0.5)*aspect; let cell=floor(input.uv*param(3u));
    let random=hash(cell+param(6u)); let bend=sin(progress*3.14159265)*param(2u);
    var uv=input.uv; var threshold=random;
    if style==1u {uv=turn(p,param(5u)*bend*3.0)/(1.0+bend*3.0)/aspect+0.5; threshold=1.0-clamp(length(p)/length(aspect*.5),0.0,1.0);}
    if style==2u {uv+=vec2(random-.5,hash(cell.yx+13.0)-.5)*bend; threshold=random;}
    if style==3u {uv.x+=sin(cell.y*1.7)*bend*.3; threshold=input.uv.y;}
    if style==4u {uv=turn(p,bend*param(5u)*12.0*exp(-length(p)*3.0))/aspect+.5; threshold=clamp(length(p)/length(aspect*.5),0.0,1.0);}
    let edge_progress=progress*(1.0+2.0*param(4u))-param(4u);
    let coverage=(1.0-smoothstep(threshold-param(4u),threshold+param(4u),edge_progress))*(1.0-progress);
    let warped=source(uv);
    let result=vec4(warped.rgb*coverage,warped.a*coverage);
    return mix(original,result,globals.mix_amount);
}
