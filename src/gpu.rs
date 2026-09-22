//! Real kernel/shader emitters for CUDA, Vulkan compute and Metal.

pub fn emit_cuda_vector_add()->String{
r#"extern "C" __global__ void nova_vector_add(const float* a,const float* b,float* out,int n){
    int i=blockIdx.x*blockDim.x+threadIdx.x;
    if(i<n) out[i]=a[i]+b[i];
}"#.into()
}
pub fn emit_vulkan_vector_add()->String{
r#"#version 450
layout(local_size_x=256) in;
layout(set=0,binding=0) readonly buffer A{float a[];};
layout(set=0,binding=1) readonly buffer B{float b[];};
layout(set=0,binding=2) writeonly buffer O{float out[];};
layout(push_constant) uniform Params{uint n;} params;
void main(){uint i=gl_GlobalInvocationID.x;if(i<params.n)out[i]=a[i]+b[i];}
"#.into()
}
pub fn emit_metal_vector_add()->String{
r#"#include <metal_stdlib>
using namespace metal;
kernel void nova_vector_add(device const float*a[[buffer(0)]],device const float*b[[buffer(1)]],device float*out[[buffer(2)]],constant uint&n[[buffer(3)]],uint i[[thread_position_in_grid]]){
    if(i<n) out[i]=a[i]+b[i];
}
"#.into()
}
pub fn validate_sources()->Result<(),String>{
    for (name,src) in [("CUDA",emit_cuda_vector_add()),("Vulkan",emit_vulkan_vector_add()),("Metal",emit_metal_vector_add())]{
        if !src.contains("nova_vector_add"){return Err(format!("{} kernel missing",name));}
    }
    Ok(())
}
#[cfg(test)]mod tests{use super::*;#[test]fn all_targets_emit(){validate_sources().unwrap();}}
