use binrw::BinRead;
use derivative::Derivative;

#[allow(non_camel_case_types, clippy::enum_variant_names)]
#[derive(BinRead, Derivative, PartialEq, Clone, Copy)]
#[derivative(Debug, Default)]
#[br(repr = i32)]
pub enum ClipFlags {
    #[derivative(Default)]
    ClipNoneNormal,
    ClipFront = 1,
    ClipBack = 2,
    ClipLeft = 4,
    ClipRight = 8,
    ClipBottom = 16,
    ClipTop = 32,
    ClipUser0 = 64,
    ClipAll = 63,
    ClipLandMask = 3840,
    ClipLandStep = 256,

    //ClipLandOn = 256,
    ClipLandUnder = 512,
    ClipLandAbove = 1024,
    ClipLandKeep = 2048,
    ClipDecalMask = 12288,
    ClipDecalStep = 4096,

    //ClipDecalNormal = 4096,
    ClipDecalVertical = 8192,
    ClipFogMask = 49152,
    ClipFogStep = 16384,

    //ClipFogDisable = 16384,
    ClipFogSky = 32768,
    ClipLightMask = 983040,
    ClipLightStep = 65536,

    ClipLightLine = 524288,
    ClipUserMask = 267386880,
    ClipUserStep = 1048576,
    MaxUserValue = 255,
    ClipHints = 268435200,
}

#[allow(non_camel_case_types, clippy::enum_variant_names)]
#[derive(BinRead, Derivative, PartialEq)]
#[derivative(Debug, Default)]
#[br(repr = i32)]
pub enum EFogMode {
    #[derivative(Default)]
    FM_None,
    FM_Fog,
    FM_Alpha,
    FM_FogAlpha,
}

#[allow(non_camel_case_types, clippy::enum_variant_names)]
#[derive(BinRead, Derivative, PartialEq)]
#[derivative(Debug, Default)]
#[br(repr = i32)]
pub enum EMainLight {
    #[derivative(Default)]
    ML_None,
    ML_Sun,
    ML_Sky,
    ML_Horizon,
    ML_Stars,
    ML_SunObject,
    ML_SunHaloObject,
    ML_MoonObject,
    ML_MoonHaloObject,
}

#[allow(
    non_camel_case_types,
    clippy::enum_variant_names,
    clippy::enum_clike_unportable_variant
)]
#[derive(BinRead, Derivative, PartialEq)]
#[derivative(Debug, Default)]
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
    PSUninitialized = 4294967295,
}

#[allow(non_camel_case_types, clippy::enum_variant_names)]
#[derive(BinRead, Derivative, PartialEq)]
#[derivative(Debug, Default)]
#[br(repr = i32)]
pub enum VertexShaderID {
    #[derivative(Default)]
    VSBasic,
    VSNormalMap,
    VSNormalMapDiffuse,
    VSGrass,
    VSDummy1,
    VSDummy2,
    VSShadowVolume,
    VSWater,
    VSWaterSimple,
    VSSprite,
    VSPoint,
    VSNormalMapThrough,
    VSDummy3,
    VSTerrain,
    VSBasicAS,
    VSNormalMapAS,
    VSNormalMapDiffuseAS,
    VSGlass,
    VSNormalMapSpecularThrough,
    VSNormalMapThroughNoFade,
    VSNormalMapSpecularThroughNoFade,
    VSShore,
    VSTerrainGrass,
    VSSuper,
    VSMulti,
    VSTree,
    VSTreeNoFade,
    VSTreePRT,
    VSTreePRTNoFade,
    VSSkin,
    VSCalmWater,
    VSTreeAdv,
    VSTreeAdvTrunk,
    VSSimulWeatherClouds,
    VSSimulWeatherCloudsCPU,
    NVertexShaderID,
}

#[allow(non_camel_case_types, clippy::enum_variant_names)]
#[derive(BinRead, Derivative, PartialEq)]
#[derivative(Debug, Default)]
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

#[allow(non_camel_case_types, clippy::enum_variant_names)]
#[derive(BinRead, Derivative, PartialEq)]
#[derivative(Debug, Default)]
#[br(repr = u32)]
pub enum TextureFilterType {
    #[derivative(Default)]
    Point,
    Linear,
    Triliniear,
    Anisotropic,
}
