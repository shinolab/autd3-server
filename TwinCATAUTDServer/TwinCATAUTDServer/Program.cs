using System;
using System.CommandLine;

namespace TwinCATAUTDServer
{
    internal class Program
    {
        [STAThread]
        private static int Main(string[] args)
        {
            Console.OutputEncoding = System.Text.Encoding.UTF8;

            var clientIpAddr = new Option<string>(
                aliases: new[] { "--client", "-c" },
                description: "Client IP address",
                getDefaultValue: () => ""
            );
            var sync0CycleTime = new Option<int>(
                aliases: new[] { "--sync0", "-s" },
                description: "Sync0 cycle time in units of 500us",
                getDefaultValue: () => 2
            );
            var taskCycleTime = new Option<int>(
                aliases: new[] { "--task", "-t" },
                description: "Send task cycle time in units of 500us",
                getDefaultValue: () => 2
            );
            var cpuBaseTime = new Option<int>(
                aliases: new[] { "--base", "-b" },
                description: "CPU base time in units of 500us",
                getDefaultValue: () => 1
            );
            var syncMode = new Option<SyncMode>(
                aliases: new[] { "--mode", "-m" },
                description: "Sync mode",
                getDefaultValue: () => SyncMode.DC
            );
            var keep = new Option<bool>(
                aliases: new[] { "--keep", "-k" },
                description: "Keep TwinCAT config window open",
                getDefaultValue: () => false
            );

            var rootCommand = new RootCommand("TwinCAT AUTD3 server");
            rootCommand.AddOption(clientIpAddr);
            rootCommand.AddOption(sync0CycleTime);
            rootCommand.AddOption(taskCycleTime);
            rootCommand.AddOption(cpuBaseTime);
            rootCommand.AddOption(syncMode);
            rootCommand.AddOption(keep);

            rootCommand.SetHandler(Setup, clientIpAddr, sync0CycleTime, taskCycleTime, cpuBaseTime, syncMode, keep);

            return rootCommand.Invoke(args);
        }

        [STAThread]
        private static void Setup(string clientIpAddr, int sync0CycleTime, int taskCycleTime, int cpuBaseTime, SyncMode syncMode, bool keep)
        {
            (new SetupTwinCAT(clientIpAddr, syncMode, 5000 * taskCycleTime, 5000 * cpuBaseTime, 500000 * sync0CycleTime, keep)).Run();
        }
    }
}
