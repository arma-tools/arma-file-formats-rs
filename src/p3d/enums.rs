use std::marker::PhantomData;

use binrw::BinRead;
use derivative::Derivative;

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, BinRead)]
pub struct ClipFlags {
    pub value: i32,

    #[br(args { value })]
    pub res: ClipFlagsEnum,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, BinRead, Derivative)]
#[derivative(Default)]
#[br(repr = i32)]
#[br(import { value: i32 })]
pub enum ClipFlagsEnum {
    #[br(pre_assert(value == 0))]
    ClipNoneNormal,
    #[br(pre_assert(value == 1))]
    ClipFront = 1,
    #[br(pre_assert(value == 2))]
    ClipBack = 2,
    #[br(pre_assert(value == 4))]
    ClipLeft = 4,
    #[br(pre_assert(value == 8))]
    ClipRight = 8,
    #[br(pre_assert(value == 16))]
    ClipBottom = 16,
    #[br(pre_assert(value == 32))]
    ClipTop = 32,
    #[br(pre_assert(value == 64))]
    ClipUser0 = 64,
    #[br(pre_assert(value == 63))]
    ClipAll = 63,
    #[br(pre_assert(value == 3840))]
    ClipLandMask = 3840,
    #[br(pre_assert(value == 256))]
    ClipLandStep = 256,

    //ClipLandOn = 256,
    #[br(pre_assert(value == 512))]
    ClipLandUnder = 512,
    #[br(pre_assert(value == 1024))]
    ClipLandAbove = 1024,
    #[br(pre_assert(value == 2048))]
    ClipLandKeep = 2048,
    #[br(pre_assert(value == 12288))]
    ClipDecalMask = 12288,
    #[br(pre_assert(value == 4096))]
    ClipDecalStep = 4096,

    //ClipDecalNormal = 4096,
    #[br(pre_assert(value == 8192))]
    ClipDecalVertical = 8192,
    #[br(pre_assert(value == 49152))]
    ClipFogMask = 49152,
    #[br(pre_assert(value == 16384))]
    ClipFogStep = 16384,

    //ClipFogDisable = 16384,
    #[br(pre_assert(value == 32768))]
    ClipFogSky = 32768,
    #[br(pre_assert(value == 983_040))]
    ClipLightMask = 983_040,
    #[br(pre_assert(value == 65536))]
    ClipLightStep = 65536,

    #[br(pre_assert(value == 524_288))]
    ClipLightLine = 524_288,
    #[br(pre_assert(value ==267_386_880))]
    ClipUserMask = 267_386_880,
    #[br(pre_assert(value ==1_048_576))]
    ClipUserStep = 1_048_576,
    #[br(pre_assert(value == 255))]
    MaxUserValue = 255,
    #[br(pre_assert(value == 268_435_200))]
    ClipHints = 268_435_200,

    #[br(pre_assert(true))]
    #[derivative(Default)]
    Unknown,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, BinRead, Derivative)]
#[derivative(Default)]
#[br(repr = i32)]
pub enum EFogMode {
    #[derivative(Default)]
    None,
    Fog,
    Alpha,
    FogAlpha,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, BinRead, Derivative)]
#[derivative(Default)]
#[br(repr = i32)]
pub enum EMainLight {
    #[derivative(Default)]
    None,
    Sun,
    Sky,
    Horizon,
    Stars,
    SunObject,
    SunHaloObject,
    MoonObject,
    MoonHaloObject,
}

#[allow(clippy::enum_clike_unportable_variant)]
#[derive(Debug, PartialEq, Eq, Clone, Copy, BinRead, Derivative)]
#[derivative(Default)]
#[br(repr = u32)]
pub enum PixelShaderID {
    #[derivative(Default)]
    PSNormal,
    PSNormalDXTA,
    PSNormalMap,
    PSNormalMapThrough,
    PSNormalMapGrass,
    PSNormalMapDiffuse,
    PSDetail,
    PSInterpolation,
    PSWater,
    PSWaterSimple,
    PSWhite,
    PSWhiteAlpha,
    PSAlphaShadow,
    PSAlphaNoShadow,
    PSDummy0,
    PSDetailMacroAS,
    PSNormalMapMacroAS,
    PSNormalMapDiffuseMacroAS,
    PSNormalMapSpecularMap,
    PSNormalMapDetailSpecularMap,
    PSNormalMapMacroASSpecularMap,
    PSNormalMapDetailMacroASSpecularMap,
    PSNormalMapSpecularDIMap,
    PSNormalMapDetailSpecularDIMap,
    PSNormalMapMacroASSpecularDIMap,
    PSNormalMapDetailMacroASSpecularDIMap,
    PSTerrain1,
    PSTerrain2,
    PSTerrain3,
    PSTerrain4,
    PSTerrain5,
    PSTerrain6,
    PSTerrain7,
    PSTerrain8,
    PSTerrain9,
    PSTerrain10,
    PSTerrain11,
    PSTerrain12,
    PSTerrain13,
    PSTerrain14,
    PSTerrain15,
    PSTerrainSimple1,
    PSTerrainSimple2,
    PSTerrainSimple3,
    PSTerrainSimple4,
    PSTerrainSimple5,
    PSTerrainSimple6,
    PSTerrainSimple7,
    PSTerrainSimple8,
    PSTerrainSimple9,
    PSTerrainSimple10,
    PSTerrainSimple11,
    PSTerrainSimple12,
    PSTerrainSimple13,
    PSTerrainSimple14,
    PSTerrainSimple15,
    PSGlass,
    PSNonTL,
    PSNormalMapSpecularThrough,
    PSGrass,
    PSNormalMapThroughSimple,
    PSNormalMapSpecularThroughSimple,
    PSRoad,
    PSShore,
    PSShoreWet,
    PSRoad2Pass,
    PSShoreFoam,
    PSNonTLFlare,
    PSNormalMapThroughLowEnd,
    PSTerrainGrass1,
    PSTerrainGrass2,
    PSTerrainGrass3,
    PSTerrainGrass4,
    PSTerrainGrass5,
    PSTerrainGrass6,
    PSTerrainGrass7,
    PSTerrainGrass8,
    PSTerrainGrass9,
    PSTerrainGrass10,
    PSTerrainGrass11,
    PSTerrainGrass12,
    PSTerrainGrass13,
    PSTerrainGrass14,
    PSTerrainGrass15,
    PSCrater1,
    PSCrater2,
    PSCrater3,
    PSCrater4,
    PSCrater5,
    PSCrater6,
    PSCrater7,
    PSCrater8,
    PSCrater9,
    PSCrater10,
    PSCrater11,
    PSCrater12,
    PSCrater13,
    PSCrater14,
    PSSprite,
    PSSpriteSimple,
    PSCloud,
    PSHorizon,
    PSSuper,
    PSMulti,
    PSTerrainX,
    PSTerrainSimpleX,
    PSTerrainGrassX,
    PSTree,
    PSTreePRT,
    PSTreeSimple,
    PSSkin,
    PSCalmWater,
    PSTreeAToC,
    PSGrassAToC,
    PSTreeAdv,
    PSTreeAdvSimple,
    PSTreeAdvTrunk,
    PSTreeAdvTrunkSimple,
    PSTreeAdvAToC,
    PSTreeAdvSimpleAToC,
    PSTreeSN,
    PSSpriteExtTi,
    PSTerrainSNX,
    PSSimulWeatherClouds,
    PSSimulWeatherCloudsWithLightning,
    PSSimulWeatherCloudsCPU,
    PSSimulWeatherCloudsWithLightningCPU,
    PSSuperExt,
    PSSuperAToC,
    NPixelShaderID,
    //PSNone = 129,
    PSUninitialized = 4_294_967_295,
}

#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, BinRead)]
pub struct VertexShaderID {
    pub value: i32,

    #[br(args { value })]
    pub e: VertexShaderIDEnum,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, BinRead, Derivative)]
#[derivative(Default)]
#[br(import { value: i32 })]
pub enum VertexShaderIDEnum {
    #[br(pre_assert(value == 0i32))]
    VSBasic,
    #[br(pre_assert(value == 1))]
    VSNormalMap,
    #[br(pre_assert(value == 2))]
    VSNormalMapDiffuse,
    #[br(pre_assert(value == 3))]
    VSGrass,
    #[br(pre_assert(value == 4))]
    VSDummy1,
    #[br(pre_assert(value == 5))]
    VSDummy2,
    #[br(pre_assert(value == 6))]
    VSShadowVolume,
    #[br(pre_assert(value == 7))]
    VSWater,
    #[br(pre_assert(value == 8))]
    VSWaterSimple,
    #[br(pre_assert(value == 9))]
    VSSprite,
    #[br(pre_assert(value == 10))]
    VSPoint,
    #[br(pre_assert(value == 11))]
    VSNormalMapThrough,
    #[br(pre_assert(value == 12))]
    VSDummy3,
    #[br(pre_assert(value == 13))]
    VSTerrain,
    #[br(pre_assert(value == 14))]
    VSBasicAS,
    #[br(pre_assert(value == 15))]
    VSNormalMapAS,
    #[br(pre_assert(value == 16))]
    VSNormalMapDiffuseAS,
    #[br(pre_assert(value == 17))]
    VSGlass,
    #[br(pre_assert(value == 18))]
    VSNormalMapSpecularThrough,
    #[br(pre_assert(value == 19))]
    VSNormalMapThroughNoFade,
    #[br(pre_assert(value == 20))]
    VSNormalMapSpecularThroughNoFade,
    #[br(pre_assert(value == 21))]
    VSShore,
    #[br(pre_assert(value == 22))]
    VSTerrainGrass,
    #[br(pre_assert(value == 23))]
    VSSuper,
    #[br(pre_assert(value == 24))]
    VSMulti,
    #[br(pre_assert(value == 25))]
    VSTree,
    #[br(pre_assert(value == 26))]
    VSTreeNoFade,
    #[br(pre_assert(value == 27))]
    VSTreePRT,
    #[br(pre_assert(value == 28))]
    VSTreePRTNoFade,
    #[br(pre_assert(value == 29))]
    VSSkin,
    #[br(pre_assert(value == 30))]
    VSCalmWater,
    #[br(pre_assert(value == 31))]
    VSTreeAdv,
    #[br(pre_assert(value == 32))]
    VSTreeAdvTrunk,
    #[br(pre_assert(value == 33))]
    VSSimulWeatherClouds,
    #[br(pre_assert(value == 34))]
    VSSimulWeatherCloudsCPU,
    #[br(pre_assert(value == 35))]
    NVertexShaderID,
    #[br(pre_assert(true))]
    #[derivative(Default)]
    Unknown(PhantomData<f32>),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, BinRead, Derivative)]
#[derivative(Default)]
#[br(repr = u32)]
pub enum UVSource {
    #[derivative(Default)]
    UVNone,
    UVTex,
    UVTexWaterAnim,
    UVPos,
    UVNorm,
    UVTex1,
    UVWorldPos,
    UVWorldNorm,
    UVTexShoreAnim,
    NUVSource,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, BinRead, Derivative)]
#[derivative(Default)]
#[br(repr = u32)]
pub enum TextureFilterType {
    #[derivative(Default)]
    Point,
    Linear,
    Triliniear,
    Anisotropic,
}
