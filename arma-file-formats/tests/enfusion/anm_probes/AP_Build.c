#ifdef WORKBENCH
// Probe builder: registers (or rebuilds) every .txa listed in AP_ARGS, one absolute path per line.
[WorkbenchPluginAttribute("AP Build", "Build anm probes", wbModules: { "ResourceManager" })]
class AP_Build : WorkbenchPlugin
{
	static const string AP_ARGS = "C:/Users/User/Documents/My Games/ArmaReforgerWorkbench/addons/AnmProbe/ap_args.txt";

	override void RunCommandline()
	{
		ResourceManager rm = Workbench.GetModule(ResourceManager);
		FileHandle f = FileIO.OpenFile(AP_ARGS, FileMode.READ);
		string line;
		while (f && f.ReadLine(line) >= 0)
		{
			line.TrimInPlace();
			if (line.IsEmpty())
				continue;
			bool ok;
			if (FileIO.FileExists(line + ".meta") || FileIO.FileExists(line.Substring(0, line.Length() - 4) + ".anm.meta"))
			{
				rm.RebuildResourceFile(line, "PC", false);
				ok = true;
			}
			else
				ok = rm.RegisterResourceFile(line, true);
			bool waited = rm.WaitForFile(line, 5000);
			Print("AP_Build: " + line + " ok=" + ok.ToString() + " waited=" + waited.ToString(), LogLevel.NORMAL);
		}
		if (f)
			f.Close();
		Sleep(3000);
		Workbench.Exit(0);
	}
}
#endif
