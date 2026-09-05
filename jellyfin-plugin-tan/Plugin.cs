using System;
using System.Collections.Generic;
using Jellyfin.Plugin.Tan.Configuration;
using MediaBrowser.Common.Configuration;
using MediaBrowser.Common.Plugins;
using MediaBrowser.Model.Plugins;
using MediaBrowser.Model.Serialization;

namespace Jellyfin.Plugin.Tan;

/// <summary>
/// TAN Audio Normalizer for Jellyfin. Doesn't run any DSP itself - it's a
/// thin client of a local <c>tan-server</c> instance (see the tan-server
/// crate in the main TAN repo), which does the actual processing. Keeping
/// the DSP out of this plugin means it isn't reimplemented in C#, and the
/// same server can back other integrations later.
/// </summary>
public class Plugin : BasePlugin<PluginConfiguration>, IHasWebPages
{
    public Plugin(IApplicationPaths applicationPaths, IXmlSerializer xmlSerializer)
        : base(applicationPaths, xmlSerializer)
    {
        Instance = this;
    }

    public override string Name => "TAN Audio Normalizer";

    public override Guid Id => Guid.Parse("6f2f9c1a-6a3f-4b9a-9a7e-6f2c1c9b3a11");

    public override string Description =>
        "Runs library audio through TAN (via a local tan-server) to normalize poorly mixed sound - quiet dialogue vs loud action - without mastering-time metadata.";

    /// <summary>
    /// The one instance Jellyfin constructs; scheduled tasks reach the
    /// current settings through this rather than each holding their own copy.
    /// </summary>
    public static Plugin? Instance { get; private set; }

    public IEnumerable<PluginPageInfo> GetPages()
    {
        yield return new PluginPageInfo
        {
            Name = "tan",
            DisplayName = "TAN",
            EmbeddedResourcePath = $"{GetType().Namespace}.Configuration.configPage.html",
            EnableInMainMenu = true,
            MenuIcon = "graphic_eq",
        };
    }
}
