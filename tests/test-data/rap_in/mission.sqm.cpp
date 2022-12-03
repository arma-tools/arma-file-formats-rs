version=54;
class EditorData
{
	moveGridStep=1;
	angleGridStep=0.2617994;
	scaleGridStep=1;
	autoGroupingDist=10;
	toggles=1;
	class ItemIDProvider
	{
		nextID=2;
	};
	class Camera
	{
		pos[]={19379.848,63.36829,14597.484};
		dir[]={-0.32342288,-0.42198381,0.84695548};
		up[]={-0.15053864,0.90660214,0.3942188};
		aside[]={0.93420672,-6.6123903e-008,0.35674155};
	};
};
// comment = 1;
sourceName="binarized";
addons[]=
{
	"gm_characters_ge_characters"
};
class FogE;
class AddonsMetaData : FogE
{
	class List : ListOG
	{
		items=1;
		class Item0
		{
			className="gm_characters_ge_characters";
			name="gm_characters_ge_characters";
			author="Global Mobilization";
			url="global-mobilization.com";
		};
	};
};
dlcs[]=
{
	"gm"
};
randomSeed=11502127;
class ScenarioData
{
	author="Willard";
};
class Mission
{
	class Intel
	{
		timeOfChanges=1800.0002;
		startWeather=0.30000001;
		startWind=0.1;
		startWaves=0.1;
		forecastWeather=0.30000001;
		forecastWind=0.1;
		forecastWaves=0.1;
		forecastLightnings=0.1;
		year=2035;
		month=6;
		day=24;
		hour=12;
		minute=0;
		startFogDecay=0.014;
		forecastFogDecay=0.014;
	};
	class Entities
	{
		items=1;
		class Item0
		{
			dataType="Group";
			side="West";
			class Entities
			{
				items=1;
				class Item0
				{
					dataType="Object";
					class PositionInfo
					{
						position[]={19363.289,22.733288,14654.612};
					};
					side="West";
					flags=7;
					class Attributes
					{
						isPlayer=1;
					};
					id=1;
					type="gm_ge_army_officer_p1_80_oli";
					atlOffset=-2.6702881e-005;
				};
			};
			class Attributes
			{
				delete FogD;
			};
			id=0;
			atlOffset=-2.6702881e-005;
		};
	};
};
