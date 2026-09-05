using MediaBrowser.Model.Plugins;

namespace Jellyfin.Plugin.Tan.Configuration;

public class PluginConfiguration : BasePluginConfiguration
{
    /// <summary>
    /// Base URL of the tan-server instance to send audio to. Defaults to a
    /// tan-server running on the same machine as the Jellyfin server.
    /// </summary>
    public string ServerUrl { get; set; } = "http://127.0.0.1:5859";

    /// <summary>
    /// Which TAN profile to apply: universal, movie, music, speech, night, or
    /// game. See tan-core's Profile presets.
    /// </summary>
    public string Profile { get; set; } = "universal";

    /// <summary>
    /// Where normalized files are written. Empty means "next to the source
    /// file" (as a "&lt;name&gt; [TAN].wav" sidecar, never overwriting the
    /// original); set this to collect output in one place instead.
    /// </summary>
    public string OutputFolder { get; set; } = string.Empty;
}
