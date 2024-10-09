struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    @builtin(position) position: vec4<f32>,
};

@group(0)
@binding(0)
var<uniform> proj_view: mat4x4<f32>;

@group(0)
@binding(1)
var<uniform> model: mat4x4<f32>;

@group(0)
@binding(2)
var texture: texture_storage_2d<rgba8unorm, read_write>;

@group(0)
@binding(3)
var<uniform> slice_size: vec2<f32>;

@group(0)
@binding(4)
var<storage, read> v_tr_pos: array<vec3<f32>>;

@group(0)
@binding(5)
var<storage, read> v_tr_state: array<vec4<f32>>;

struct Config {
    sound_speed: f32,
    num_trans: u32,
    max_pressure: f32,
    _pad: u32,
}

@group(0)
@binding(6)
var<uniform> config: Config;

@group(0)
@binding(7)
var color_map: texture_1d<f32>;

@vertex
fn vs_main(
    @location(0) position: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
) -> VertexOutput {
    var result: VertexOutput;
    result.tex_coord = tex_coord;
    result.position = proj_view * model * position;
    return result;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    var coord = vertex.tex_coord;
    coord.x *= slice_size.x;
    coord.y *= slice_size.y;
    return textureLoad(texture, vec2<i32>(coord));
}

const ULTRASOUND_FREQ: f32 = 40000;
const COLOR_MAP_TEXTURE_SIZE: f32 = 256;

const PI: f32 = radians(180.0);
const T4010A1_AMPLITUDE: f32 = 55114.85; // [Pa*mm]
const P0: f32 = T4010A1_AMPLITUDE / (4. * PI);

fn coloring(t: f32) -> vec4<f32> {
    return textureLoad(color_map, u32(clamp(t, 0.0, 1.0) * COLOR_MAP_TEXTURE_SIZE), 0);
}

@compute
@workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let id_x = i32(id.x);
    let id_y = i32(id.y);
    let width = i32(slice_size.x);
    let height = i32(slice_size.y);
    if id_x >= width || id_y >= height {
        return;
    }

    let x = f32(id.x) / slice_size.x - 0.5;
    let y = f32(id.y) / slice_size.y - 0.5;
    let point = (model * vec4(x, y, 0.0, 1.0)).xyz;

    let wavenum = 2 * PI * ULTRASOUND_FREQ / config.sound_speed;

    var re: f32 = 0.;
    var im: f32 = 0.;
    for (var i: u32 = 0; i < config.num_trans; i++) {
        let r = distance(v_tr_pos[i], point);

        let amp = v_tr_state[i].x;
        let phase = v_tr_state[i].y;
        let en = v_tr_state[i].z;

        let p = -phase - wavenum * r;
        let a = en * P0 * amp / r;
        re += a * cos(p);
        im += a * sin(p);
    }
    let c = sqrt(re * re + im * im) / config.max_pressure;
    textureStore(texture, vec2(id_x, id_y), coloring(c));
}