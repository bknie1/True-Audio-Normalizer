using MediaBrowser.Controller;
using MediaBrowser.Controller.Library;
using MediaBrowser.Controller.Plugins;
using Microsoft.Extensions.DependencyInjection;

namespace Jellyfin.Plugin.Tan;

/// <summary>
/// Jellyfin's plugin loader looks for a class implementing this interface in
/// each plugin assembly and calls it during startup, before the DI container
/// is finalized - this is the hook that lets a plugin add its own services
/// (here, <see cref="TanMediaSourceProvider"/>) to the container. Needs a
/// parameterless constructor since it runs before DI is available.
/// </summary>
public class TanServiceRegistrator : IPluginServiceRegistrator
{
    public void RegisterServices(IServiceCollection serviceCollection, IServerApplicationHost applicationHost)
    {
        serviceCollection.AddSingleton<IMediaSourceProvider, TanMediaSourceProvider>();
    }
}
