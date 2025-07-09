#version 450

layout(set = 0, binding = 0) uniform texture2D inputImg;
layout(r8, set = 0, binding = 1) uniform writeonly image2D luma;
layout(r8, set = 0, binding = 2) uniform writeonly image2D chr_u;
layout(r8, set = 0, binding = 3) uniform writeonly image2D chr_v;
layout(r8, set = 0, binding = 4) uniform writeonly image2D alpha;

layout(set = 0, binding = 5) uniform sampler default_sampler;

layout(local_size_x = 16, local_size_y = 16) in;
void main() {
    ivec2 pixel_coords = ivec2(gl_GlobalInvocationID.xy);
    ivec2 image_size = imageSize(luma);

    if (pixel_coords.x >= image_size.x || pixel_coords.y >= image_size.y) {
        return;
    }

    vec4 rgba = texelFetch(sampler2D(inputImg, default_sampler), pixel_coords, 0);

    vec4 yuva = vec4(0.0);

    yuva.x = rgba.r * 0.299 + rgba.g * 0.587 + rgba.b * 0.114;
    yuva.y = rgba.r * -0.169 + rgba.g * -0.331 + rgba.b * 0.5 + 0.5;
    yuva.z = rgba.r * 0.5 + rgba.g * -0.419 + rgba.b * -0.081 + 0.5;
    yuva.w = rgba.a;

    imageStore(luma, pixel_coords, vec4(yuva.x));
    imageStore(alpha, pixel_coords, vec4(yuva.w));
    
    if (pixel_coords.x % 2 == 0 && pixel_coords.y % 2 == 0) {
        ivec2 chroma_coords = pixel_coords / 2;
        imageStore(chr_u, chroma_coords, vec4(yuva.y));
        imageStore(chr_v, chroma_coords, vec4(yuva.z));
    }
}