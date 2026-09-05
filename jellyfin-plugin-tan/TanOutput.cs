using System.IO;

namespace Jellyfin.Plugin.Tan;

/// <summary>
/// Where a TAN-normalized output lives for a given source item - shared by
/// <see cref="ScheduledTasks.NormalizeLibraryTask"/> (which writes it) and
/// <see cref="TanMediaSourceProvider"/> (which offers it to Jellyfin's
/// player), so the two can never disagree on the path.
/// </summary>
public static class TanOutput
{
    /// <summary>
    /// Video items get a muxed container (original picture, TAN audio);
    /// audio-only items get the processed WAV directly.
    /// </summary>
    public static string ExtensionFor(bool hasVideo) => hasVideo ? "mkv" : "wav";

    public static string PathFor(string sourcePath, bool hasVideo, string? outputFolder)
    {
        var dir = string.IsNullOrWhiteSpace(outputFolder)
            ? Path.GetDirectoryName(sourcePath) ?? "."
            : outputFolder;
        var name = $"{Path.GetFileNameWithoutExtension(sourcePath)} [TAN].{ExtensionFor(hasVideo)}";
        return Path.Combine(dir, name);
    }
}
