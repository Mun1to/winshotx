// S29: que teselas de 16x16 han cambiado entre dos fotogramas, comparado en la GPU.
//
// Lo usa `record/comparar_gpu.rs`. Compilado a `comparar.cso` (que es lo que va dentro del
// binario) con el fxc del SDK de Windows:
//
//   fxc /nologo /T cs_5_0 /E main /O3 /Fo comparar.cso comparar.hlsl
//
// Un grupo por tesela: cada hilo mira un pixel y, si alguno es distinto, el primero del grupo
// escribe un 1 en la casilla de su tesela. Cada casilla se escribe en cada pasada, asi que no
// hace falta limpiar la mascara antes.

Texture2D<float4> anterior : register(t0);
Texture2D<float4> actual : register(t1);
RWByteAddressBuffer mascara : register(u0);

groupshared uint distinto;

[numthreads(16, 16, 1)]
void main(uint3 grupo : SV_GroupID, uint3 pixel : SV_DispatchThreadID, uint indice : SV_GroupIndex)
{
    if (indice == 0)
        distinto = 0;
    GroupMemoryBarrierWithGroupSync();

    uint ancho, alto;
    actual.GetDimensions(ancho, alto);
    if (pixel.x < ancho && pixel.y < alto)
    {
        float4 a = anterior.Load(int3(pixel.xy, 0));
        float4 b = actual.Load(int3(pixel.xy, 0));
        if (any(a != b))
            InterlockedOr(distinto, 1u);
    }
    GroupMemoryBarrierWithGroupSync();

    if (indice == 0)
    {
        uint teselas_por_fila = (ancho + 15) / 16;
        mascara.Store((grupo.y * teselas_por_fila + grupo.x) * 4, distinto);
    }
}
