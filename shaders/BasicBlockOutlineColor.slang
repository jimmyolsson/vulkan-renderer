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
    float color; // unnused
}
ConstantBuffer<UniformBuffer> ubo;

struct VSOutput {
    float4 pos : SV_Position;
    float3 fragColor;
    float2 fragTexCoord;
    float3 fragNormals;
};

[shader("vertex")]
VSOutput vertMain(VSInput input) {
    VSOutput output;
    output.pos = mul(ubo.projection, mul(ubo.view, mul(ubo.model, float4(input.inPosition, 1.0))));
    output.fragColor = input.inColor;
    output.fragTexCoord = input.inTexCoord;
    output.fragNormals = input.inNormals;
    return output;
}

Sampler2D texture;

[shader("fragment")]
float4 fragMain(VSOutput vertIn) : SV_Target
{
    float bw = 0.005;
    float2 uv = vertIn.fragTexCoord;

    bool isBorder = uv.x < bw || uv.x > 1.0 - bw ||
                    uv.y < bw || uv.y > 1.0 - bw;

    //float4 textureColor = float4(texture.Sample(vertIn.fragTexCoord).rgb, 1.0);
    float4 textureColor = float4(1.0, 0.5, 0.31, 1);

    if (isBorder) {
        float4 borderColor = float4(0.0, 0.0, 0.0, 0.2); // transparent dark border
        float4 textureColor = float4(1.0, 0.5, 0.31, 1);
        return lerp(textureColor, borderColor, borderColor.a);
    }

    return textureColor;
}
