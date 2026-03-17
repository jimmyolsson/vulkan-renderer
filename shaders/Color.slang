//struct VSInput {
//    float3 inPosition;
//}
struct VSInput {
    float3 inPosition;
    float3 inColor;
    float2 inTexCoord;
    float3 inNormals;
}

struct UniformBuffer {
    float4x4 model;
    float4x4 view;
    float4x4 projection;
    float3 color;
}
ConstantBuffer<UniformBuffer> ubo;

struct VSOutput {
    float4 pos : SV_Position;
    float3 color;
};

[shader("vertex")]
VSOutput vertMain(VSInput input) {
    VSOutput output;
    output.pos = mul(ubo.projection, mul(ubo.view, mul(ubo.model, float4(input.inPosition, 1.0))));
    output.color = ubo.color;
    return output;
}

[shader("fragment")]
float4 fragMain(VSOutput vertIn) : SV_Target
{
    return float4(vertIn.color, 1.0);
    //return float4(vertIn.color, 1.0);
}
