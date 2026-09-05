using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Security.Cryptography;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using MediaBrowser.Controller.Entities;
using MediaBrowser.Controller.Library;
using MediaBrowser.Model.Dto;
using MediaBrowser.Model.Entities;
using MediaBrowser.Model.MediaInfo;

namespace Jellyfin.Plugin.Tan;

/// <summary>
/// Offers a library item's TAN-normalized output (produced by
/// <see cref="ScheduledTasks.NormalizeLibraryTask"/>, if it exists) as an
/// additional playback source - "Play Version" in Jellyfin's clients lists
/// it alongside the original. This is what makes Jellyfin actually *offer*
/// the normalized copy, rather than it just sitting on disk.
///
/// Registered into Jellyfin's DI container by <see cref="TanServiceRegistrator"/>.
/// </summary>
public class TanMediaSourceProvider : IMediaSourceProvider
{
    public Task<IEnumerable<MediaSourceInfo>> GetMediaSources(BaseItem item, CancellationToken cancellationToken)
    {
        var config = Plugin.Instance?.Configuration;
        if (config == null || string.IsNullOrEmpty(item.Path))
        {
            return Task.FromResult(Enumerable.Empty<MediaSourceInfo>());
        }

        var originalStreams = item.GetMediaStreams();
        var hasVideo = originalStreams.Any(s => s.Type == MediaStreamType.Video);
        var tanPath = TanOutput.PathFor(item.Path, hasVideo, config.OutputFolder);
        if (!File.Exists(tanPath))
        {
            return Task.FromResult(Enumerable.Empty<MediaSourceInfo>());
        }

        var mediaStreams = new List<MediaStream>();
        var index = 0;
        if (hasVideo)
        {
            // The video is copied verbatim when muxing (see NormalizeLibraryTask),
            // so the original's own video stream description still applies.
            var originalVideo = originalStreams.First(s => s.Type == MediaStreamType.Video);
            mediaStreams.Add(new MediaStream
            {
                Type = MediaStreamType.Video,
                Index = index++,
                IsDefault = true,
                Codec = originalVideo.Codec,
                Width = originalVideo.Width,
                Height = originalVideo.Height,
            });
        }

        mediaStreams.Add(new MediaStream
        {
            Type = MediaStreamType.Audio,
            Index = index,
            IsDefault = true,
            Codec = hasVideo ? "aac" : "pcm_s16le",
            Channels = 2,
        });

        var source = new MediaSourceInfo
        {
            Id = StableId(tanPath),
            Path = tanPath,
            Protocol = MediaProtocol.File,
            Type = MediaSourceType.Default,
            Container = Path.GetExtension(tanPath).TrimStart('.'),
            Name = "TAN Normalized",
            IsRemote = false,
            RunTimeTicks = item.RunTimeTicks,
            SupportsDirectStream = true,
            SupportsDirectPlay = true,
            SupportsTranscoding = true,
            MediaStreams = mediaStreams,
        };

        return Task.FromResult<IEnumerable<MediaSourceInfo>>(new[] { source });
    }

    public Task<ILiveStream> OpenMediaSource(string openToken, List<ILiveStream> currentLiveStreams, CancellationToken cancellationToken)
    {
        // Only ever hands back plain static files (RequiresOpening is never
        // set), so Jellyfin should never call this - but the interface
        // requires an implementation.
        throw new NotSupportedException("TanMediaSourceProvider only supplies static file sources, which don't need opening.");
    }

    /// <summary>
    /// A deterministic id derived from the path - stable across server
    /// restarts, unlike <see cref="string.GetHashCode()"/> (randomized per
    /// process in modern .NET).
    /// </summary>
    private static string StableId(string path)
    {
        var hash = MD5.HashData(Encoding.UTF8.GetBytes(path));
        return Convert.ToHexString(hash);
    }
}
