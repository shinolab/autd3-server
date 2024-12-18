struct VertexOutput {
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
    @builtin(position) position: vec4<f32>,
};

@group(0)
@binding(0)
var<uniform> proj_view: mat4x4<f32>;

@vertex
fn vs_main(
    @location(0) position: vec4<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) model_matrix_0: vec4<f32>,
    @location(3) model_matrix_1: vec4<f32>,
    @location(4) model_matrix_2: vec4<f32>,
    @location(5) model_matrix_3: vec4<f32>,
    @location(6) color: vec4<f32>,
) -> VertexOutput {
    let model_matrix = mat4x4<f32>(
        model_matrix_0,
        model_matrix_1,
        model_matrix_2,
        model_matrix_3,
    );
    var result: VertexOutput;
    result.tex_coord = tex_coord;
    result.position = proj_view * model_matrix * position;
    result.color = color;
    return result;
}

@group(0)
@binding(1)
var r_color: texture_2d<f32>;

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return vertex.color * textureLoad(r_color, vec2<i32>(vertex.tex_coord), 0);
}
