struct VSInput {
    float3 inPosition;
    float3 inColor;
    float2 inTexCoord;
    float3 inNormals;
}

struct ShaderData {
    float4x4 model;
    float4x4 view;
    float4x4 projection;
    float4 color;
}

struct PushData {
    ShaderData* shaderData;
    uint renderableIndex;
}

[[vk::push_constant]]
PushData pushData;

struct VSOutput {
    float4 pos : SV_Position;
    float3 color;
};

[shader("vertex")]
VSOutput vertMain(VSInput input) {
    VSOutput output;
    ShaderData* shaderData = pushData.shaderData + pushData.renderableIndex;
    output.pos = mul(shaderData->projection, mul(shaderData->view, mul(shaderData->model, float4(input.inPosition, 1.0))));
    output.color = shaderData->color.rgb * (input.inColor + 1.0);

    return output;
}

[shader("fragment")]
float4 fragMain(VSOutput vertIn) : SV_Target
{
    return float4(vertIn.color, 1.0);
    //return float4(vertIn.color, 1.0);
}
