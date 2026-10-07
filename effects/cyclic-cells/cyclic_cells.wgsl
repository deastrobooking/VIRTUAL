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
    let original=source(input.uv);
    let aspect=globals.texel_size.y/max(globals.texel_size.x,0.000001);
    let grid=vec2(round(param(0u)*aspect),param(0u));
    let cell=floor(input.uv*grid); let center=(cell+.5)/grid;
    let sampled=source(center);
    var cells: array<u32,25>; var next: array<u32,9>;
    let tick=floor(globals.time_seconds*param(4u));
    for(var y=0u;y<5u;y++) {for(var x=0u;x<5u;x++) {
        let neighbor=cell+vec2(f32(x)-2.0,f32(y)-2.0);
        let c=source((neighbor+.5)/grid);
        let value=luma(c);
        let states=u32(round(param(6u))); var state=u32(floor(clamp(value+param(1u)*.4,0.0,.999)*f32(states))); if hash(neighbor+tick+7.0)<param(2u) {state=(state+1u)%states;}
        cells[y*5u+x]=state;
    }}
    for(var y=0u;y<3u;y++) {for(var x=0u;x<3u;x++) {
        let current=cells[(y+1u)*5u+x+1u]; var count=0u;
        for(var dy=0u;dy<3u;dy++) {for(var dx=0u;dx<3u;dx++) {
            if dx!=1u || dy!=1u { let neighbor=cells[(y+dy)*5u+x+dx]; if neighbor==(current+1u)%u32(round(param(6u))) {count+=1u;} }
        }}
        var state=current; if count>=u32(round(param(7u))) {state=(current+1u)%u32(round(param(6u)));}
        next[y*3u+x]=state;
    }}
    var state=next[4u];
    if param(5u)>1.5 {
        let current=next[4u]; var count=0u;
        for(var i=0u;i<9u;i++) {if i!=4u {let neighbor=next[i]; if neighbor==(current+1u)%u32(round(param(6u))) {count+=1u;} }}
        if count>=u32(round(param(7u))) {state=(current+1u)%u32(round(param(6u)));}
    }
    let phase=f32(state)/max(param(6u)-1.0,1.0); let palette=.5+.5*cos(6.2831853*(phase+param(8u)+vec3(0.0,.33,.67))); let rgb=mix(sampled.rgb,palette,.65);
    let inner=abs(fract(input.uv*grid)-.5);
    var edge=1.0;
    if param(3u)>0.0 {edge=1.0-smoothstep(.5-param(3u),.5,max(inner.x,inner.y));}
    let result=vec4(rgb*edge,sampled.a);
    return mix(original,result,globals.mix_amount);
}
