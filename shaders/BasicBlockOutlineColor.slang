struct VSInput {
    float3 inPosition;
    float3 inColor;
    float2 inTexCoord;
    float3 inNormals;
}

Sampler2D textures[];

struct ShaderData {
    float4x4 model;
    float4x4 view;
    float4x4 projection;
    float4 color;
    uint textureIndex;
}

struct PushData {
    ShaderData* shaderData;
    uint renderableIndex;
}

[[vk::push_constant]]
PushData pushData;

struct VSOutput {
    float4 pos : SV_Position;
    float3 fragColor;
    float2 fragTexCoord;
    float3 fragNormals;
    uint textureIndex;
};

[shader("vertex")]
VSOutput vertMain(VSInput input) {
    VSOutput output;
    ShaderData* shaderData = pushData.shaderData + pushData.renderableIndex;
    output.pos = mul(shaderData->projection, mul(shaderData->view, mul(shaderData->model, float4(input.inPosition, 1.0))));
    output.fragColor = shaderData->color.rgb * (input.inColor + 1.0);
    output.fragTexCoord = input.inTexCoord;
    output.fragNormals = input.inNormals;
    output.textureIndex = shaderData->textureIndex;

    //output.fragColor = float3(1.0, 0.3, 0.1);
    //output.fragTexCoord = input.inTexCoord;
    //output.fragNormals = input.inNormals;
    //output.textureIndex = 0;
    return output;
}

[shader("fragment")]
float4 fragMain(VSOutput vertIn) : SV_Target
{
    float bw = 0.005;
    float2 uv = vertIn.fragTexCoord;

    bool isBorder = uv.x < bw || uv.x > 1.0 - bw ||
                    uv.y < bw || uv.y > 1.0 - bw;

    float4 textureColor = textures[NonUniformResourceIndex(vertIn.textureIndex)].Sample(vertIn.fragTexCoord);
    textureColor.rgb *= vertIn.fragColor;

    if (isBorder) {
        float4 borderColor = float4(0.0, 0.0, 0.0, 0.2); // transparent dark border
        return lerp(textureColor, borderColor, borderColor.a);
    }

    return textureColor;
}
