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

fn history(uv: vec2<f32>) -> vec4<f32> {
    if any(uv<vec2(0.0)) || any(uv>vec2(1.0)) {return vec4(0.0);}
    return textureSampleLevel(custom_history_texture,effect_sampler,uv,0.0);
}
@fragment fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let original=source(input.uv);
    if globals.history_valid==0u || param(0u)<=0.0 {return original;}
    let t=globals.time_seconds*param(3u); let tick=floor(t); let phase=fract(t);
    let seed=param(11u);
    let a=vec2(hash(vec2(tick,seed)),hash(vec2(seed,tick+17.0)))*2.0-1.0;
    let b=vec2(hash(vec2(tick+1.0,seed)),hash(vec2(seed,tick+18.0)))*2.0-1.0;
    let wander=mix(mix(a,b,phase*phase*(3.0-2.0*phase)),a,param(9u))*param(2u);
    let aspect=vec2(globals.texel_size.y/max(globals.texel_size.x,0.000001),1.0);
    var uv=(input.uv-0.5)*aspect;
    var echo=vec4(0.0); var weight=0.0;
    let count=u32(round(param(8u)));
    for(var i=0u;i<8u;i++) {
        if i>=count {break;}
        uv=turn(uv, param(6u))/(1.0+param(7u))-(wander+vec2(param(4u),param(5u)))*aspect;
        let w=pow(param(1u),f32(i+1u));
        let sample=history(uv/aspect+0.5);
        echo+=vec4(sample.rgb*sample.a,sample.a)*w; weight+=w;
    }
    echo/=max(weight,0.0001);
    let e=clamp(echo.rgb*param(0u)*param(1u),vec3(0.0),vec3(1.0));
    let o=original.rgb*original.a;
    var rgb=min(o+e,vec3(1.0));
    if param(10u)>1.5 {rgb=max(o,e);} else if param(10u)>.5 {rgb=1.0-(1.0-o)*(1.0-e);}
    let alpha=max(original.a,echo.a*param(0u)*param(1u));
    let result=vec4(rgb/max(alpha,0.00001),alpha);
    return mix(original,result,clamp(globals.mix_amount,0.0,1.0));
}
